//! User settings: provider selection, hotkey, defaults, and paths.
//!
//! Mock-first: a fresh install defaults **both** providers to `Mock`, so the
//! whole record → transcribe → rewrite → preview → copy workflow runs
//! end-to-end with zero local models installed. Users switch to the local
//! Whisper / Foundry providers from the Settings screen once ready.
//!
//! Settings persist to `<app data>/settings.json` (local config, git-ignored).
//! Transcripts and audio are never persisted — only these preferences are.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::VfError;
use crate::rewrite::OutputMode;

/// Which transcription back-end is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionKind {
    /// Deterministic canned transcript — no models required.
    Mock,
    /// Local whisper.cpp via `whisper-rs`.
    LocalWhisper,
}

/// Which rewrite back-end is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewriteKind {
    /// Deterministic rewrite that still applies mode formatting + style rules.
    Mock,
    /// Microsoft Foundry Local (phi-4-mini-instruct).
    FoundryLocal,
}

/// The GGML model filename expected in the app data dir's `models/` folder.
pub const MODEL_FILENAME: &str = "ggml-base.en.bin";
/// Default global hotkey (accelerator string parsed by the shortcut plugin).
pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
/// Default Foundry Local model id.
pub const DEFAULT_FOUNDRY_MODEL: &str = "phi-4-mini-instruct";

/// All user-configurable preferences. Field names use snake_case to match the
/// TypeScript `Settings` interface in `src/lib/types.ts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Global hotkey accelerator, e.g. `Ctrl+Shift+Space`.
    pub hotkey: String,
    /// Output mode selected by default on launch.
    pub default_mode: OutputMode,
    /// Active transcription provider.
    pub transcription_provider: TranscriptionKind,
    /// Active rewrite provider.
    pub rewrite_provider: RewriteKind,
    /// Absolute path to the Whisper GGML model file.
    pub whisper_model_path: String,
    /// Foundry Local model name.
    pub foundry_model: String,
    /// Optional manual Foundry endpoint override (for debugging). When set,
    /// dynamic port discovery is skipped.
    pub foundry_endpoint: Option<String>,
    /// Automatically copy the output to the clipboard after processing.
    pub auto_copy: bool,
}

impl Settings {
    /// Build defaults given the directory where the Whisper model should live.
    pub fn defaults(model_dir: &Path) -> Self {
        Settings {
            hotkey: DEFAULT_HOTKEY.to_string(),
            default_mode: OutputMode::Raw,
            // Mock-first: zero local setup required for a fresh checkout.
            transcription_provider: TranscriptionKind::Mock,
            rewrite_provider: RewriteKind::Mock,
            whisper_model_path: model_dir.join(MODEL_FILENAME).display().to_string(),
            foundry_model: DEFAULT_FOUNDRY_MODEL.to_string(),
            foundry_endpoint: None,
            auto_copy: false,
        }
    }

    /// Path of the settings file inside the app data dir.
    pub fn file_path(app_data_dir: &Path) -> PathBuf {
        app_data_dir.join("settings.json")
    }

    /// Load settings from disk, falling back to defaults (and writing them) if
    /// the file is absent. A corrupt file is treated as "use defaults" rather
    /// than a hard failure, so the app always starts.
    pub fn load_or_default(app_data_dir: &Path, model_dir: &Path) -> Self {
        let path = Self::file_path(app_data_dir);
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str::<Settings>(&contents)
                .unwrap_or_else(|_| Settings::defaults(model_dir)),
            Err(_) => {
                let defaults = Settings::defaults(model_dir);
                let _ = defaults.save(app_data_dir);
                defaults
            }
        }
    }

    /// Persist settings to `<app data>/settings.json`.
    pub fn save(&self, app_data_dir: &Path) -> Result<(), VfError> {
        std::fs::create_dir_all(app_data_dir).map_err(|e| VfError::SettingsError {
            detail: format!("could not create settings dir: {e}"),
        })?;
        let path = Self::file_path(app_data_dir);
        let json = serde_json::to_string_pretty(self).map_err(|e| VfError::SettingsError {
            detail: format!("could not serialize settings: {e}"),
        })?;
        std::fs::write(&path, json).map_err(|e| VfError::SettingsError {
            detail: format!("could not write settings: {e}"),
        })
    }
}
