//! `FoundryLocalProvider` — local LLM rewrite via Microsoft Foundry Local.
//!
//! Integration approach: a thin **CLI bridge + local REST** client.
//!
//! * The `foundry` CLI manages the service and models. We ensure the service is
//!   running (`foundry service start`) and the model is present/loaded
//!   (`foundry model download` / `foundry model load`).
//! * Foundry Local serves an **OpenAI-compatible** REST API on a **dynamic**
//!   localhost port, so we never hardcode it — we discover the endpoint by
//!   parsing `foundry service status`.
//! * Inference is a `POST {endpoint}/v1/chat/completions` with model
//!   `phi-4-mini-instruct`.
//!
//! Every failure maps to a typed [`VfError`] (`FoundryUnavailable` /
//! `RewriteFailed`); the app never panics if Foundry is missing.

use std::io::ErrorKind;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::errors::VfError;
use crate::rewrite::{OutputMode, RewriteProvider, StyleRules};

/// Default Foundry Local model id.
const DEFAULT_MODEL: &str = "phi-4-mini-instruct";

/// Local LLM rewrite provider backed by Foundry Local.
pub struct FoundryLocalProvider {
    model: String,
    client: reqwest::Client,
    /// Cached discovered endpoint (e.g. `http://127.0.0.1:5273`). Lazily filled.
    endpoint: Mutex<Option<String>>,
}

impl FoundryLocalProvider {
    pub fn new() -> Self {
        FoundryLocalProvider {
            model: DEFAULT_MODEL.to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            endpoint: Mutex::new(None),
        }
    }

    /// Ensure the service is up and the model is available, returning the
    /// discovered REST endpoint. Results are cached after first success.
    async fn ensure_ready(&self) -> Result<String, VfError> {
        if let Some(ep) = self.endpoint.lock().ok().and_then(|g| g.clone()) {
            return Ok(ep);
        }

        // 1. Try to discover an already-running service.
        let mut endpoint = self.discover_endpoint().await?;

        // 2. If not running, start it and retry discovery.
        if endpoint.is_none() {
            self.run_foundry(&["service", "start"]).await?;
            endpoint = self.discover_endpoint().await?;
        }

        let endpoint = endpoint.ok_or_else(|| VfError::FoundryUnavailable {
            detail: "Could not determine the Foundry Local endpoint from `foundry service status`."
                .into(),
        })?;

        // 3. Best-effort ensure the model is downloaded and loaded. These are
        //    idempotent; failures here surface as RewriteFailed at inference.
        let _ = self.run_foundry(&["model", "download", &self.model]).await;
        let _ = self.run_foundry(&["model", "load", &self.model]).await;

        if let Ok(mut guard) = self.endpoint.lock() {
            *guard = Some(endpoint.clone());
        }
        Ok(endpoint)
    }

    /// Run `foundry service status` and parse out the REST endpoint URL.
    async fn discover_endpoint(&self) -> Result<Option<String>, VfError> {
        let output = self.run_foundry(&["service", "status"]).await?;
        Ok(parse_endpoint(&output))
    }

    /// Invoke the `foundry` CLI. Maps a missing executable to
    /// [`VfError::FoundryUnavailable`] so the UI can guide installation.
    async fn run_foundry(&self, args: &[&str]) -> Result<String, VfError> {
        let result = Command::new("foundry").args(args).output().await;

        match result {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                // `service status` returns useful text even on non-zero exit;
                // return combined output and let callers parse/decide.
                Ok(format!("{stdout}\n{stderr}"))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Err(VfError::FoundryUnavailable {
                detail: "The `foundry` CLI was not found on PATH.".into(),
            }),
            Err(e) => Err(VfError::FoundryUnavailable {
                detail: format!("Failed to run `foundry {}`: {e}", args.join(" ")),
            }),
        }
    }

    fn build_messages(&self, text: &str, mode: OutputMode, style: &StyleRules) -> Vec<ChatMessage> {
        let system = format!(
            "{}\n\nTask: {}",
            style.system_prompt(),
            mode.instruction()
        );
        vec![
            ChatMessage { role: "system".into(), content: system },
            ChatMessage {
                role: "user".into(),
                content: format!("Transcript:\n\n{text}"),
            },
        ]
    }
}

impl Default for FoundryLocalProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RewriteProvider for FoundryLocalProvider {
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
            .map_err(|e| VfError::RewriteFailed {
                detail: format!("request to {url} failed: {e}"),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
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
            // Drop any trailing path (keep scheme://host:port only).
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

    #[test]
    fn parses_dynamic_port_endpoint() {
        let sample = "Model management service is running on http://127.0.0.1:5273/openai/status";
        assert_eq!(
            parse_endpoint(sample),
            Some("http://127.0.0.1:5273".to_string())
        );
    }

    #[test]
    fn returns_none_when_absent() {
        assert_eq!(parse_endpoint("service is not running"), None);
    }
}
