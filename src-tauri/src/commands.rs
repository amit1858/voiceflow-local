//! Tauri command handlers — the IPC surface exposed to the React frontend.
//!
//! Commands return `Result<_, VfError>`; the `Err` serializes to
//! `{ code, message, hint }` so the UI can render a friendly banner. See
//! `src/lib/ipc.ts` for the matching TypeScript wrappers.

use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::audio::recorder::ActiveRecording;
use crate::audio::temp::TempWav;
use crate::errors::VfError;
use crate::pipeline::{run_pipeline, PipelineResult};
use crate::rewrite::OutputMode;
use crate::state::{AppState, RecordingSession};

/// Start capturing microphone audio to a fresh temp WAV.
#[tauri::command]
pub fn start_recording(state: State<'_, AppState>) -> Result<(), VfError> {
    let mut guard = state
        .recording
        .lock()
        .map_err(|_| VfError::internal("recording state poisoned"))?;

    if guard.is_some() {
        return Err(VfError::AlreadyRecording);
    }

    // Allocate the temp path; the RAII guard deletes the file when dropped.
    let temp = TempWav::new();
    let active = ActiveRecording::start(temp.path().to_path_buf())?;

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

    // Clone the shared providers so we don't hold the `State` borrow across await.
    let transcriber = state.transcriber.clone();
    let rewriter = state.rewriter.clone();
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

    Ok(accelerator)
}
