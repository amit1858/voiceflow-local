//! `MockTranscriptionProvider` — deterministic, model-free transcription.
//!
//! Used by default (mock-first) so the whole workflow runs without a Whisper
//! model installed. It ignores the audio contents and returns a fixed, clean
//! sample transcript, which lets the UI, output modes, and copy flow be
//! validated end-to-end with zero local setup.

use std::path::Path;

use async_trait::async_trait;

use crate::errors::VfError;
use crate::transcription::TranscriptionProvider;

/// A canned transcript that reads like a realistic short voice note.
const MOCK_TRANSCRIPT: &str = "This is a mock transcript from VoiceFlow Local. \
Everything runs on your machine, and no real model is needed in mock mode. \
We should confirm the plan for the next release and share the timeline with the team.";

/// Deterministic transcription provider requiring no local models.
pub struct MockTranscriptionProvider;

impl MockTranscriptionProvider {
    pub fn new() -> Self {
        MockTranscriptionProvider
    }
}

impl Default for MockTranscriptionProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TranscriptionProvider for MockTranscriptionProvider {
    async fn transcribe(&self, wav: &Path) -> Result<String, VfError> {
        // Sanity-check the pipeline actually produced a file, so mock mode still
        // exercises the audio path and surfaces InvalidAudioFile on failure.
        if !wav.exists() {
            return Err(VfError::InvalidAudioFile {
                detail: format!("expected a captured WAV at {}", wav.display()),
            });
        }
        Ok(MOCK_TRANSCRIPT.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn errors_when_audio_missing() {
        let p = MockTranscriptionProvider::new();
        let err = p.transcribe(Path::new("does-not-exist.wav")).await.unwrap_err();
        assert_eq!(err.code(), "InvalidAudioFile");
    }

    #[tokio::test]
    async fn returns_deterministic_text() {
        let p = MockTranscriptionProvider::new();
        // Use this source file as a stand-in "existing" path.
        let here = Path::new(file!());
        if here.exists() {
            let out = p.transcribe(here).await.unwrap();
            assert!(out.starts_with("This is a mock transcript"));
        }
    }
}
