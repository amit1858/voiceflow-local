//! Typed error domain for VoiceFlow Local.
//!
//! Every fallible operation surfaces a [`VfError`]. Errors serialize to a
//! stable `{ code, message, hint }` shape so the React layer can render a
//! friendly banner with remediation text (see `src/lib/types.ts`).

use serde::Serialize;

/// All error conditions the app can produce. The `code` is a stable string the
/// frontend switches on; `message` is human-readable; `hint` carries optional
/// remediation detail (e.g. an exact model path or a shell command).
#[derive(Debug, Clone, thiserror::Error)]
pub enum VfError {
    // ---- Audio ----------------------------------------------------------
    #[error("Microphone permission was denied")]
    MicPermissionDenied,

    #[error("No microphone/input device was found")]
    NoMicrophone,

    #[error("Failed to start audio capture")]
    AudioStartFailed { detail: String },

    #[error("Failed to stop audio capture")]
    AudioStopFailed { detail: String },

    #[error("Audio capture failed")]
    AudioCaptureFailed { detail: String },

    #[error("The captured audio file is missing or invalid")]
    InvalidAudioFile { detail: String },

    #[error("Failed to clean up temporary audio files")]
    TempCleanupFailed { detail: String },

    // ---- Whisper / transcription ---------------------------------------
    #[error("The local Whisper model file is missing")]
    ModelMissing { expected_path: String, hint: String },

    #[error("The Whisper model file is invalid or unreadable")]
    #[cfg_attr(not(feature = "whisper"), allow(dead_code))]
    ModelInvalid { detail: String },

    #[error("Failed to load the Whisper model")]
    #[cfg_attr(not(feature = "whisper"), allow(dead_code))]
    ModelLoadFailed { detail: String },

    #[error("Transcription failed")]
    TranscriptionFailed { detail: String },

    // ---- Foundry Local / rewrite ---------------------------------------
    #[error("Microsoft Foundry Local is not installed")]
    FoundryNotInstalled,

    #[error("The Foundry Local service is not running")]
    FoundryServiceNotRunning { detail: String },

    #[error("Could not discover the Foundry Local endpoint port")]
    FoundryPortNotDiscovered { detail: String },

    #[error("The Phi model is not installed in Foundry Local")]
    PhiNotInstalled { model: String },

    #[error("Foundry Local did not respond")]
    FoundryNoResponse { detail: String },

    #[error("Foundry Local timed out")]
    FoundryTimeout { detail: String },

    #[error("Rewriting failed")]
    RewriteFailed { detail: String },

    // ---- App / state ----------------------------------------------------
    #[error("A recording is already in progress")]
    AlreadyRecording,

    #[error("No recording is in progress")]
    NotRecording,

    #[error("Failed to read or write settings")]
    SettingsError { detail: String },

    #[error("Internal error")]
    Internal { detail: String },
}

impl VfError {
    /// Stable machine-readable code shared with the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            VfError::MicPermissionDenied => "MicPermissionDenied",
            VfError::NoMicrophone => "NoMicrophone",
            VfError::AudioStartFailed { .. } => "AudioStartFailed",
            VfError::AudioStopFailed { .. } => "AudioStopFailed",
            VfError::AudioCaptureFailed { .. } => "AudioCaptureFailed",
            VfError::InvalidAudioFile { .. } => "InvalidAudioFile",
            VfError::TempCleanupFailed { .. } => "TempCleanupFailed",
            VfError::ModelMissing { .. } => "ModelMissing",
            VfError::ModelInvalid { .. } => "ModelInvalid",
            VfError::ModelLoadFailed { .. } => "ModelLoadFailed",
            VfError::TranscriptionFailed { .. } => "TranscriptionFailed",
            VfError::FoundryNotInstalled => "FoundryNotInstalled",
            VfError::FoundryServiceNotRunning { .. } => "FoundryServiceNotRunning",
            VfError::FoundryPortNotDiscovered { .. } => "FoundryPortNotDiscovered",
            VfError::PhiNotInstalled { .. } => "PhiNotInstalled",
            VfError::FoundryNoResponse { .. } => "FoundryNoResponse",
            VfError::FoundryTimeout { .. } => "FoundryTimeout",
            VfError::RewriteFailed { .. } => "RewriteFailed",
            VfError::AlreadyRecording => "AlreadyRecording",
            VfError::NotRecording => "NotRecording",
            VfError::SettingsError { .. } => "SettingsError",
            VfError::Internal { .. } => "Internal",
        }
    }

    /// Optional remediation hint carried to the UI.
    pub fn hint(&self) -> Option<String> {
        match self {
            VfError::ModelMissing { expected_path, hint } => {
                Some(format!("Expected model at: {expected_path}\n{hint}"))
            }
            VfError::FoundryNotInstalled => Some(
                "Install with `winget install Microsoft.FoundryLocal`, then run \
`foundry service start` and `foundry model load phi-4-mini-instruct`. You can keep using \
Mock mode in Settings until it is ready."
                    .to_string(),
            ),
            VfError::FoundryServiceNotRunning { .. } => {
                Some("Run `foundry service start` in a terminal, then retry.".to_string())
            }
            VfError::FoundryPortNotDiscovered { .. } => Some(
                "Could not parse the endpoint from `foundry service status`. Restart the service \
or set a manual endpoint override in Settings."
                    .to_string(),
            ),
            VfError::PhiNotInstalled { model } => Some(format!(
                "Run `foundry model download {model}` then `foundry model load {model}`."
            )),
            VfError::FoundryTimeout { .. } => {
                Some("The model may still be loading. Wait a moment and try again.".to_string())
            }
            VfError::ModelInvalid { detail }
            | VfError::ModelLoadFailed { detail }
            | VfError::TranscriptionFailed { detail }
            | VfError::RewriteFailed { detail }
            | VfError::AudioStartFailed { detail }
            | VfError::AudioStopFailed { detail }
            | VfError::AudioCaptureFailed { detail }
            | VfError::InvalidAudioFile { detail }
            | VfError::TempCleanupFailed { detail }
            | VfError::FoundryNoResponse { detail }
            | VfError::SettingsError { detail }
            | VfError::Internal { detail } => {
                if detail.is_empty() {
                    None
                } else {
                    Some(detail.clone())
                }
            }
            _ => None,
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        VfError::Internal { detail: detail.into() }
    }
}

/// Wire representation sent to the frontend via `Result<_, VfError>` command
/// returns. Tauri serializes the `Err` variant with this shape.
#[derive(Serialize)]
struct VfErrorWire {
    code: &'static str,
    message: String,
    hint: Option<String>,
}

impl Serialize for VfError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        VfErrorWire {
            code: self.code(),
            message: self.to_string(),
            hint: self.hint(),
        }
        .serialize(serializer)
    }
}

#[allow(dead_code)]
pub type VfResult<T> = Result<T, VfError>;
