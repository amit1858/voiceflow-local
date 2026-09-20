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
use crate::settings::{RewriteKind, Settings, TranscriptionKind, TtsKind};

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
    checks.extend(check_stt(settings, models_root).await);
    checks.extend(check_tts(settings, models_root).await);
    checks.extend(check_foundry(settings).await);

    checks
}

/// Verify the OS temp dir is writable (where the WAV is captured).
fn check_temp_writable() -> HealthCheck {
    let dir = crate::audio::temp::temp_dir();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        return HealthCheck::new(
            "temp_writable",
            "Private temp audio folder writable",
            HealthStatus::Fail,
            format!("Cannot create {}: {error}", dir.display()),
        );
    }
    let probe = dir.join(format!("voiceflow-health-{}.tmp", uuid::Uuid::new_v4()));
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            match crate::audio::temp::cleanup_stale() {
                Ok(report) if report.failures.is_empty() => HealthCheck::new(
                    "temp_writable",
                    "Private temp audio folder writable",
                    HealthStatus::Pass,
                    format!(
                        "{} is writable; removed {} stale VoiceFlow WAV(s)",
                        dir.display(),
                        report.removed
                    ),
                ),
                Ok(report) => HealthCheck::new(
                    "temp_writable",
                    "Private temp audio folder writable",
                    HealthStatus::Fail,
                    format!(
                        "{} stale cleanup(s) failed: {}",
                        report.failures.len(),
                        report.failures.join("; ")
                    ),
                ),
                Err(error) => HealthCheck::new(
                    "temp_writable",
                    "Private temp audio folder writable",
                    HealthStatus::Fail,
                    error.to_string(),
                ),
            }
        }
        Err(e) => HealthCheck::new(
            "temp_writable",
            "Private temp audio folder writable",
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
async fn check_stt(settings: &Settings, models_root: &Path) -> Vec<HealthCheck> {
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

    // STT model files present and checksum-verified?
    let entry = models::find(&settings.stt_model).filter(|e| e.kind == ModelKind::Stt);
    let (present, verified) = match entry {
        Some(e) => {
            let exists = e.is_present(models_root);
            let integrity = e.verify(models_root);
            let present = HealthCheck::new(
                "stt_model_exists",
                "Speech-to-text model present",
                status_for(active, exists),
                if exists {
                    format!(
                        "`{}` is installed in {}.",
                        e.display_name,
                        e.dir(models_root).display()
                    )
                } else {
                    format!(
                        "`{}` is not downloaded yet. Download it from Settings or run \
scripts/setup-local-models.ps1 (the packaged app also bundles the tiny model).",
                        e.display_name
                    )
                },
            );
            let verified = match integrity {
                Ok(()) => HealthCheck::new(
                    "stt_model_verified",
                    "Speech-to-text model hash verified",
                    HealthStatus::Pass,
                    format!(
                        "`{}` matches revision {} and all pinned SHA-256 values.",
                        e.display_name, e.revision
                    ),
                ),
                Err(error) => HealthCheck::new(
                    "stt_model_verified",
                    "Speech-to-text model hash verified",
                    status_for(active, false),
                    error.hint().unwrap_or_else(|| error.to_string()),
                ),
            };
            (present, verified)
        }
        None => (
            HealthCheck::new(
                "stt_model_exists",
                "Speech-to-text model present",
                status_for(active, false),
                format!("Unknown STT model id `{}`.", settings.stt_model),
            ),
            HealthCheck::new(
                "stt_model_verified",
                "Speech-to-text model hash verified",
                status_for(active, false),
                "Cannot verify an unknown model.",
            ),
        ),
    };

    let ready = if !active {
        HealthCheck::new(
            "stt_engine_load",
            "Speech-to-text engine loads model",
            HealthStatus::Skipped,
            "Developer mock transcription is active.",
        )
    } else if !speech_engine_available() {
        HealthCheck::new(
            "stt_engine_load",
            "Speech-to-text engine loads model",
            HealthStatus::Fail,
            "The consumer speech engine is not compiled into this build.",
        )
    } else if let Some(entry) = entry {
        if entry.verify(models_root).is_err() {
            HealthCheck::new(
                "stt_engine_load",
                "Speech-to-text engine loads model",
                HealthStatus::Skipped,
                "Skipped until the selected model passes checksum verification.",
            )
        } else {
            match crate::transcription::sherpa::SherpaSttProvider::from_model(
                models_root,
                &settings.stt_model,
            ) {
                Ok(provider) => match provider.check_ready().await {
                    Ok(()) => HealthCheck::new(
                        "stt_engine_load",
                        "Speech-to-text engine loads model",
                        HealthStatus::Pass,
                        "The selected ONNX model loaded successfully.",
                    ),
                    Err(error) => HealthCheck::new(
                        "stt_engine_load",
                        "Speech-to-text engine loads model",
                        HealthStatus::Fail,
                        error.hint().unwrap_or_else(|| error.to_string()),
                    ),
                },
                Err(error) => HealthCheck::new(
                    "stt_engine_load",
                    "Speech-to-text engine loads model",
                    HealthStatus::Fail,
                    error.hint().unwrap_or_else(|| error.to_string()),
                ),
            }
        }
    } else {
        HealthCheck::new(
            "stt_engine_load",
            "Speech-to-text engine loads model",
            HealthStatus::Fail,
            "Unknown selected STT model.",
        )
    };

    vec![engine, present, verified, ready]
}

fn status_for(active: bool, ready: bool) -> HealthStatus {
    if ready {
        HealthStatus::Pass
    } else if active {
        HealthStatus::Fail
    } else {
        HealthStatus::Skipped
    }
}

/// Audio-output device availability + TTS voice presence (and, when the engine
/// is compiled in and the voice is present, a tiny synthesis smoke test).
async fn check_tts(settings: &Settings, models_root: &Path) -> Vec<HealthCheck> {
    let active = settings.tts_provider == TtsKind::Sherpa;

    // Output device (needed for any playback, mock or real).
    let output = HealthCheck::new(
        "audio_output",
        "Audio output device available",
        if audio_output_available() {
            HealthStatus::Pass
        } else {
            HealthStatus::Fail
        },
        if audio_output_available() {
            "A default output device was found for speech playback.".to_string()
        } else {
            "No audio output device was found. Connect speakers/headphones to hear spoken output."
                .to_string()
        },
    );

    // TTS voice files present on disk?
    let entry = models::find(&settings.tts_voice).filter(|e| e.kind == ModelKind::Tts);
    let voice_present = entry
        .map(|e| e.verify(models_root).is_ok())
        .unwrap_or(false);
    let voice = match entry {
        Some(e) => HealthCheck::new(
            "tts_voice_exists",
            "Text-to-speech voice present",
            if voice_present {
                HealthStatus::Pass
            } else if active {
                HealthStatus::Fail
            } else {
                HealthStatus::Skipped
            },
            if voice_present {
                format!(
                    "`{}` is installed and checksum-verified in {}.",
                    e.display_name,
                    e.dir(models_root).display()
                )
            } else {
                format!(
                    "`{}` is missing or failed checksum verification. Download it from Settings or \
run scripts/setup-local-models.ps1.",
                    e.display_name
                )
            },
        ),
        None => HealthCheck::new(
            "tts_voice_exists",
            "Text-to-speech voice present",
            if active {
                HealthStatus::Fail
            } else {
                HealthStatus::Skipped
            },
            format!("Unknown TTS voice id `{}`.", settings.tts_voice),
        ),
    };

    // Synthesis smoke test: only meaningful when the engine is compiled in, the
    // provider is active, and the voice is present.
    let synth = if !active {
        HealthCheck::new(
            "tts_synth",
            "Text-to-speech synthesis",
            HealthStatus::Skipped,
            "Mock TTS active (a beep, no model). Switch the TTS provider to the local neural engine \
to run this check.",
        )
    } else if !speech_engine_available() {
        HealthCheck::new(
            "tts_synth",
            "Text-to-speech synthesis",
            HealthStatus::Fail,
            "This build has no local speech engine, so neural TTS is unavailable. Use the \
speech-enabled release build or build with `--features sherpa`.",
        )
    } else if !voice_present {
        HealthCheck::new(
            "tts_synth",
            "Text-to-speech synthesis",
            HealthStatus::Skipped,
            "Skipped because the selected voice is not installed yet.",
        )
    } else {
        // Engine + voice present: attempt a tiny synthesis to prove it loads.
        match tiny_tts_smoke(models_root, &settings.tts_voice).await {
            Ok(()) => HealthCheck::new(
                "tts_synth",
                "Text-to-speech synthesis",
                HealthStatus::Pass,
                "Synthesized a short sample successfully.",
            ),
            Err(e) => HealthCheck::new(
                "tts_synth",
                "Text-to-speech synthesis",
                HealthStatus::Fail,
                e.hint().unwrap_or_else(|| e.to_string()),
            ),
        }
    };

    vec![output, voice, synth]
}

/// Synthesize a tiny sample (no playback) to verify the voice actually loads.
/// A no-op stub when the `sherpa` feature is disabled.
#[cfg(feature = "sherpa")]
async fn tiny_tts_smoke(models_root: &Path, voice_id: &str) -> Result<(), crate::errors::VfError> {
    use crate::tts::sherpa::SherpaTtsProvider;
    use crate::tts::TtsProvider;

    let provider = SherpaTtsProvider::from_voice(models_root, voice_id)?;
    let out = provider.synthesize("test").await?;
    if out.samples.is_empty() {
        return Err(crate::errors::VfError::TtsSynthFailed {
            detail: "the voice produced no audio".to_string(),
        });
    }
    Ok(())
}

#[cfg(not(feature = "sherpa"))]
async fn tiny_tts_smoke(
    _models_root: &Path,
    _voice_id: &str,
) -> Result<(), crate::errors::VfError> {
    Err(crate::errors::VfError::SpeechEngineUnavailable {
        detail: "built without the `sherpa` feature".to_string(),
    })
}

/// Whether the real sherpa-onnx speech engine is compiled into this build.
pub fn speech_engine_available() -> bool {
    cfg!(feature = "sherpa")
}

/// Whether a default audio **output** device is available (for TTS playback).
pub fn audio_output_available() -> bool {
    cpal::default_host().default_output_device().is_some()
}

/// A compact snapshot of the running build's speech capabilities, surfaced as a
/// UI capability badge so selecting an unavailable provider is never a silent
/// trap. Field names match the TypeScript `Capabilities` interface.
#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    /// True when the running build has the real sherpa-onnx speech engine.
    pub speech_engine: bool,
    /// True when the selected STT model's files are present on disk.
    pub stt_model_installed: bool,
    /// True only when all selected STT files match their pinned SHA-256 values.
    pub stt_model_verified: bool,
    /// Debug builds expose mock STT; consumer builds do not.
    pub mock_transcription_available: bool,
    /// True when the selected TTS voice's files are present on disk.
    pub tts_voice_installed: bool,
    /// True when a default audio output device exists.
    pub audio_output_available: bool,
}

/// Compute the capability snapshot for the given settings.
pub fn capabilities(settings: &Settings, models_root: &Path) -> Capabilities {
    let stt_installed = models::find(&settings.stt_model)
        .filter(|e| e.kind == ModelKind::Stt)
        .map(|e| e.is_present(models_root))
        .unwrap_or(false);
    let tts_installed = models::find(&settings.tts_voice)
        .filter(|e| e.kind == ModelKind::Tts)
        .map(|e| e.is_present(models_root))
        .unwrap_or(false);

    Capabilities {
        speech_engine: speech_engine_available(),
        stt_model_installed: stt_installed,
        stt_model_verified: models::find(&settings.stt_model)
            .filter(|e| e.kind == ModelKind::Stt)
            .map(|e| e.verify(models_root).is_ok())
            .unwrap_or(false),
        mock_transcription_available: crate::settings::Settings::runtime_mode()
            .mock_transcription_allowed(),
        tts_voice_installed: tts_installed,
        audio_output_available: audio_output_available(),
    }
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn readiness_status_never_passes_only_because_provider_is_inactive() {
            assert!(matches!(status_for(true, true), HealthStatus::Pass));
            assert!(matches!(status_for(true, false), HealthStatus::Fail));
            assert!(matches!(status_for(false, false), HealthStatus::Skipped));
            assert!(matches!(status_for(false, true), HealthStatus::Pass));
        }
    }

    let provider = FoundryLocalRewriteProvider::new(
        settings.foundry_model.clone(),
        settings.foundry_endpoint.clone(),
    );
    let mut checks = Vec::new();

    // 1. Installed.
    let installed = provider.check_installed().await;
    let installed_ok = installed.is_ok();
    checks.push(match &installed {
        Ok(v) => HealthCheck::new(
            "foundry_installed",
            "Foundry Local installed",
            HealthStatus::Pass,
            format!(
                "`foundry` CLI available ({})",
                v.trim().lines().next().unwrap_or("").trim()
            ),
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
