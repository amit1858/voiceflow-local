//! `WhisperCppProvider` — in-process transcription via whisper.cpp bindings.
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
pub struct WhisperCppProvider {
    model_path: PathBuf,
    language: String,
}

impl WhisperCppProvider {
    /// Create a provider that will load the GGML model at `model_path`.
    pub fn new(model_path: PathBuf) -> Self {
        WhisperCppProvider { model_path, language: "en".to_string() }
    }

    /// Verify the model file exists, returning a precise [`VfError::ModelMissing`]
    /// otherwise. Called before any (potentially expensive) load attempt.
    fn ensure_model_present(&self) -> Result<(), VfError> {
        if self.model_path.exists() {
            Ok(())
        } else {
            Err(VfError::ModelMissing {
                expected_path: self.model_path.display().to_string(),
                hint: MODEL_DOWNLOAD_HINT.to_string(),
            })
        }
    }
}

#[async_trait]
impl TranscriptionProvider for WhisperCppProvider {
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
