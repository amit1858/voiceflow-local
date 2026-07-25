//! Text-to-speech provider abstraction.
//!
//! Mirrors the transcription layer: all synthesis goes through the
//! [`TtsProvider`] trait so back-ends can be swapped without touching the
//! commands. The default is the model-free [`mock::MockTtsProvider`] (a short
//! beep, so mock mode still validates the Speak flow with zero setup); the real
//! local engine is [`sherpa::SherpaTtsProvider`] (sherpa-onnx VITS — prebuilt
//! binaries, no toolchain).
//!
//! TTS is a **post-preview** action (Speak / auto-speak), never part of
//! `run_pipeline`.

use async_trait::async_trait;
use serde::Serialize;

use crate::errors::VfError;

pub mod mock;
pub mod playback;
pub mod sherpa;

/// A block of synthesized mono audio ready for playback.
#[derive(Debug, Clone)]
pub struct Synthesized {
    /// Mono samples in `[-1.0, 1.0]`.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// A synthesizable voice exposed to the UI voice picker. Field names match the
/// TypeScript `Voice` interface.
#[derive(Debug, Clone, Serialize)]
pub struct Voice {
    pub id: String,
    pub display_name: String,
    /// Whether the voice's model files are present on disk.
    pub installed: bool,
}

/// Turns text into speech audio.
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Synthesize `text` into mono audio samples.
    async fn synthesize(&self, text: &str) -> Result<Synthesized, VfError>;

    /// List the voices this provider can offer (installed or not).
    fn list_voices(&self) -> Vec<Voice>;
}
