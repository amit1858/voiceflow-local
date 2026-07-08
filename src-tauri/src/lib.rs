//! VoiceFlow Local — Tauri application entry point (library crate).
//!
//! Wires up plugins (global shortcut, clipboard), registers the default hotkey,
//! resolves the whisper model directory, manages [`AppState`], and exposes the
//! command handlers to the React frontend.

mod audio;
mod commands;
mod errors;
mod pipeline;
mod rewrite;
mod state;
mod transcription;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use state::{AppState, DEFAULT_HOTKEY};

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
            // Resolve `<app data>/models`, where the whisper GGML model lives.
            let model_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("models");
            if let Err(e) = std::fs::create_dir_all(&model_dir) {
                eprintln!("Warning: could not create model dir {model_dir:?}: {e}");
            }

            app.manage(AppState::new(model_dir));

            // Register the default global hotkey. The plugin handler above emits
            // "hotkey-toggle" whenever it (or any later hotkey) is pressed.
            if let Err(e) = app.global_shortcut().register(DEFAULT_HOTKEY) {
                eprintln!("Warning: failed to register default hotkey {DEFAULT_HOTKEY}: {e}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_and_process,
            commands::cancel_recording,
            commands::get_hotkey,
            commands::set_hotkey,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VoiceFlow Local");
}
