//! Transcription provider abstraction.
//!
//! All transcription goes through the [`TranscriptionProvider`] trait so that
//! additional back-ends (cloud STT, other local engines) can be added later
//! without touching the pipeline. The default is the model-free
//! [`mock::MockTranscriptionProvider`]; the real local engine is
//! [`sherpa::SherpaSttProvider`] (sherpa-onnx, prebuilt binaries — no toolchain).

use std::path::Path;

use async_trait::async_trait;

use crate::errors::VfError;

pub mod mock;
pub mod sherpa;

/// Turns a 16 kHz mono WAV file into text.
#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    /// Transcribe the audio at `wav` (16 kHz mono PCM) into a plain string.
    async fn transcribe(&self, wav: &Path) -> Result<String, VfError>;
}
