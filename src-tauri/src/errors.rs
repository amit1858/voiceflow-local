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

    #[error("The recording was too short")]
    RecordingTooShort { detail: String },

    #[error("No speech was detected")]
    NoSpeechDetected { detail: String },

    // ---- Speech engine / transcription ---------------------------------
    #[error("The local speech engine is not available in this build")]
    SpeechEngineUnavailable { detail: String },

    #[error("The local speech model file is missing")]
    ModelMissing { expected_path: String, hint: String },

    #[error("The speech model file is invalid or unreadable")]
    #[cfg_attr(not(feature = "sherpa"), allow(dead_code))]
    ModelInvalid { detail: String },

    #[error("Failed to load the speech model")]
    #[cfg_attr(not(feature = "sherpa"), allow(dead_code))]
    ModelLoadFailed { detail: String },

    #[error("Transcription failed")]
    TranscriptionFailed { detail: String },

    // ---- Model manager (download / verify) -----------------------------
    #[error("Unknown model id")]
    UnknownModel { id: String },

    #[error("Downloading the model failed")]
    ModelDownloadFailed { detail: String },

    #[error("The downloaded model failed checksum verification")]
    ModelChecksumMismatch { detail: String },

    // ---- Text to speech -------------------------------------------------
    #[error("The selected voice model is missing")]
    TtsVoiceMissing { expected_path: String, hint: String },

    #[error("Speech synthesis failed")]
    TtsSynthFailed { detail: String },

    #[error("Could not play the synthesized audio")]
    TtsPlaybackFailed { detail: String },

    #[error("No audio output device was found")]
    NoAudioOutputDevice,

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
            VfError::RecordingTooShort { .. } => "RecordingTooShort",
            VfError::NoSpeechDetected { .. } => "NoSpeechDetected",
            VfError::SpeechEngineUnavailable { .. } => "SpeechEngineUnavailable",
            VfError::ModelMissing { .. } => "ModelMissing",
            VfError::ModelInvalid { .. } => "ModelInvalid",
            VfError::ModelLoadFailed { .. } => "ModelLoadFailed",
            VfError::TranscriptionFailed { .. } => "TranscriptionFailed",
            VfError::UnknownModel { .. } => "UnknownModel",
            VfError::ModelDownloadFailed { .. } => "ModelDownloadFailed",
            VfError::ModelChecksumMismatch { .. } => "ModelChecksumMismatch",
            VfError::TtsVoiceMissing { .. } => "TtsVoiceMissing",
            VfError::TtsSynthFailed { .. } => "TtsSynthFailed",
            VfError::TtsPlaybackFailed { .. } => "TtsPlaybackFailed",
            VfError::NoAudioOutputDevice => "NoAudioOutputDevice",
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
            VfError::TtsVoiceMissing { expected_path, hint } => {
                Some(format!("Expected voice at: {expected_path}\n{hint}"))
            }
            VfError::SpeechEngineUnavailable { .. } => Some(
                "This build was compiled without the local speech engine. Use the \
speech-enabled release build (which ships the prebuilt sherpa-onnx binaries), or build from \
source with `--features sherpa`. Mock providers keep working in any build."
                    .to_string(),
            ),
            VfError::NoAudioOutputDevice => Some(
                "Connect speakers or headphones and check the Windows output device, then retry."
                    .to_string(),
            ),
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
            | VfError::RecordingTooShort { detail }
            | VfError::NoSpeechDetected { detail }
            | VfError::ModelDownloadFailed { detail }
            | VfError::ModelChecksumMismatch { detail }
            | VfError::TtsSynthFailed { detail }
            | VfError::TtsPlaybackFailed { detail }
            | VfError::FoundryNoResponse { detail }
            | VfError::SettingsError { detail }
            | VfError::Internal { detail } => {
                if detail.is_empty() {
                    None
                } else {
                    Some(detail.clone())
                }
            }
            VfError::UnknownModel { id } => Some(format!("No model registered with id `{id}`.")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(VfError::NoMicrophone.code(), "NoMicrophone");
        assert_eq!(VfError::FoundryNotInstalled.code(), "FoundryNotInstalled");
        assert_eq!(
            VfError::PhiNotInstalled { model: "phi-4-mini-instruct".into() }.code(),
            "PhiNotInstalled"
        );
        assert_eq!(
            VfError::InvalidAudioFile { detail: "x".into() }.code(),
            "InvalidAudioFile"
        );
    }

    #[test]
    fn model_missing_hint_includes_path() {
        let e = VfError::ModelMissing {
            expected_path: "C:/models/ggml-base.en.bin".into(),
            hint: "download it".into(),
        };
        let hint = e.hint().unwrap();
        assert!(hint.contains("C:/models/ggml-base.en.bin"));
        assert!(hint.contains("download it"));
    }

    #[test]
    fn foundry_not_installed_hint_mentions_winget() {
        let hint = VfError::FoundryNotInstalled.hint().unwrap();
        assert!(hint.to_lowercase().contains("winget"));
    }

    #[test]
    fn serializes_to_code_message_hint() {
        let e = VfError::PhiNotInstalled { model: "phi-4-mini-instruct".into() };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "PhiNotInstalled");
        assert!(json["message"].is_string());
        assert!(json["hint"].is_string());
    }

    #[test]
    fn hint_is_null_when_absent() {
        let e = VfError::NotRecording;
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "NotRecording");
        assert!(json["hint"].is_null());
    }

    #[test]
    fn tts_voice_missing_hint_includes_path_and_serializes() {
        let e = VfError::TtsVoiceMissing {
            expected_path: "C:/models/tts/vits-ljs".into(),
            hint: "download the voice".into(),
        };
        assert_eq!(e.code(), "TtsVoiceMissing");
        let hint = e.hint().unwrap();
        assert!(hint.contains("C:/models/tts/vits-ljs"));
        assert!(hint.contains("download the voice"));
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "TtsVoiceMissing");
        assert!(json["hint"].is_string());
    }

    #[test]
    fn speech_engine_unavailable_hint_mentions_sherpa_feature() {
        let hint = VfError::SpeechEngineUnavailable { detail: String::new() }
            .hint()
            .unwrap();
        assert!(hint.contains("--features sherpa"));
        assert!(hint.to_lowercase().contains("mock"));
    }

    #[test]
    fn no_audio_output_device_has_code_and_hint() {
        let e = VfError::NoAudioOutputDevice;
        assert_eq!(e.code(), "NoAudioOutputDevice");
        assert!(e.hint().unwrap().to_lowercase().contains("output device"));
    }

    #[test]
    fn tts_synth_and_playback_carry_detail_hint() {
        let synth = VfError::TtsSynthFailed { detail: "bad voice graph".into() };
        assert_eq!(synth.code(), "TtsSynthFailed");
        assert_eq!(synth.hint().unwrap(), "bad voice graph");

        let play = VfError::TtsPlaybackFailed { detail: "no sink".into() };
        assert_eq!(play.code(), "TtsPlaybackFailed");
        assert_eq!(play.hint().unwrap(), "no sink");
    }

    #[test]
    fn download_and_checksum_errors_map_cleanly() {
        let dl = VfError::ModelDownloadFailed { detail: "HTTP 404".into() };
        assert_eq!(dl.code(), "ModelDownloadFailed");
        assert_eq!(dl.hint().unwrap(), "HTTP 404");

        let sum = VfError::ModelChecksumMismatch { detail: "expected a, got b".into() };
        let json = serde_json::to_value(&sum).unwrap();
        assert_eq!(json["code"], "ModelChecksumMismatch");
        assert_eq!(json["hint"], "expected a, got b");
    }

    #[test]
    fn unknown_model_hint_names_the_id() {
        let e = VfError::UnknownModel { id: "does-not-exist".into() };
        assert_eq!(e.code(), "UnknownModel");
        assert!(e.hint().unwrap().contains("does-not-exist"));
    }

    #[test]
    fn audio_guard_errors_have_stable_codes() {
        assert_eq!(
            VfError::RecordingTooShort { detail: "0.1s".into() }.code(),
            "RecordingTooShort"
        );
        assert_eq!(
            VfError::NoSpeechDetected { detail: "rms 0.0001".into() }.code(),
            "NoSpeechDetected"
        );
    }
}
