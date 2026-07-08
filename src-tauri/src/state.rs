//! Shared application state managed by Tauri.
//!
//! Providers are built **per request** from the current [`Settings`] (see
//! [`AppState::transcriber`] / [`AppState::rewriter`]) so switching between Mock
//! and local providers in the Settings screen takes effect immediately without
//! restarting the app. Mock-first: a fresh install resolves both factories to
//! the deterministic mock providers.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::audio::recorder::ActiveRecording;
use crate::audio::temp::TempWav;
use crate::rewrite::foundry_local::FoundryLocalRewriteProvider;
use crate::rewrite::mock::MockRewriteProvider;
use crate::rewrite::{RewriteProvider, StyleRules};
use crate::settings::{RewriteKind, Settings, TranscriptionKind};
use crate::transcription::mock::MockTranscriptionProvider;
use crate::transcription::whisper_cpp::LocalWhisperTranscriptionProvider;
use crate::transcription::TranscriptionProvider;

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
    /// The currently-registered global hotkey (mirrors `settings.hotkey`).
    pub hotkey: Mutex<String>,
    /// User settings; providers are rebuilt from these on each request.
    pub settings: Mutex<Settings>,
    /// The app data dir (settings.json lives here).
    pub app_data_dir: PathBuf,
    /// The folder where the Whisper GGML model should live.
    pub model_dir: PathBuf,
    /// Shared writing-style rules.
    pub style: StyleRules,
}

impl AppState {
    /// Build state from resolved directories and loaded settings.
    pub fn new(app_data_dir: PathBuf, model_dir: PathBuf, settings: Settings) -> Self {
        AppState {
            recording: Mutex::new(None),
            hotkey: Mutex::new(settings.hotkey.clone()),
            settings: Mutex::new(settings),
            app_data_dir,
            model_dir,
            style: StyleRules::default(),
        }
    }

    /// Snapshot the current settings.
    pub fn current_settings(&self) -> Settings {
        self.settings
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| Settings::defaults(&self.model_dir))
    }

    /// Build a transcription provider from the active settings.
    pub fn transcriber(&self) -> Box<dyn TranscriptionProvider> {
        let settings = self.current_settings();
        match settings.transcription_provider {
            TranscriptionKind::Mock => Box::new(MockTranscriptionProvider::new()),
            TranscriptionKind::LocalWhisper => Box::new(LocalWhisperTranscriptionProvider::new(
                PathBuf::from(settings.whisper_model_path),
            )),
        }
    }

    /// Build a rewrite provider from the active settings.
    pub fn rewriter(&self) -> Box<dyn RewriteProvider> {
        let settings = self.current_settings();
        match settings.rewrite_provider {
            RewriteKind::Mock => Box::new(MockRewriteProvider::new()),
            RewriteKind::FoundryLocal => Box::new(FoundryLocalRewriteProvider::new(
                settings.foundry_model,
                settings.foundry_endpoint,
            )),
        }
    }
}
