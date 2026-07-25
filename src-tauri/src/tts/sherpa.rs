//! `SherpaTtsProvider` — local neural text-to-speech via sherpa-onnx (VITS).
//!
//! The counterpart to [`crate::transcription::sherpa::SherpaSttProvider`]. It
//! loads a VITS voice (model + tokens + lexicon) resolved from the app-data
//! models dir by registry id (see [`crate::models`]) and synthesizes speech off
//! the async executor. Backed by the same prebuilt sherpa-onnx binaries — end
//! users of the release build need no CMake / C++ / libclang toolchain.
//!
//! If the voice files are missing it returns [`VfError::TtsVoiceMissing`] with
//! the exact expected path; if the binary was built without the `sherpa`
//! feature it returns [`VfError::SpeechEngineUnavailable`] (never silent).

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::errors::VfError;
use crate::models::{self, FileRole, ModelKind};
use crate::tts::{Synthesized, TtsProvider, Voice};

/// Hint shown when the selected voice's files are absent.
const VOICE_DOWNLOAD_HINT: &str =
    "Open Settings and download the voice, or run scripts/setup-local-models.ps1. \
The bundled default voice also ships with the packaged app.";

/// Text-to-speech provider backed by sherpa-onnx (VITS ONNX).
#[derive(Debug)]
pub struct SherpaTtsProvider {
    voice_id: String,
    model: PathBuf,
    tokens: PathBuf,
    lexicon: PathBuf,
}

impl SherpaTtsProvider {
    /// Resolve a provider for TTS voice `voice_id` under `models_root`.
    pub fn from_voice(models_root: &Path, voice_id: &str) -> Result<Self, VfError> {
        let entry = models::find(voice_id)
            .filter(|e| e.kind == ModelKind::Tts)
            .ok_or_else(|| VfError::UnknownModel { id: voice_id.to_string() })?;

        let role = |r: FileRole| {
            entry
                .path_for_role(models_root, r)
                .ok_or_else(|| VfError::Internal {
                    detail: format!("TTS voice `{voice_id}` is missing a required file role"),
                })
        };

        Ok(SherpaTtsProvider {
            voice_id: voice_id.to_string(),
            model: role(FileRole::TtsModel)?,
            tokens: role(FileRole::TtsTokens)?,
            lexicon: role(FileRole::TtsLexicon)?,
        })
    }

    /// The selected voice id (used by health checks / diagnostics).
    #[allow(dead_code)]
    pub fn voice_id(&self) -> &str {
        &self.voice_id
    }

    /// Verify every voice file exists, returning a precise
    /// [`VfError::TtsVoiceMissing`] otherwise.
    pub fn ensure_voice_present(&self) -> Result<(), VfError> {
        for path in [&self.model, &self.tokens, &self.lexicon] {
            let present = std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false);
            if !present {
                return Err(VfError::TtsVoiceMissing {
                    expected_path: path.display().to_string(),
                    hint: VOICE_DOWNLOAD_HINT.to_string(),
                });
            }
        }
        Ok(())
    }
}

#[async_trait]
impl TtsProvider for SherpaTtsProvider {
    async fn synthesize(&self, text: &str) -> Result<Synthesized, VfError> {
        self.ensure_voice_present()?;

        let model = self.model.clone();
        let tokens = self.tokens.clone();
        let lexicon = self.lexicon.clone();
        let text = text.to_string();

        tokio::task::spawn_blocking(move || run_sherpa_tts(&model, &tokens, &lexicon, &text))
            .await
            .map_err(|e| VfError::TtsSynthFailed { detail: format!("join error: {e}") })?
    }

    fn list_voices(&self) -> Vec<Voice> {
        // Every registered TTS voice, with installed reflecting on-disk presence
        // relative to this provider's model directory root.
        let root = self
            .model
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        models::REGISTRY
            .iter()
            .filter(|e| e.kind == ModelKind::Tts)
            .map(|e| Voice {
                id: e.id.to_string(),
                display_name: e.display_name.to_string(),
                installed: root.as_ref().map(|r| e.is_present(r)).unwrap_or(false),
            })
            .collect()
    }
}

#[cfg(feature = "sherpa")]
fn run_sherpa_tts(
    model: &Path,
    tokens: &Path,
    lexicon: &Path,
    text: &str,
) -> Result<Synthesized, VfError> {
    use sherpa_rs::tts::{VitsTts, VitsTtsConfig};

    let config = VitsTtsConfig {
        model: model.to_string_lossy().to_string(),
        tokens: tokens.to_string_lossy().to_string(),
        lexicon: lexicon.to_string_lossy().to_string(),
        num_threads: 2,
        ..Default::default()
    };

    let mut tts = VitsTts::new(config);
    let audio = tts
        .create(text, 0, 1.0)
        .map_err(|e| VfError::TtsSynthFailed { detail: e.to_string() })?;

    Ok(Synthesized {
        samples: audio.samples,
        sample_rate: audio.sample_rate as u32,
    })
}

#[cfg(not(feature = "sherpa"))]
fn run_sherpa_tts(
    _model: &Path,
    _tokens: &Path,
    _lexicon: &Path,
    _text: &str,
) -> Result<Synthesized, VfError> {
    Err(VfError::SpeechEngineUnavailable {
        detail: "This build was compiled without the `sherpa` feature, so local neural \
text-to-speech is unavailable. Use the speech-enabled release build (prebuilt binaries, no \
toolchain) or build with `--features sherpa`. Mock TTS (beep) keeps working."
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_voice_rejects_unknown_id() {
        let err = SherpaTtsProvider::from_voice(Path::new("."), "nope").unwrap_err();
        assert_eq!(err.code(), "UnknownModel");
    }

    #[test]
    fn from_voice_rejects_stt_model_as_voice() {
        // whisper-tiny-en is an STT model, not a TTS voice.
        let err = SherpaTtsProvider::from_voice(Path::new("."), "whisper-tiny-en").unwrap_err();
        assert_eq!(err.code(), "UnknownModel");
    }

    #[test]
    fn ensure_voice_present_reports_missing_path() {
        let root = std::env::temp_dir().join(format!("vf-tts-{}", uuid::Uuid::new_v4()));
        let p = SherpaTtsProvider::from_voice(&root, "vits-ljs").unwrap();
        let err = p.ensure_voice_present().unwrap_err();
        assert_eq!(err.code(), "TtsVoiceMissing");
    }

    #[cfg(not(feature = "sherpa"))]
    #[tokio::test]
    async fn synthesize_without_feature_is_engine_unavailable() {
        let root = std::env::temp_dir().join(format!("vf-tts2-{}", uuid::Uuid::new_v4()));
        let entry = models::find("vits-ljs").unwrap();
        let dir = entry.dir(&root);
        std::fs::create_dir_all(&dir).unwrap();
        for f in entry.files {
            std::fs::write(dir.join(f.filename), b"x").unwrap();
        }
        let p = SherpaTtsProvider::from_voice(&root, "vits-ljs").unwrap();
        let err = p.synthesize("hello").await.unwrap_err();
        assert_eq!(err.code(), "SpeechEngineUnavailable");
        let _ = std::fs::remove_dir_all(&root);
    }
}
