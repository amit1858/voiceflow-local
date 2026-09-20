//! User settings: provider selection, hotkey, defaults, and model choices.
//!
//! Debug builds retain explicit mock providers for development. Consumer
//! release builds default to Sherpa STT and migrate any persisted mock STT
//! selection to the real local provider.
//!
//! Settings persist to `<app data>/settings.json` (local config, git-ignored).
//! Transcripts and audio are never persisted — only these preferences are.
//!
//! ## Migration
//! Old v1 settings files are upgraded **gracefully**: the legacy
//! `transcription_provider: "local_whisper"` value is accepted (serde alias) and
//! treated as `sherpa`; the removed `whisper_model_path` field is ignored; and
//! every new field carries a serde default, so a v1 `settings.json` loads without
//! losing the user's hotkey, mode, or Foundry preferences.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::VfError;
use crate::models::{self, ModelKind};
use crate::rewrite::OutputMode;

/// Which transcription back-end is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionKind {
    /// Deterministic canned transcript — no models required.
    Mock,
    /// Local, self-contained speech-to-text via sherpa-onnx (prebuilt binaries).
    /// Accepts the legacy `local_whisper` value for backward compatibility.
    #[serde(alias = "local_whisper")]
    Sherpa,
}

/// Which text-to-speech back-end is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsKind {
    /// Silent/stub synthesis (a short tone) — no voice model required.
    Mock,
    /// Local neural TTS via sherpa-onnx (VITS/Piper/Kokoro).
    Sherpa,
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

/// Default global hotkey (accelerator string parsed by the shortcut plugin).
pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
/// Default Foundry Local model id.
pub const DEFAULT_FOUNDRY_MODEL: &str = "phi-4-mini-instruct";

fn default_stt_model() -> String {
    models::default_id(ModelKind::Stt).to_string()
}

fn default_tts_voice() -> String {
    models::default_id(ModelKind::Tts).to_string()
}

fn default_transcription_provider() -> TranscriptionKind {
    Settings::runtime_mode().default_transcription_provider()
}

fn default_tts_provider() -> TtsKind {
    TtsKind::Mock
}

fn default_rewrite_provider() -> RewriteKind {
    RewriteKind::Mock
}

fn default_hotkey() -> String {
    DEFAULT_HOTKEY.to_string()
}

fn default_foundry_model() -> String {
    DEFAULT_FOUNDRY_MODEL.to_string()
}

fn default_mode() -> OutputMode {
    OutputMode::Raw
}

/// All user-configurable preferences. Field names use snake_case to match the
/// TypeScript `Settings` interface in `src/lib/types.ts`. Every field has a
/// serde default so older settings files migrate without data loss.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Global hotkey accelerator, e.g. `Ctrl+Shift+Space`.
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    /// Output mode selected by default on launch.
    #[serde(default = "default_mode")]
    pub default_mode: OutputMode,
    /// Active transcription provider.
    #[serde(default = "default_transcription_provider")]
    pub transcription_provider: TranscriptionKind,
    /// Active rewrite provider.
    #[serde(default = "default_rewrite_provider")]
    pub rewrite_provider: RewriteKind,
    /// Registry id of the selected STT model (e.g. `whisper-tiny-en`).
    #[serde(default = "default_stt_model")]
    pub stt_model: String,
    /// Active text-to-speech provider.
    #[serde(default = "default_tts_provider")]
    pub tts_provider: TtsKind,
    /// Registry id of the selected TTS voice (e.g. `vits-ljs`).
    #[serde(default = "default_tts_voice")]
    pub tts_voice: String,
    /// Speak the output automatically after processing completes.
    #[serde(default)]
    pub auto_speak: bool,
    /// Foundry Local model name.
    #[serde(default = "default_foundry_model")]
    pub foundry_model: String,
    /// Optional manual Foundry endpoint override (for debugging). When set,
    /// dynamic port discovery is skipped.
    #[serde(default)]
    pub foundry_endpoint: Option<String>,
    /// Automatically copy the output to the clipboard after processing.
    #[serde(default)]
    pub auto_copy: bool,
}

impl Settings {
    pub fn runtime_mode() -> RuntimeMode {
        if cfg!(debug_assertions) {
            RuntimeMode::Development
        } else {
            RuntimeMode::Consumer
        }
    }

    /// Build defaults. `_model_dir` is kept for signature stability with callers
    /// (models are now resolved by registry id under the app-data models dir).
    pub fn defaults(_model_dir: &Path) -> Self {
        Settings {
            hotkey: DEFAULT_HOTKEY.to_string(),
            default_mode: OutputMode::Raw,
            transcription_provider: Self::runtime_mode().default_transcription_provider(),
            rewrite_provider: RewriteKind::Mock,
            stt_model: default_stt_model(),
            tts_provider: TtsKind::Mock,
            tts_voice: default_tts_voice(),
            auto_speak: false,
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
    /// than a hard failure, so the app always starts. Older v1 files migrate
    /// automatically via serde defaults + the `local_whisper` alias.
    pub fn load_or_default(app_data_dir: &Path, model_dir: &Path) -> Self {
        let path = Self::file_path(app_data_dir);
        match std::fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<Settings>(&contents) {
                Ok(mut settings) => {
                    // Repair any unknown/blank model ids after migration so we
                    // never point the engine at a model that isn't registered.
                    settings.normalize_for_mode(Self::runtime_mode());
                    // Persist the upgraded shape so the file is current.
                    let _ = settings.save(app_data_dir);
                    settings
                }
                Err(_) => {
                    let defaults = Settings::defaults(model_dir);
                    let _ = defaults.save(app_data_dir);
                    defaults
                }
            },
            Err(_) => {
                let defaults = Settings::defaults(model_dir);
                let _ = defaults.save(app_data_dir);
                defaults
            }
        }
    }

    /// Ensure model ids reference registered models; fall back to the bundled
    /// defaults otherwise.
    fn normalize_for_mode(&mut self, mode: RuntimeMode) {
        if !mode.mock_transcription_allowed()
            && self.transcription_provider == TranscriptionKind::Mock
        {
            self.transcription_provider = TranscriptionKind::Sherpa;
        }
        if models::find(&self.stt_model).map(|e| e.kind) != Some(ModelKind::Stt) {
            self.stt_model = default_stt_model();
        }
        if models::find(&self.tts_voice).map(|e| e.kind) != Some(ModelKind::Tts) {
            self.tts_voice = default_tts_voice();
        }
    }

    pub fn normalize_for_runtime(&mut self) {
        self.normalize_for_mode(Self::runtime_mode());
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Development,
    Consumer,
}

impl RuntimeMode {
    pub fn default_transcription_provider(self) -> TranscriptionKind {
        match self {
            RuntimeMode::Development => TranscriptionKind::Mock,
            RuntimeMode::Consumer => TranscriptionKind::Sherpa,
        }
    }

    pub fn mock_transcription_allowed(self) -> bool {
        matches!(self, RuntimeMode::Development)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_and_consumer_defaults_are_distinct() {
        assert_eq!(
            RuntimeMode::Development.default_transcription_provider(),
            TranscriptionKind::Mock
        );
        assert_eq!(
            RuntimeMode::Consumer.default_transcription_provider(),
            TranscriptionKind::Sherpa
        );
        let s = Settings::defaults(Path::new("."));
        assert_eq!(
            s.transcription_provider,
            Settings::runtime_mode().default_transcription_provider()
        );
        assert_eq!(s.tts_provider, TtsKind::Mock);
        assert_eq!(s.rewrite_provider, RewriteKind::Mock);
        assert!(!s.auto_speak);
        assert!(!s.auto_copy);
        assert_eq!(s.stt_model, "whisper-tiny-en");
        assert_eq!(s.tts_voice, "vits-ljs");
    }

    #[test]
    fn migrates_legacy_local_whisper_value() {
        // A v1 settings.json used "local_whisper" and had a whisper_model_path.
        let legacy = r#"{
            "hotkey": "Ctrl+Alt+V",
            "default_mode": "teams",
            "transcription_provider": "local_whisper",
            "rewrite_provider": "foundry_local",
            "whisper_model_path": "C:/old/ggml-base.en.bin",
            "foundry_model": "phi-4-mini-instruct",
            "foundry_endpoint": null,
            "auto_copy": true
        }"#;
        let mut s: Settings = serde_json::from_str(legacy).expect("legacy settings should migrate");
        s.normalize_for_mode(RuntimeMode::Development);
        // Legacy provider maps to Sherpa; unknown fields are dropped.
        assert_eq!(s.transcription_provider, TranscriptionKind::Sherpa);
        // Preserved fields survive migration.
        assert_eq!(s.hotkey, "Ctrl+Alt+V");
        assert_eq!(s.default_mode, OutputMode::Teams);
        assert_eq!(s.rewrite_provider, RewriteKind::FoundryLocal);
        assert!(s.auto_copy);
        // New fields get sensible defaults.
        assert_eq!(s.tts_provider, TtsKind::Mock);
        assert_eq!(s.stt_model, "whisper-tiny-en");
        assert_eq!(s.tts_voice, "vits-ljs");
        assert!(!s.auto_speak);
    }

    #[test]
    fn normalize_repairs_unknown_model_ids() {
        let mut s = Settings::defaults(Path::new("."));
        s.stt_model = "no-such-model".to_string();
        s.tts_voice = "no-such-voice".to_string();
        s.normalize_for_mode(RuntimeMode::Development);
        assert_eq!(s.stt_model, "whisper-tiny-en");
        assert_eq!(s.tts_voice, "vits-ljs");
    }

    #[test]
    fn roundtrips_new_shape() {
        let s = Settings::defaults(Path::new("."));
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.transcription_provider, s.transcription_provider);
        assert_eq!(back.tts_provider, s.tts_provider);
        assert_eq!(back.stt_model, s.stt_model);
    }

    #[test]
    fn consumer_migration_never_keeps_mock_transcription() {
        let mut settings = Settings::defaults(Path::new("."));
        settings.transcription_provider = TranscriptionKind::Mock;
        settings.normalize_for_mode(RuntimeMode::Consumer);
        assert_eq!(settings.transcription_provider, TranscriptionKind::Sherpa);
    }
}
