//! Transcription provider abstraction.
//!
//! All transcription goes through the [`TranscriptionProvider`] trait so that
//! additional back-ends (cloud STT, other local engines) can be added later
//! without touching the pipeline. The first implementation is
//! [`whisper_cpp::WhisperCppProvider`].

use std::path::Path;

use async_trait::async_trait;

use crate::errors::VfError;

pub mod whisper_cpp;

/// Turns a 16 kHz mono WAV file into text.
#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    /// Transcribe the audio at `wav` (16 kHz mono PCM) into a plain string.
    async fn transcribe(&self, wav: &Path) -> Result<String, VfError>;
}
