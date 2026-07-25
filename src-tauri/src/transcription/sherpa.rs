//! `SherpaSttProvider` — local speech-to-text via sherpa-onnx.
//!
//! Replaces the old compile-from-source whisper.cpp path. The engine is
//! sherpa-onnx (through the `sherpa-rs` crate) which ships **prebuilt** ONNX
//! Runtime + sherpa-onnx binaries via its `download-binaries` feature — so end
//! users of the release build need no CMake / C++ / libclang toolchain.
//!
//! The provider loads a Whisper ONNX model (encoder + decoder + tokens) resolved
//! from the app-data models dir by registry id (see [`crate::models`]). If the
//! model files are missing it returns [`VfError::ModelMissing`] with the exact
//! expected path; if the binary was built without the `sherpa` feature it
//! returns [`VfError::SpeechEngineUnavailable`] (never a silent canned success —
//! the UI capability badge and health checks surface this up front).

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::errors::VfError;
use crate::models::{self, FileRole, ModelKind};
use crate::transcription::TranscriptionProvider;

/// Hint shown when the STT model files are absent.
const MODEL_DOWNLOAD_HINT: &str =
    "Open Settings and download the speech-to-text model, or run \
scripts/setup-local-models.ps1. The bundled tiny model also ships with the packaged app.";

/// Transcription provider backed by sherpa-onnx (Whisper ONNX).
#[derive(Debug)]
pub struct SherpaSttProvider {
    model_id: String,
    encoder: PathBuf,
    decoder: PathBuf,
    tokens: PathBuf,
    language: String,
}

impl SherpaSttProvider {
    /// Resolve a provider for STT model `model_id` under `models_root`.
    pub fn from_model(models_root: &Path, model_id: &str) -> Result<Self, VfError> {
        let entry = models::find(model_id).filter(|e| e.kind == ModelKind::Stt).ok_or_else(
            || VfError::UnknownModel { id: model_id.to_string() },
        )?;

        let role = |r: FileRole| {
            entry
                .path_for_role(models_root, r)
                .ok_or_else(|| VfError::Internal {
                    detail: format!("STT model `{model_id}` is missing a required file role"),
                })
        };

        Ok(SherpaSttProvider {
            model_id: model_id.to_string(),
            encoder: role(FileRole::SttEncoder)?,
            decoder: role(FileRole::SttDecoder)?,
            tokens: role(FileRole::SttTokens)?,
            language: "en".to_string(),
        })
    }

    /// The selected model id (used by health checks / diagnostics).
    #[allow(dead_code)]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Verify every model file exists, returning a precise
    /// [`VfError::ModelMissing`] otherwise.
    pub fn ensure_model_present(&self) -> Result<(), VfError> {
        for path in [&self.encoder, &self.decoder, &self.tokens] {
            let present = std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false);
            if !present {
                return Err(VfError::ModelMissing {
                    expected_path: path.display().to_string(),
                    hint: MODEL_DOWNLOAD_HINT.to_string(),
                });
            }
        }
        Ok(())
    }
}

#[async_trait]
impl TranscriptionProvider for SherpaSttProvider {
    async fn transcribe(&self, wav: &Path) -> Result<String, VfError> {
        self.ensure_model_present()?;

        let encoder = self.encoder.clone();
        let decoder = self.decoder.clone();
        let tokens = self.tokens.clone();
        let language = self.language.clone();
        let wav = wav.to_path_buf();

        // ONNX inference is CPU-heavy and blocking — run it off the async
        // executor so Tauri's runtime is never stalled.
        tokio::task::spawn_blocking(move || run_sherpa(&encoder, &decoder, &tokens, &language, &wav))
            .await
            .map_err(|e| VfError::TranscriptionFailed { detail: format!("join error: {e}") })?
    }
}

#[cfg(feature = "sherpa")]
fn run_sherpa(
    encoder: &Path,
    decoder: &Path,
    tokens: &Path,
    language: &str,
    wav: &Path,
) -> Result<String, VfError> {
    use sherpa_rs::whisper::{WhisperConfig, WhisperRecognizer};

    use crate::audio::wav::{read_wav_as_f32_mono, TARGET_SAMPLE_RATE};

    // Decode the WAV to the f32 mono samples the recognizer expects.
    let (samples, sample_rate) = read_wav_as_f32_mono(wav)?;
    if sample_rate != TARGET_SAMPLE_RATE {
        return Err(VfError::TranscriptionFailed {
            detail: format!("expected {TARGET_SAMPLE_RATE} Hz audio, got {sample_rate} Hz"),
        });
    }

    let config = WhisperConfig {
        decoder: decoder.to_string_lossy().to_string(),
        encoder: encoder.to_string_lossy().to_string(),
        tokens: tokens.to_string_lossy().to_string(),
        language: language.to_string(),
        num_threads: Some(2),
        ..Default::default()
    };

    let mut recognizer = WhisperRecognizer::new(config)
        .map_err(|e| VfError::ModelLoadFailed { detail: e.to_string() })?;

    let result = recognizer.transcribe(sample_rate, samples);
    Ok(result.text.trim().to_string())
}

#[cfg(not(feature = "sherpa"))]
fn run_sherpa(
    _encoder: &Path,
    _decoder: &Path,
    _tokens: &Path,
    _language: &str,
    _wav: &Path,
) -> Result<String, VfError> {
    Err(VfError::SpeechEngineUnavailable {
        detail: "This build was compiled without the `sherpa` feature, so local speech-to-text \
is unavailable. Use the speech-enabled release build (prebuilt binaries, no toolchain) or build \
with `--features sherpa`. Mock transcription keeps working."
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_model_rejects_unknown_id() {
        let err = SherpaSttProvider::from_model(Path::new("."), "nope").unwrap_err();
        assert_eq!(err.code(), "UnknownModel");
    }

    #[test]
    fn from_model_rejects_tts_voice_as_stt() {
        // vits-ljs is a TTS voice, not an STT model.
        let err = SherpaSttProvider::from_model(Path::new("."), "vits-ljs").unwrap_err();
        assert_eq!(err.code(), "UnknownModel");
    }

    #[test]
    fn ensure_model_present_reports_missing_path() {
        let root = std::env::temp_dir().join(format!("vf-stt-{}", uuid::Uuid::new_v4()));
        let p = SherpaSttProvider::from_model(&root, "whisper-tiny-en").unwrap();
        let err = p.ensure_model_present().unwrap_err();
        assert_eq!(err.code(), "ModelMissing");
    }

    #[cfg(not(feature = "sherpa"))]
    #[tokio::test]
    async fn transcribe_without_feature_is_engine_unavailable() {
        // Point at a real (existing) file so we pass the presence check and hit
        // the feature stub. Reuse this source file as a stand-in.
        let root = std::env::temp_dir().join(format!("vf-stt2-{}", uuid::Uuid::new_v4()));
        let entry = models::find("whisper-tiny-en").unwrap();
        let dir = entry.dir(&root);
        std::fs::create_dir_all(&dir).unwrap();
        for f in entry.files {
            std::fs::write(dir.join(f.filename), b"x").unwrap();
        }
        let p = SherpaSttProvider::from_model(&root, "whisper-tiny-en").unwrap();
        let wav = dir.join("in.wav");
        std::fs::write(&wav, b"x").unwrap();
        let err = p.transcribe(&wav).await.unwrap_err();
        assert_eq!(err.code(), "SpeechEngineUnavailable");
        let _ = std::fs::remove_dir_all(&root);
    }
}
