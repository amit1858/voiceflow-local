//! `MockRewriteProvider` — deterministic, model-free rewrite.
//!
//! Used by default (mock-first) so the full workflow runs without Foundry
//! Local. It does NOT call any model; instead it applies deterministic,
//! mode-specific formatting to the input text and relies on the shared
//! [`StyleRules`] post-filter (applied by the pipeline) to enforce the "no
//! kindly" rule. This keeps the mock output shaped like each output mode so the
//! UI and workflow can be validated end-to-end.

use async_trait::async_trait;

use crate::errors::VfError;
use crate::rewrite::{OutputMode, RewriteProvider, StyleRules};

/// Deterministic rewrite provider requiring no local models.
pub struct MockRewriteProvider;

impl MockRewriteProvider {
    pub fn new() -> Self {
        MockRewriteProvider
    }
}

impl Default for MockRewriteProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RewriteProvider for MockRewriteProvider {
    async fn rewrite(
        &self,
        text: &str,
        mode: OutputMode,
        style: &StyleRules,
    ) -> Result<String, VfError> {
        // Apply the style filter to the input up front so the mock body is clean;
        // the pipeline applies it again to the final output (idempotent).
        let body = style.apply(text.trim());

        let out = match mode {
            OutputMode::Raw => body,
            OutputMode::Teams => {
                format!("Hi team,\n\n{body}\n\nCould you take a look and share next steps?")
            }
            OutputMode::Email => format!(
                "Hi,\n\nSharing a quick update. {body}\n\nCould you let me know the next step?\n\nThanks,\n[Your name]"
            ),
            OutputMode::ProductNote => format!(
                "Product note\n\nSummary:\n- {body}\n\nDetails:\n- Captured from a voice note.\n- Action: review and refine."
            ),
            OutputMode::ExecutiveSummary => format!(
                "Executive summary\n\nContext: {body}\nWhy it matters: Impacts current priorities.\nDecision/risk: Needs a quick decision.\nNext step: Confirm the owner and timeline."
            ),
        };

        Ok(format!("{out}\n\n[mock rewrite — enable Foundry Local for model output]"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_is_deterministic_and_mode_shaped() {
        let p = MockRewriteProvider::new();
        let style = StyleRules::default();
        let a = p.rewrite("we should ship on friday", OutputMode::Teams, &style).await.unwrap();
        let b = p.rewrite("we should ship on friday", OutputMode::Teams, &style).await.unwrap();
        assert_eq!(a, b, "mock output must be deterministic");
        assert!(a.starts_with("Hi team"), "Teams mode should be shaped");
    }

    #[tokio::test]
    async fn mock_strips_kindly_via_style() {
        let p = MockRewriteProvider::new();
        let style = StyleRules::default();
        let out = p.rewrite("kindly review the doc", OutputMode::Raw, &style).await.unwrap();
        assert!(!out.to_lowercase().contains("kindly"));
    }
}
