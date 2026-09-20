//! VoiceFlow Local — Tauri application entry point (library crate).
//!
//! Wires up plugins (global shortcut, clipboard), registers the default hotkey,
//! resolves the whisper model directory, manages [`AppState`], and exposes the
//! command handlers to the React frontend.

#[cfg(all(not(debug_assertions), not(feature = "sherpa")))]
compile_error!(
    "Consumer release builds must include the `sherpa` feature; mock-only release builds are forbidden."
);

mod audio;
mod commands;
mod errors;
mod health;
mod models;
mod pipeline;
mod rewrite;
mod settings;
mod state;
mod transcription;
mod tts;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use settings::Settings;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    // Toggle recording on key-down; ignore the release event.
                    if event.state == ShortcutState::Pressed {
                        let _ = app.emit("hotkey-toggle", ());
                    }
                })
                .build(),
        )
        .setup(|app| {
            // Resolve the app data dir and `<app data>/models`, where the whisper
            // GGML model lives.
            let app_data_dir = app.path().app_data_dir()?;
            let model_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&model_dir)?;

            let stale = crate::audio::temp::cleanup_stale()?;
            if !stale.failures.is_empty() {
                eprintln!(
                    "VoiceFlow temp cleanup removed {} file(s), but {} failed: {}",
                    stale.removed,
                    stale.failures.len(),
                    stale.failures.join("; ")
                );
            }

            if !cfg!(debug_assertions) {
                let resource_dir = app.path().resource_dir()?;
                crate::models::install_bundled_models(&resource_dir.join("resources"), &model_dir)
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("consumer speech model installation failed: {e}"),
                        )
                    })?;
            }

            // Debug defaults to explicit mocks; consumer release defaults to Sherpa.
            let settings = Settings::load_or_default(&app_data_dir, &model_dir);
            let hotkey = settings.hotkey.clone();

            app.manage(AppState::new(app_data_dir, model_dir, settings));

            // Register the configured global hotkey. The plugin handler above
            // emits "hotkey-toggle" whenever it is pressed.
            if let Err(e) = app.global_shortcut().register(hotkey.as_str()) {
                eprintln!("Warning: failed to register hotkey {hotkey}: {e}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_and_process,
            commands::cancel_recording,
            commands::get_hotkey,
            commands::set_hotkey,
            commands::get_settings,
            commands::save_settings,
            commands::run_health_checks,
            commands::clear_temp_files,
            commands::speak,
            commands::stop_speaking,
            commands::list_voices,
            commands::list_models,
            commands::download_model,
            commands::get_capabilities,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VoiceFlow Local");
}
