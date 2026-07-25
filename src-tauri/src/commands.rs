//! Tauri command handlers — the IPC surface exposed to the React frontend.
//!
//! Commands return `Result<_, VfError>`; the `Err` serializes to
//! `{ code, message, hint }` so the UI can render a friendly banner. See
//! `src/lib/ipc.ts` for the matching TypeScript wrappers.

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::audio::recorder::ActiveRecording;
use crate::audio::temp::TempWav;
use crate::errors::VfError;
use crate::health::{self, HealthCheck};
use crate::pipeline::{run_pipeline, PipelineResult};
use crate::rewrite::OutputMode;
use crate::settings::Settings;
use crate::state::{AppState, RecordingSession};

/// Start capturing microphone audio to a fresh temp WAV.
#[tauri::command]
pub fn start_recording(app: AppHandle, state: State<'_, AppState>) -> Result<(), VfError> {
    let mut guard = state
        .recording
        .lock()
        .map_err(|_| VfError::internal("recording state poisoned"))?;

    if guard.is_some() {
        return Err(VfError::AlreadyRecording);
    }

    // Allocate the temp path; the RAII guard deletes the file when dropped.
    let temp = TempWav::new();

    // Emit a live input level (~10×/second) so the UI can show a mic meter and
    // the user can see whether the mic is actually picking up sound.
    let level_app = app.clone();
    let on_level: crate::audio::recorder::LevelCb =
        std::sync::Arc::new(move |level: f32| {
            let _ = level_app.emit("input-level", level);
        });
    let active = ActiveRecording::start(temp.path().to_path_buf(), Some(on_level))?;

    *guard = Some(RecordingSession { active, temp });
    Ok(())
}

/// Stop recording, run the transcribe → rewrite → style pipeline, and return
/// the preview text. The temp WAV is always deleted before returning.
#[tauri::command]
pub async fn stop_and_process(
    state: State<'_, AppState>,
    mode: OutputMode,
) -> Result<PipelineResult, VfError> {
    // Take ownership of the session so no other command can touch it.
    let session = {
        let mut guard = state
            .recording
            .lock()
            .map_err(|_| VfError::internal("recording state poisoned"))?;
        guard.take()
    }
    .ok_or(VfError::NotRecording)?;

    let RecordingSession { active, temp } = session;

    // `temp` is our RAII guard: on any early return below it drops and deletes
    // the WAV. We also call `cleanup()` explicitly on the success path.
    let wav_path = active.stop()?;

    // Build providers from the current settings (mock-first by default). A
    // misconfigured real provider fails closed here with a typed error (the
    // temp WAV is still cleaned up because `temp` drops on this early return).
    let transcriber = state.transcriber()?;
    let rewriter = state.rewriter();
    let style = state.style.clone();

    let result = run_pipeline(&wav_path, mode, &*transcriber, &*rewriter, &style).await;

    // Explicit cleanup on the happy path (Drop still covers error/panic paths).
    temp.cleanup();
    drop(temp);

    result
}

/// Cancel an in-progress recording and discard the audio without processing.
#[tauri::command]
pub fn cancel_recording(state: State<'_, AppState>) -> Result<(), VfError> {
    let session = {
        let mut guard = state
            .recording
            .lock()
            .map_err(|_| VfError::internal("recording state poisoned"))?;
        guard.take()
    };

    if let Some(RecordingSession { active, temp }) = session {
        active.cancel();
        temp.cleanup(); // drop also cleans up, but be explicit
    }
    Ok(())
}

/// Return the currently-configured global hotkey accelerator string.
#[tauri::command]
pub fn get_hotkey(state: State<'_, AppState>) -> Result<String, VfError> {
    state
        .hotkey
        .lock()
        .map(|h| h.clone())
        .map_err(|_| VfError::internal("hotkey state poisoned"))
}

/// Re-register the global hotkey. Returns the accepted accelerator string.
#[tauri::command]
pub fn set_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    accelerator: String,
) -> Result<String, VfError> {
    let accelerator = accelerator.trim().to_string();
    if accelerator.is_empty() {
        return Err(VfError::internal("hotkey must not be empty"));
    }

    let shortcuts = app.global_shortcut();

    // Unregister the previous hotkey (ignore errors — it may already be gone).
    if let Ok(prev) = state.hotkey.lock() {
        let _ = shortcuts.unregister(prev.as_str());
    }

    // Register the new one; the plugin's global handler emits "hotkey-toggle".
    shortcuts.register(accelerator.as_str()).map_err(|e| {
        VfError::internal(format!(
            "Could not register hotkey '{accelerator}': {e}. Use a form like 'Ctrl+Shift+Space'."
        ))
    })?;

    let mut guard = state
        .hotkey
        .lock()
        .map_err(|_| VfError::internal("hotkey state poisoned"))?;
    *guard = accelerator.clone();
    drop(guard);

    // Mirror the change into settings and persist it.
    if let Ok(mut s) = state.settings.lock() {
        s.hotkey = accelerator.clone();
        let _ = s.save(&state.app_data_dir);
    }

    Ok(accelerator)
}

/// Return the current user settings.
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, VfError> {
    Ok(state.current_settings())
}

/// Persist new settings. Re-registers the global hotkey if it changed.
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, VfError> {
    settings.save(&state.app_data_dir)?;

    // Determine the previous hotkey so we can re-register if it changed.
    let previous = state.hotkey.lock().ok().map(|h| h.clone());

    {
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| VfError::internal("settings state poisoned"))?;
        *guard = settings.clone();
    }

    if previous.as_deref() != Some(settings.hotkey.as_str()) {
        let shortcuts = app.global_shortcut();
        if let Some(prev) = previous {
            let _ = shortcuts.unregister(prev.as_str());
        }
        shortcuts.register(settings.hotkey.as_str()).map_err(|e| {
            VfError::internal(format!(
                "Could not register hotkey '{}': {e}. Use a form like 'Ctrl+Shift+Space'.",
                settings.hotkey
            ))
        })?;
        if let Ok(mut h) = state.hotkey.lock() {
            *h = settings.hotkey.clone();
        }
    }

    Ok(settings)
}

/// Run all provider/environment health checks against the current settings.
#[tauri::command]
pub async fn run_health_checks(
    state: State<'_, AppState>,
) -> Result<Vec<HealthCheck>, VfError> {
    let settings = state.current_settings();
    Ok(health::run_health_checks(&settings, &state.model_dir).await)
}

/// Delete any leftover `voiceflow-*.wav` temp files. Returns how many were
/// removed. Never a hard error unless the temp dir itself is unreadable.
#[tauri::command]
pub fn clear_temp_files() -> Result<usize, VfError> {
    let dir = std::env::temp_dir();
    let entries = std::fs::read_dir(&dir).map_err(|e| VfError::TempCleanupFailed {
        detail: format!("could not read {}: {e}", dir.display()),
    })?;

    let mut removed = 0usize;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("voiceflow-") && name.ends_with(".wav") {
            if std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}
