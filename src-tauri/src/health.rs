//! Health checks surfaced in the UI health panel.
//!
//! Each check returns a typed pass/fail with a human message so users can see
//! exactly what is (or isn't) ready before switching off mock mode. Checks are
//! best-effort and never panic; a failing check simply reports `Fail`.

use std::path::Path;

use cpal::traits::HostTrait;
use serde::Serialize;

use crate::models::{self, ModelKind};
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
pub async fn run_health_checks(settings: &Settings, models_root: &Path) -> Vec<HealthCheck> {
    let mut checks = Vec::new();

    checks.push(check_temp_writable());
    checks.push(check_microphone());
    checks.extend(check_stt(settings, models_root));
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

/// Speech-engine capability + STT model presence.
fn check_stt(settings: &Settings, models_root: &Path) -> Vec<HealthCheck> {
    let active = settings.transcription_provider == TranscriptionKind::Sherpa;

    // Is the real speech engine compiled into this build?
    let engine = HealthCheck::new(
        "speech_engine",
        "Local speech engine available",
        if speech_engine_available() {
            HealthStatus::Pass
        } else if active {
            HealthStatus::Fail
        } else {
            HealthStatus::Skipped
        },
        if speech_engine_available() {
            "sherpa-onnx is compiled in (prebuilt binaries; no toolchain needed).".to_string()
        } else {
            "This build has no local speech engine. Use the speech-enabled release build or build \
with `--features sherpa`. Mock providers still work."
                .to_string()
        },
    );

    // STT model files present on disk?
    let entry = models::find(&settings.stt_model).filter(|e| e.kind == ModelKind::Stt);
    let present = match entry {
        Some(e) => {
            let ok = e.is_present(models_root);
            HealthCheck::new(
                "stt_model_exists",
                "Speech-to-text model present",
                if ok {
                    HealthStatus::Pass
                } else if active {
                    HealthStatus::Fail
                } else {
                    HealthStatus::Skipped
                },
                if ok {
                    format!("`{}` is installed in {}.", e.display_name, e.dir(models_root).display())
                } else {
                    format!(
                        "`{}` is not downloaded yet. Download it from Settings or run \
scripts/setup-local-models.ps1 (the packaged app also bundles the tiny model).",
                        e.display_name
                    )
                },
            )
        }
        None => HealthCheck::new(
            "stt_model_exists",
            "Speech-to-text model present",
            if active { HealthStatus::Fail } else { HealthStatus::Skipped },
            format!("Unknown STT model id `{}`.", settings.stt_model),
        ),
    };

    vec![engine, present]
}

/// Whether the real sherpa-onnx speech engine is compiled into this build.
pub fn speech_engine_available() -> bool {
    cfg!(feature = "sherpa")
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
