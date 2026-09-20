//! Rewrite provider abstraction + output modes.
//!
//! Rewriting turns a raw transcript into a polished message in one of several
//! [`OutputMode`]s. All rewriting is behind the [`RewriteProvider`] trait so
//! cloud LLMs can be swapped in later. The first implementation is
//! [`foundry_local::FoundryLocalRewriteProvider`] (Microsoft Foundry Local, Phi-4-mini).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::errors::VfError;

pub mod foundry_local;
pub mod mock;
pub mod style;

pub use style::StyleRules;

/// The desired shape of the final output. Serde uses snake_case to match the
/// TypeScript `OutputMode` union in `src/lib/types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    /// Verbatim (cleaned) transcript — bypasses the rewrite provider.
    Raw,
    /// Short, friendly Microsoft Teams chat message.
    Teams,
    /// Structured email with greeting and sign-off.
    Email,
    /// Concise product / engineering note.
    ProductNote,
    /// Tight summary aimed at leadership.
    ExecutiveSummary,
}

impl OutputMode {
    /// Raw mode bypasses the LLM rewrite (style post-filter still applies).
    pub fn is_raw(self) -> bool {
        matches!(self, OutputMode::Raw)
    }

    /// Human-readable label (for logging/telemetry-free diagnostics).
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            OutputMode::Raw => "Raw transcript",
            OutputMode::Teams => "Teams message",
            OutputMode::Email => "Email",
            OutputMode::ProductNote => "Product note",
            OutputMode::ExecutiveSummary => "Executive summary",
        }
    }

    /// Mode-specific instruction injected into the LLM user prompt.
    pub fn instruction(self) -> &'static str {
        match self {
            OutputMode::Raw => {
                "Fix punctuation and obvious speech-to-text errors only. Do NOT add, remove, or \
change meaning. Keep the speaker's own words and order; just make it read cleanly."
            }
            OutputMode::Teams => {
                "Rewrite as a short, crisp Microsoft Teams chat message: conversational but \
professional. Usually open with a brief greeting and end with a clear ask or next step. \
Keep it to a few sentences."
            }
            OutputMode::Email => {
                "Rewrite as a professional email with: a brief greeting, one or two lines of \
context, the main point, a clear ask or next step, and a short closing. Do not invent a \
specific recipient or sender name; use neutral placeholders only if strictly necessary."
            }
            OutputMode::ProductNote => {
                "Rewrite as structured product/engineering notes. Use short headings and bullet \
points where they help. Keep it practical and precise; lead with the key point."
            }
            OutputMode::ExecutiveSummary => {
                "Rewrite as an executive summary for senior leadership, in this order: context, \
the key point, why it matters, any risk or decision needed, and the next step. Crisp and \
free of jargon."
            }
        }
    }
}

/// Turns raw text into a polished message for a given [`OutputMode`].
#[async_trait]
pub trait RewriteProvider: Send + Sync {
    /// Rewrite `text` into `mode`, honoring the shared [`StyleRules`].
    async fn rewrite(
        &self,
        text: &str,
        mode: OutputMode,
        style: &StyleRules,
    ) -> Result<String, VfError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_raw_bypasses_rewrite() {
        assert!(OutputMode::Raw.is_raw());
        for m in [
            OutputMode::Teams,
            OutputMode::Email,
            OutputMode::ProductNote,
            OutputMode::ExecutiveSummary,
        ] {
            assert!(!m.is_raw());
        }
    }

    #[test]
    fn instructions_are_mode_specific_and_nonempty() {
        let all = [
            OutputMode::Raw,
            OutputMode::Teams,
            OutputMode::Email,
            OutputMode::ProductNote,
            OutputMode::ExecutiveSummary,
        ];
        for m in all {
            assert!(!m.instruction().is_empty());
        }
        // Each instruction is distinct.
        let mut seen = std::collections::HashSet::new();
        for m in all {
            assert!(
                seen.insert(m.instruction()),
                "duplicate instruction for {m:?}"
            );
        }
    }

    #[test]
    fn output_mode_serde_roundtrip_snake_case() {
        let json = serde_json::to_string(&OutputMode::ProductNote).unwrap();
        assert_eq!(json, "\"product_note\"");
        let back: OutputMode = serde_json::from_str("\"executive_summary\"").unwrap();
        assert_eq!(back, OutputMode::ExecutiveSummary);
    }
}
