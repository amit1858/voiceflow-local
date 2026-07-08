//! Health checks surfaced in the UI health panel.
//!
//! Each check returns a typed pass/fail with a human message so users can see
//! exactly what is (or isn't) ready before switching off mock mode. Checks are
//! best-effort and never panic; a failing check simply reports `Fail`.

use std::path::Path;

use cpal::traits::HostTrait;
use serde::Serialize;

use crate::rewrite::foundry_local::FoundryLocalRewriteProvider;
use crate::settings::{RewriteKind, Settings, TranscriptionKind};

/// Outcome of a single health check.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Pass,
    Fail,
    /// Not applicable given current settings (e.g. Foundry checks in mock mode).
    Skipped,
}

/// A single health-check result. Field names match `HealthCheck` in
/// `src/lib/types.ts`.
#[derive(Debug, Clone, Serialize)]
pub struct HealthCheck {
    /// Stable identifier the UI can key on.
    pub id: String,
    /// Short human-readable label.
    pub label: String,
    /// Pass / fail / skipped.
    pub status: HealthStatus,
    /// Detail message with remediation where useful.
    pub message: String,
}

impl HealthCheck {
    fn new(id: &str, label: &str, status: HealthStatus, message: impl Into<String>) -> Self {
        HealthCheck {
            id: id.to_string(),
            label: label.to_string(),
            status,
            message: message.into(),
        }
    }
}

/// Run all health checks against the given settings.
pub async fn run_health_checks(settings: &Settings) -> Vec<HealthCheck> {
    let mut checks = Vec::new();

    checks.push(check_temp_writable());
    checks.push(check_microphone());
    checks.extend(check_whisper(settings));
    checks.extend(check_foundry(settings).await);

    checks
}

/// Verify the OS temp dir is writable (where the WAV is captured).
fn check_temp_writable() -> HealthCheck {
    let dir = std::env::temp_dir();
    let probe = dir.join(format!("voiceflow-health-{}.tmp", uuid::Uuid::new_v4()));
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            HealthCheck::new(
                "temp_writable",
                "Temp audio folder writable",
                HealthStatus::Pass,
                format!("{} is writable", dir.display()),
            )
        }
        Err(e) => HealthCheck::new(
            "temp_writable",
            "Temp audio folder writable",
            HealthStatus::Fail,
            format!("Cannot write to {}: {e}", dir.display()),
        ),
    }
}

/// Verify a default input device (microphone) is available.
fn check_microphone() -> HealthCheck {
    let host = cpal::default_host();
    match host.default_input_device() {
        Some(_) => HealthCheck::new(
            "microphone",
            "Microphone available",
            HealthStatus::Pass,
            "A default input device was found",
        ),
        None => HealthCheck::new(
            "microphone",
            "Microphone available",
            HealthStatus::Fail,
            "No microphone/input device was found. Connect a mic and check Windows privacy settings.",
        ),
    }
}

/// Whisper model presence (and load, when the whisper feature is built).
fn check_whisper(settings: &Settings) -> Vec<HealthCheck> {
    let active = settings.transcription_provider == TranscriptionKind::LocalWhisper;
    let path = Path::new(&settings.whisper_model_path);

    let exists = if path.exists() {
        HealthCheck::new(
            "whisper_model_exists",
            "Whisper model exists",
            HealthStatus::Pass,
            format!("Found model at {}", path.display()),
        )
    } else {
        HealthCheck::new(
            "whisper_model_exists",
            "Whisper model exists",
            if active { HealthStatus::Fail } else { HealthStatus::Skipped },
            format!(
                "No model at {}. Download a GGML model (e.g. ggml-base.en.bin) and place it there, \
or keep using the Mock transcription provider.",
                path.display()
            ),
        )
    };

    let load = check_whisper_load(settings, active);

    vec![exists, load]
}

#[cfg(feature = "whisper")]
fn check_whisper_load(settings: &Settings, active: bool) -> HealthCheck {
    use crate::transcription::whisper_cpp::LocalWhisperTranscriptionProvider;

    if !active {
        return HealthCheck::new(
            "whisper_model_loads",
            "Whisper model loads",
            HealthStatus::Skipped,
            "Mock transcription provider active; local Whisper not in use.",
        );
    }

    let provider = LocalWhisperTranscriptionProvider::new(settings.whisper_model_path.clone().into());
    match provider.ensure_model_present() {
        Ok(()) => HealthCheck::new(
            "whisper_model_loads",
            "Whisper model loads",
            HealthStatus::Pass,
            "Model file is present and will be loaded on first use.",
        ),
        Err(e) => HealthCheck::new(
            "whisper_model_loads",
            "Whisper model loads",
            HealthStatus::Fail,
            e.to_string(),
        ),
    }
}

#[cfg(not(feature = "whisper"))]
fn check_whisper_load(_settings: &Settings, _active: bool) -> HealthCheck {
    HealthCheck::new(
        "whisper_model_loads",
        "Whisper model loads",
        HealthStatus::Skipped,
        "This build was compiled without the `whisper` feature; local Whisper is unavailable.",
    )
}

/// Foundry Local: installed, service running, port discovered, Phi available,
/// and a chat-completion smoke test.
async fn check_foundry(settings: &Settings) -> Vec<HealthCheck> {
    let active = settings.rewrite_provider == RewriteKind::FoundryLocal;

    if !active {
        return vec![HealthCheck::new(
            "foundry",
            "Foundry Local",
            HealthStatus::Skipped,
            "Mock rewrite provider active; Foundry Local not in use. Switch the rewrite provider in \
Settings to run these checks.",
        )];
    }

    let provider =
        FoundryLocalRewriteProvider::new(settings.foundry_model.clone(), settings.foundry_endpoint.clone());
    let mut checks = Vec::new();

    // 1. Installed.
    let installed = provider.check_installed().await;
    let installed_ok = installed.is_ok();
    checks.push(match &installed {
        Ok(v) => HealthCheck::new(
            "foundry_installed",
            "Foundry Local installed",
            HealthStatus::Pass,
            format!("`foundry` CLI available ({})", v.trim().lines().next().unwrap_or("").trim()),
        ),
        Err(e) => HealthCheck::new(
            "foundry_installed",
            "Foundry Local installed",
            HealthStatus::Fail,
            e.hint().unwrap_or_else(|| e.to_string()),
        ),
    });

    if !installed_ok {
        // Nothing else can run without the CLI.
        for (id, label) in [
            ("foundry_service", "Foundry service running"),
            ("foundry_port", "Foundry port discovered"),
            ("foundry_phi", "Phi model available"),
            ("foundry_smoke", "Foundry chat-completion smoke test"),
        ] {
            checks.push(HealthCheck::new(
                id,
                label,
                HealthStatus::Skipped,
                "Skipped because Foundry Local is not installed.",
            ));
        }
        return checks;
    }

    // 2 + 3. Service running + port discovered (ensure_ready covers both).
    let endpoint = provider.ensure_ready().await;
    match &endpoint {
        Ok(ep) => {
            checks.push(HealthCheck::new(
                "foundry_service",
                "Foundry service running",
                HealthStatus::Pass,
                "The Foundry Local service responded.",
            ));
            checks.push(HealthCheck::new(
                "foundry_port",
                "Foundry port discovered",
                HealthStatus::Pass,
                format!("Endpoint: {ep}"),
            ));
        }
        Err(e) => {
            checks.push(HealthCheck::new(
                "foundry_service",
                "Foundry service running",
                HealthStatus::Fail,
                e.hint().unwrap_or_else(|| e.to_string()),
            ));
            checks.push(HealthCheck::new(
                "foundry_port",
                "Foundry port discovered",
                HealthStatus::Fail,
                "Could not discover the endpoint because the service is unavailable.",
            ));
        }
    }

    // 4. Phi model available.
    match provider.model_available().await {
        Ok(true) => checks.push(HealthCheck::new(
            "foundry_phi",
            "Phi model available",
            HealthStatus::Pass,
            format!("`{}` is present in Foundry Local.", settings.foundry_model),
        )),
        Ok(false) => checks.push(HealthCheck::new(
            "foundry_phi",
            "Phi model available",
            HealthStatus::Fail,
            format!(
                "`{}` was not found. Run `foundry model download {}`.",
                settings.foundry_model, settings.foundry_model
            ),
        )),
        Err(e) => checks.push(HealthCheck::new(
            "foundry_phi",
            "Phi model available",
            HealthStatus::Fail,
            e.hint().unwrap_or_else(|| e.to_string()),
        )),
    }

    // 5. Smoke test (only if the endpoint was reachable).
    if endpoint.is_ok() {
        match provider.smoke_test().await {
            Ok(()) => checks.push(HealthCheck::new(
                "foundry_smoke",
                "Foundry chat-completion smoke test",
                HealthStatus::Pass,
                "A test chat completion succeeded.",
            )),
            Err(e) => checks.push(HealthCheck::new(
                "foundry_smoke",
                "Foundry chat-completion smoke test",
                HealthStatus::Fail,
                e.hint().unwrap_or_else(|| e.to_string()),
            )),
        }
    } else {
        checks.push(HealthCheck::new(
            "foundry_smoke",
            "Foundry chat-completion smoke test",
            HealthStatus::Skipped,
            "Skipped because the endpoint is not reachable.",
        ));
    }

    checks
}
