//! `FoundryLocalRewriteProvider` — local LLM rewrite via Microsoft Foundry Local.
//!
//! Integration approach: a thin **CLI bridge + local REST** client.
//!
//! * The `foundry` CLI manages the service and models. We ensure the service is
//!   running (`foundry service start`) and the model is present/loaded
//!   (`foundry model download` / `foundry model load`).
//! * Foundry Local serves an **OpenAI-compatible** REST API on a **dynamic**
//!   localhost port, so we never hardcode it — we discover the endpoint by
//!   parsing `foundry service status` (unless a manual override is configured
//!   in Settings for debugging).
//! * Inference is a `POST {endpoint}/v1/chat/completions` with the configured
//!   model (default `phi-4-mini-instruct`).
//!
//! Every failure maps to a specific [`VfError`] so the UI can guide the user.

use std::io::ErrorKind;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::errors::VfError;
use crate::rewrite::{OutputMode, RewriteProvider, StyleRules};

/// Local LLM rewrite provider backed by Foundry Local.
pub struct FoundryLocalRewriteProvider {
    model: String,
    /// Optional manual endpoint override (skips dynamic port discovery).
    endpoint_override: Option<String>,
    client: reqwest::Client,
    /// Cached discovered endpoint. Lazily filled from `foundry service status`.
    endpoint: Mutex<Option<String>>,
}

impl FoundryLocalRewriteProvider {
    /// Create a provider for `model`, optionally forcing `endpoint_override`.
    pub fn new(model: String, endpoint_override: Option<String>) -> Self {
        FoundryLocalRewriteProvider {
            model,
            endpoint_override,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            endpoint: Mutex::new(None),
        }
    }

    #[allow(dead_code)]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Ensure the service is up and return the REST endpoint. Honors the manual
    /// override; otherwise discovers (and starts, if needed) the service.
    pub async fn ensure_ready(&self) -> Result<String, VfError> {
        if let Some(ep) = &self.endpoint_override {
            if !ep.trim().is_empty() {
                return Ok(ep.trim().trim_end_matches('/').to_string());
            }
        }

        if let Some(ep) = self.endpoint.lock().ok().and_then(|g| g.clone()) {
            return Ok(ep);
        }

        // Try an already-running service first.
        let mut endpoint = self.discover_endpoint().await?;

        // If not running, start it and retry discovery.
        if endpoint.is_none() {
            self.run_foundry(&["service", "start"]).await?;
            endpoint = self.discover_endpoint().await?;
        }

        let endpoint = endpoint.ok_or_else(|| VfError::FoundryPortNotDiscovered {
            detail: "no http(s) endpoint found in `foundry service status` output".into(),
        })?;

        // Best-effort ensure the model is present and loaded (idempotent).
        let _ = self.run_foundry(&["model", "download", &self.model]).await;
        let _ = self.run_foundry(&["model", "load", &self.model]).await;

        if let Ok(mut guard) = self.endpoint.lock() {
            *guard = Some(endpoint.clone());
        }
        Ok(endpoint)
    }

    /// Run `foundry service status` and parse out the REST endpoint URL.
    pub async fn discover_endpoint(&self) -> Result<Option<String>, VfError> {
        let output = self.run_foundry(&["service", "status"]).await?;
        Ok(parse_endpoint(&output))
    }

    /// Verify the `foundry` CLI is installed by running `foundry --version`.
    pub async fn check_installed(&self) -> Result<String, VfError> {
        self.run_foundry(&["--version"]).await
    }

    /// True if the configured model appears in `foundry model list`.
    pub async fn model_available(&self) -> Result<bool, VfError> {
        let list = self.run_foundry(&["model", "list"]).await?;
        Ok(list.to_lowercase().contains(&self.model.to_lowercase()))
    }

    /// Fire a tiny chat completion to confirm end-to-end inference works.
    pub async fn smoke_test(&self) -> Result<(), VfError> {
        let endpoint = self.ensure_ready().await?;
        let url = format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'));
        let body = ChatRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "Reply with the single word: ok".into(),
            }],
            temperature: 0.0,
            stream: false,
        };
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&url, e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
            Err(VfError::RewriteFailed {
                detail: format!("smoke test returned {status}: {detail}"),
            })
        }
    }

    /// Invoke the `foundry` CLI. Maps a missing executable to
    /// [`VfError::FoundryNotInstalled`].
    async fn run_foundry(&self, args: &[&str]) -> Result<String, VfError> {
        let result = Command::new("foundry").args(args).output().await;
        match result {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                Ok(format!("{stdout}\n{stderr}"))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Err(VfError::FoundryNotInstalled),
            Err(e) => Err(VfError::FoundryServiceNotRunning {
                detail: format!("failed to run `foundry {}`: {e}", args.join(" ")),
            }),
        }
    }

    fn build_messages(&self, text: &str, mode: OutputMode, style: &StyleRules) -> Vec<ChatMessage> {
        let system = format!("{}\n\nTask: {}", style.system_prompt(), mode.instruction());
        vec![
            ChatMessage { role: "system".into(), content: system },
            ChatMessage { role: "user".into(), content: format!("Transcript:\n\n{text}") },
        ]
    }
}

#[async_trait]
impl RewriteProvider for FoundryLocalRewriteProvider {
    async fn rewrite(
        &self,
        text: &str,
        mode: OutputMode,
        style: &StyleRules,
    ) -> Result<String, VfError> {
        let endpoint = self.ensure_ready().await?;
        let url = format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'));

        let body = ChatRequest {
            model: self.model.clone(),
            messages: self.build_messages(text, mode, style),
            temperature: 0.3,
            stream: false,
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&url, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
            // A 404 for the model usually means it isn't loaded.
            if status.as_u16() == 404 && detail.to_lowercase().contains("model") {
                return Err(VfError::PhiNotInstalled { model: self.model.clone() });
            }
            return Err(VfError::RewriteFailed {
                detail: format!("Foundry returned {status}: {detail}"),
            });
        }

        let parsed: ChatResponse = resp.json().await.map_err(|e| VfError::RewriteFailed {
            detail: format!("could not parse Foundry response: {e}"),
        })?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();

        if content.trim().is_empty() {
            return Err(VfError::RewriteFailed {
                detail: "Foundry returned an empty completion.".into(),
            });
        }

        Ok(content.trim().to_string())
    }
}

/// Map a reqwest error to the most specific [`VfError`].
fn map_reqwest_error(url: &str, e: reqwest::Error) -> VfError {
    if e.is_timeout() {
        VfError::FoundryTimeout { detail: format!("request to {url} timed out") }
    } else if e.is_connect() {
        VfError::FoundryNoResponse {
            detail: format!("could not connect to {url}: {e}"),
        }
    } else {
        VfError::RewriteFailed { detail: format!("request to {url} failed: {e}") }
    }
}

/// Parse a `http://host:port` endpoint out of arbitrary CLI text.
///
/// Foundry's status output includes a line with the served endpoint; we scan
/// for the first `http://` or `https://` token and trim trailing path/punctuation.
fn parse_endpoint(text: &str) -> Option<String> {
    for scheme in ["http://", "https://"] {
        if let Some(start) = text.find(scheme) {
            let rest = &text[start..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
                .unwrap_or(rest.len());
            let mut url = rest[..end].trim_end_matches('/').to_string();
            if let Some(idx) = url[scheme.len()..].find('/') {
                url.truncate(scheme.len() + idx);
            }
            if url.len() > scheme.len() {
                return Some(url);
            }
        }
    }
    None
}

// ---- OpenAI-compatible wire types -----------------------------------------

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[cfg(test)]
mod tests {
    use super::parse_endpoint;
    use super::FoundryLocalRewriteProvider;
    use crate::rewrite::{OutputMode, StyleRules};

    #[test]
    fn parses_dynamic_port_endpoint() {
        let sample = "Model management service is running on http://127.0.0.1:5273/openai/status";
        assert_eq!(parse_endpoint(sample), Some("http://127.0.0.1:5273".to_string()));
    }

    #[test]
    fn returns_none_when_absent() {
        assert_eq!(parse_endpoint("service is not running"), None);
    }

    #[test]
    fn build_messages_injects_style_and_mode_instruction() {
        let provider =
            FoundryLocalRewriteProvider::new("phi-4-mini-instruct".into(), None);
        let style = StyleRules::default();
        let msgs = provider.build_messages("we should ship friday", OutputMode::Teams, &style);

        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[1].role, "user");

        // System prompt carries the style rules (incl. the "kindly" ban) and the
        // mode-specific instruction.
        assert!(msgs[0].content.to_lowercase().contains("kindly"));
        assert!(msgs[0].content.contains(OutputMode::Teams.instruction()));

        // User message carries the transcript.
        assert!(msgs[1].content.contains("we should ship friday"));
    }

    #[test]
    fn each_mode_builds_a_distinct_instruction() {
        let provider =
            FoundryLocalRewriteProvider::new("phi-4-mini-instruct".into(), None);
        let style = StyleRules::default();
        let email = provider.build_messages("x", OutputMode::Email, &style)[0]
            .content
            .clone();
        let exec = provider.build_messages("x", OutputMode::ExecutiveSummary, &style)[0]
            .content
            .clone();
        assert_ne!(email, exec);
    }
}
