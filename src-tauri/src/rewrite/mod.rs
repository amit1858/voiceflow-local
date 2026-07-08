//! Rewrite provider abstraction + output modes.
//!
//! Rewriting turns a raw transcript into a polished message in one of several
//! [`OutputMode`]s. All rewriting is behind the [`RewriteProvider`] trait so
//! cloud LLMs can be swapped in later. The first implementation is
//! [`foundry_local::FoundryLocalProvider`] (Microsoft Foundry Local, Phi-4-mini).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::errors::VfError;

pub mod foundry_local;
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
                "Return the text unchanged except for light cleanup of filler words."
            }
            OutputMode::Teams => {
                "Rewrite the transcript as a short, friendly Microsoft Teams chat message. \
Keep it to a few sentences, conversational but professional. No greeting or sign-off."
            }
            OutputMode::Email => {
                "Rewrite the transcript as a clear, professional email. Include a brief greeting, \
well-structured body paragraphs, and a short sign-off. Do not invent a specific recipient \
name or sender name; use neutral placeholders only if strictly necessary."
            }
            OutputMode::ProductNote => {
                "Rewrite the transcript as a concise product/engineering note. Lead with the key \
point, then supporting detail as tight bullet points where helpful. Objective and precise."
            }
            OutputMode::ExecutiveSummary => {
                "Rewrite the transcript as a tight executive summary for senior leadership. \
Start with the single most important takeaway, then 2-4 crisp supporting points. \
Avoid jargon; focus on impact and decisions."
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
