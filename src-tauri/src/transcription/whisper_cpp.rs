//! `LocalWhisperTranscriptionProvider` — in-process transcription via whisper.cpp bindings.
//!
//! The provider loads a local GGML model (e.g. `ggml-base.en.bin`) from the app
//! data directory. If the model file is missing it returns
//! [`VfError::ModelMissing`] with the exact expected path and a download hint —
//! it never panics or crashes the app.
//!
//! whisper.cpp is compiled from C/C++, which requires CMake and a C/C++
//! toolchain at build time. To keep the crate type-checkable on machines
//! without that toolchain, the real implementation is gated behind the default
//! `whisper` Cargo feature; with the feature off, [`transcribe`] returns a
//! typed error explaining the binary was built without Whisper support.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::errors::VfError;
use crate::transcription::TranscriptionProvider;

/// Download hint shown when the model file is absent.
const MODEL_DOWNLOAD_HINT: &str = "Download a GGML English model (e.g. ggml-base.en.bin) from \
https://huggingface.co/ggerganov/whisper.cpp/tree/main and place it at the path above.";

/// Transcription provider backed by whisper.cpp.
pub struct LocalWhisperTranscriptionProvider {
    model_path: PathBuf,
    language: String,
}

impl LocalWhisperTranscriptionProvider {
    /// Create a provider that will load the GGML model at `model_path`.
    pub fn new(model_path: PathBuf) -> Self {
        LocalWhisperTranscriptionProvider { model_path, language: "en".to_string() }
    }

    /// The configured model path (used by health checks).
    #[allow(dead_code)]
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Verify the model file exists, returning a precise [`VfError::ModelMissing`]
    /// otherwise. Called before any (potentially expensive) load attempt.
    pub fn ensure_model_present(&self) -> Result<(), VfError> {
        if self.model_path.exists() {
            Ok(())
        } else {
            Err(VfError::ModelMissing {
                expected_path: self.model_path.display().to_string(),
                hint: MODEL_DOWNLOAD_HINT.to_string(),
            })
        }
    }

    /// Cheap sanity check that the model file looks like a real GGML model
    /// before we hand it to whisper.cpp. Returns [`VfError::ModelInvalid`] for
    /// obviously bad files (empty, truncated download, or a Git-LFS pointer) so
    /// the user gets an actionable message instead of a cryptic native crash.
    #[cfg(feature = "whisper")]
    pub fn validate_model(&self) -> Result<(), VfError> {
        self.ensure_model_present()?;
        let len = std::fs::metadata(&self.model_path)
            .map_err(|e| VfError::ModelInvalid {
                detail: format!(
                    "cannot read model metadata at {}: {e}",
                    self.model_path.display()
                ),
            })?
            .len();
        if len < MIN_PLAUSIBLE_MODEL_BYTES {
            return Err(VfError::ModelInvalid {
                detail: format!(
                    "the model file at {} is only {len} bytes; a valid GGML Whisper model is tens \
of MB. The download is likely incomplete or a Git-LFS pointer file — re-download it.",
                    self.model_path.display()
                ),
            });
        }
        Ok(())
    }
}

/// Minimum plausible size for a real GGML Whisper model (the smallest, `tiny`,
/// is ~75 MB). Anything below this is not a usable model file.
#[cfg(feature = "whisper")]
const MIN_PLAUSIBLE_MODEL_BYTES: u64 = 1_000_000;

#[async_trait]
impl TranscriptionProvider for LocalWhisperTranscriptionProvider {
    async fn transcribe(&self, wav: &Path) -> Result<String, VfError> {
        self.ensure_model_present()?;

        let model_path = self.model_path.clone();
        let language = self.language.clone();
        let wav = wav.to_path_buf();

        // whisper inference is CPU-heavy and blocking — run it off the async
        // executor so we never stall Tauri's runtime.
        tokio::task::spawn_blocking(move || run_whisper(&model_path, &language, &wav))
            .await
            .map_err(|e| VfError::TranscriptionFailed { detail: format!("join error: {e}") })?
    }
}

#[cfg(feature = "whisper")]
fn run_whisper(model_path: &Path, language: &str, wav: &Path) -> Result<String, VfError> {
    use whisper_rs::{
        FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
    };

    use crate::audio::wav::read_wav_as_f32_mono;

    // Reject obviously-bad model files up front (truncated / LFS pointer) so the
    // native loader never sees them.
    LocalWhisperTranscriptionProvider::new(model_path.to_path_buf()).validate_model()?;

    // Decode the WAV to the f32 mono samples whisper expects.
    let (samples, sample_rate) = read_wav_as_f32_mono(wav)?;
    if sample_rate != crate::audio::wav::TARGET_SAMPLE_RATE {
        return Err(VfError::TranscriptionFailed {
            detail: format!(
                "expected {} Hz audio, got {} Hz",
                crate::audio::wav::TARGET_SAMPLE_RATE,
                sample_rate
            ),
        });
    }

    let model_str = model_path.to_string_lossy().to_string();
    let ctx = WhisperContext::new_with_params(&model_str, WhisperContextParameters::default())
        .map_err(|e| VfError::ModelLoadFailed { detail: e.to_string() })?;

    let mut state = ctx
        .create_state()
        .map_err(|e| VfError::ModelLoadFailed { detail: e.to_string() })?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(language));
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state
        .full(params, &samples)
        .map_err(|e| VfError::TranscriptionFailed { detail: e.to_string() })?;

    let num_segments = state
        .full_n_segments()
        .map_err(|e| VfError::TranscriptionFailed { detail: e.to_string() })?;

    let mut text = String::new();
    for i in 0..num_segments {
        let segment = state
            .full_get_segment_text(i)
            .map_err(|e| VfError::TranscriptionFailed { detail: e.to_string() })?;
        text.push_str(&segment);
    }

    Ok(text.trim().to_string())
}

#[cfg(not(feature = "whisper"))]
fn run_whisper(_model_path: &Path, _language: &str, _wav: &Path) -> Result<String, VfError> {
    Err(VfError::TranscriptionFailed {
        detail: "This build was compiled without the `whisper` feature. Rebuild with the \
default features (requires CMake + a C/C++ toolchain) to enable local transcription."
            .to_string(),
    })
}

#[cfg(all(test, feature = "whisper"))]
mod smoke {
    //! Opt-in real-model smoke test. Ignored by default because it needs a
    //! local GGML model and a WAV. Run it explicitly (from `src-tauri`):
    //!
    //! ```powershell
    //! $env:WHISPER_SMOKE_MODEL = "C:\path\to\ggml-tiny.en.bin"
    //! $env:WHISPER_SMOKE_WAV   = "C:\path\to\jfk.wav"   # 16 kHz mono
    //! cargo test smoke -- --ignored --nocapture
    //! ```
    //!
    //! It exercises the full local path: model validation → WAV decode →
    //! whisper.cpp load → inference, and asserts the transcript is non-empty and
    //! contains an expected word from the sample clip.
    use super::*;

    #[test]
    #[ignore = "needs WHISPER_SMOKE_MODEL + WHISPER_SMOKE_WAV pointing at a real model/clip"]
    fn transcribes_known_clip() {
        let model = std::env::var("WHISPER_SMOKE_MODEL")
            .expect("set WHISPER_SMOKE_MODEL to a GGML model path");
        let wav = std::env::var("WHISPER_SMOKE_WAV")
            .expect("set WHISPER_SMOKE_WAV to a 16 kHz mono WAV path");

        let provider = LocalWhisperTranscriptionProvider::new(model.into());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let text = rt
            .block_on(provider.transcribe(Path::new(&wav)))
            .expect("transcription should succeed");

        eprintln!("SMOKE TRANSCRIPT: {text}");
        assert!(!text.trim().is_empty(), "transcript must not be empty");
        let expect = std::env::var("WHISPER_SMOKE_EXPECT").unwrap_or_else(|_| "country".to_string());
        assert!(
            text.to_lowercase().contains(&expect.to_lowercase()),
            "expected transcript to contain {expect:?}, got: {text:?}"
        );
    }
}
