//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::audio::recorder::ActiveRecording;
use crate::audio::temp::TempWav;
use crate::rewrite::foundry_local::FoundryLocalProvider;
use crate::rewrite::{RewriteProvider, StyleRules};
use crate::transcription::whisper_cpp::WhisperCppProvider;
use crate::transcription::TranscriptionProvider;

/// Default global hotkey (accelerator string parsed by the shortcut plugin).
pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";

/// The GGML model filename expected in the app data dir's `models/` folder.
pub const MODEL_FILENAME: &str = "ggml-base.en.bin";

/// An in-progress recording plus the RAII temp-file guard for its WAV. Keeping
/// the [`TempWav`] here guarantees the file is deleted even if processing is
/// abandoned (drop the session and the WAV is removed).
pub struct RecordingSession {
    pub active: ActiveRecording,
    pub temp: TempWav,
}

/// Application-wide state shared across Tauri commands.
pub struct AppState {
    /// The in-progress recording, if any.
    pub recording: Mutex<Option<RecordingSession>>,
    /// The currently-registered global hotkey.
    pub hotkey: Mutex<String>,
    /// Local transcription provider.
    pub transcriber: Arc<dyn TranscriptionProvider>,
    /// Local rewrite provider.
    pub rewriter: Arc<dyn RewriteProvider>,
    /// Shared writing-style rules.
    pub style: StyleRules,
}

impl AppState {
    /// Build state with the default local providers. `model_dir` is the folder
    /// where the whisper GGML model should live (`<app data>/models`).
    pub fn new(model_dir: PathBuf) -> Self {
        let model_path = model_dir.join(MODEL_FILENAME);
        AppState {
            recording: Mutex::new(None),
            hotkey: Mutex::new(DEFAULT_HOTKEY.to_string()),
            transcriber: Arc::new(WhisperCppProvider::new(model_path)),
            rewriter: Arc::new(FoundryLocalProvider::new()),
            style: StyleRules::default(),
        }
    }
}
