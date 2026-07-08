//! The end-to-end processing pipeline: transcribe → rewrite → style filter.
//!
//! This is provider-agnostic: it takes trait objects so the same flow works
//! for any [`TranscriptionProvider`] / [`RewriteProvider`] combination.

use std::path::Path;

use serde::Serialize;

use crate::errors::VfError;
use crate::rewrite::{OutputMode, RewriteProvider, StyleRules};
use crate::transcription::TranscriptionProvider;

/// Result returned to the frontend. Field names match `PipelineResult` in
/// `src/lib/types.ts`.
#[derive(Debug, Clone, Serialize)]
pub struct PipelineResult {
    /// The whisper transcript after the style post-filter.
    pub raw_transcript: String,
    /// The final text for `mode` after the style post-filter.
    pub output: String,
    /// The mode that produced `output`.
    pub mode: OutputMode,
}

/// Run the full pipeline for one recording.
///
/// 1. Transcribe the WAV to text.
/// 2. Apply the style post-filter to the raw transcript.
/// 3. For non-Raw modes, rewrite via the LLM then re-apply the style filter.
///
/// The style filter runs on *every* path (including Raw) so forbidden words can
/// never reach the clipboard.
pub async fn run_pipeline(
    wav: &Path,
    mode: OutputMode,
    transcriber: &dyn TranscriptionProvider,
    rewriter: &dyn RewriteProvider,
    style: &StyleRules,
) -> Result<PipelineResult, VfError> {
    let raw = transcriber.transcribe(wav).await?;
    let raw_styled = style.apply(&raw);

    if raw_styled.is_empty() {
        return Err(VfError::TranscriptionFailed {
            detail: "The transcript was empty. Try speaking closer to the mic.".into(),
        });
    }

    let output = if mode.is_raw() {
        raw_styled.clone()
    } else {
        let rewritten = rewriter.rewrite(&raw_styled, mode, style).await?;
        style.apply(&rewritten)
    };

    Ok(PipelineResult { raw_transcript: raw_styled, output, mode })
}
