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
    #[error("Microphone permission was denied")]
    MicPermissionDenied,

    #[error("No microphone/input device was found")]
    NoMicrophone,

    #[error("The local Whisper model file is missing")]
    ModelMissing { expected_path: String, hint: String },

    #[error("Failed to load the Whisper model")]
    #[cfg_attr(not(feature = "whisper"), allow(dead_code))]
    ModelLoadFailed { detail: String },

    #[error("Microsoft Foundry Local is not available")]
    FoundryUnavailable { detail: String },

    #[error("Transcription failed")]
    TranscriptionFailed { detail: String },

    #[error("Rewriting failed")]
    RewriteFailed { detail: String },

    #[error("Audio capture failed")]
    AudioCaptureFailed { detail: String },

    #[error("A recording is already in progress")]
    AlreadyRecording,

    #[error("No recording is in progress")]
    NotRecording,

    #[error("Internal error")]
    Internal { detail: String },
}

impl VfError {
    /// Stable machine-readable code shared with the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            VfError::MicPermissionDenied => "MicPermissionDenied",
            VfError::NoMicrophone => "NoMicrophone",
            VfError::ModelMissing { .. } => "ModelMissing",
            VfError::ModelLoadFailed { .. } => "ModelLoadFailed",
            VfError::FoundryUnavailable { .. } => "FoundryUnavailable",
            VfError::TranscriptionFailed { .. } => "TranscriptionFailed",
            VfError::RewriteFailed { .. } => "RewriteFailed",
            VfError::AudioCaptureFailed { .. } => "AudioCaptureFailed",
            VfError::AlreadyRecording => "AlreadyRecording",
            VfError::NotRecording => "NotRecording",
            VfError::Internal { .. } => "Internal",
        }
    }

    /// Optional remediation hint carried to the UI.
    pub fn hint(&self) -> Option<String> {
        match self {
            VfError::ModelMissing { expected_path, hint } => {
                Some(format!("Expected model at: {expected_path}\n{hint}"))
            }
            VfError::FoundryUnavailable { detail } => Some(format!(
                "{detail}\nInstall with `winget install Microsoft.FoundryLocal`, then run `foundry service start` and `foundry model load phi-4-mini-instruct`."
            )),
            VfError::ModelLoadFailed { detail }
            | VfError::TranscriptionFailed { detail }
            | VfError::RewriteFailed { detail }
            | VfError::AudioCaptureFailed { detail }
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
