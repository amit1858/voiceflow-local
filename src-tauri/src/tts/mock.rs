//! `MockTtsProvider` — the default, model-free text-to-speech provider.
//!
//! It renders a short, pleasant two-tone beep whose length scales with the text
//! length (bounded), so the Speak / auto-speak flow is fully exercised in mock
//! mode with **zero setup** — no model, no download, no toolchain. It never
//! reads the input text content aloud (there is no real speech model), which
//! keeps the local-first, no-history guarantees intact.

use async_trait::async_trait;

use crate::errors::VfError;
use crate::tts::{Synthesized, TtsProvider, Voice};

/// Sample rate of the mock render.
const MOCK_SAMPLE_RATE: u32 = 22_050;

/// Deterministic mock TTS: emits a short beep instead of real speech.
#[derive(Debug, Default)]
pub struct MockTtsProvider;

impl MockTtsProvider {
    pub fn new() -> Self {
        MockTtsProvider
    }
}

#[async_trait]
impl TtsProvider for MockTtsProvider {
    async fn synthesize(&self, text: &str) -> Result<Synthesized, VfError> {
        // Duration scales gently with text length so longer output "sounds"
        // longer, but is always short and bounded.
        let words = text.split_whitespace().count().max(1);
        let secs = (0.25 + words as f32 * 0.03).min(1.5);
        let n = (MOCK_SAMPLE_RATE as f32 * secs) as usize;

        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / MOCK_SAMPLE_RATE as f32;
            // Two soft sine tones with a short fade in/out to avoid clicks.
            let tone = 0.2
                * ((t * 2.0 * std::f32::consts::PI * 440.0).sin()
                    + 0.5 * (t * 2.0 * std::f32::consts::PI * 660.0).sin());
            let fade = {
                let edge = 0.02_f32;
                let up = (t / edge).clamp(0.0, 1.0);
                let down = ((secs - t) / edge).clamp(0.0, 1.0);
                up.min(down)
            };
            samples.push(tone * fade);
        }

        Ok(Synthesized {
            samples,
            sample_rate: MOCK_SAMPLE_RATE,
        })
    }

    fn list_voices(&self) -> Vec<Voice> {
        vec![Voice {
            id: "mock".to_string(),
            display_name: "Mock beep (no model)".to_string(),
            installed: true,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn produces_nonempty_bounded_audio() {
        let p = MockTtsProvider::new();
        let out = p.synthesize("hello world this is a test").await.unwrap();
        assert_eq!(out.sample_rate, MOCK_SAMPLE_RATE);
        assert!(!out.samples.is_empty());
        // Bounded to <= 1.5s.
        assert!(out.samples.len() <= (MOCK_SAMPLE_RATE as usize * 3) / 2 + 1);
        // Samples stay within range.
        assert!(out.samples.iter().all(|s| s.abs() <= 1.0));
    }

    #[tokio::test]
    async fn empty_text_still_beeps() {
        let p = MockTtsProvider::new();
        let out = p.synthesize("").await.unwrap();
        assert!(!out.samples.is_empty());
    }

    #[test]
    fn lists_a_single_installed_voice() {
        let voices = MockTtsProvider::new().list_voices();
        assert_eq!(voices.len(), 1);
        assert!(voices[0].installed);
    }
}
