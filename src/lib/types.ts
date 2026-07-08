// Shared types mirrored from the Rust backend (src-tauri/src).
// Keep these in sync with `errors.rs`, `rewrite/mod.rs` and `pipeline.rs`.

/** Output rewrite modes. Must match Rust `OutputMode` serde rename values. */
export type OutputMode =
  | "raw"
  | "teams"
  | "email"
  | "product_note"
  | "executive_summary";

export interface OutputModeOption {
  value: OutputMode;
  label: string;
  description: string;
}

export const OUTPUT_MODES: OutputModeOption[] = [
  {
    value: "raw",
    label: "Raw transcript",
    description: "Cleaned transcript, no rewrite.",
  },
  {
    value: "teams",
    label: "Teams message",
    description: "Short, friendly chat message.",
  },
  {
    value: "email",
    label: "Email",
    description: "Structured email with greeting and sign-off.",
  },
  {
    value: "product_note",
    label: "Product note",
    description: "Concise product/engineering note.",
  },
  {
    value: "executive_summary",
    label: "Executive summary",
    description: "Tight summary for leadership.",
  },
];

/** Stable error codes emitted by the Rust `VfError` enum (serde tag = "code"). */
export type VfErrorCode =
  | "MicPermissionDenied"
  | "NoMicrophone"
  | "AudioStartFailed"
  | "AudioStopFailed"
  | "AudioCaptureFailed"
  | "InvalidAudioFile"
  | "TempCleanupFailed"
  | "ModelMissing"
  | "ModelInvalid"
  | "ModelLoadFailed"
  | "TranscriptionFailed"
  | "FoundryNotInstalled"
  | "FoundryServiceNotRunning"
  | "FoundryPortNotDiscovered"
  | "PhiNotInstalled"
  | "FoundryNoResponse"
  | "FoundryTimeout"
  | "RewriteFailed"
  | "AlreadyRecording"
  | "NotRecording"
  | "SettingsError"
  | "Internal";

/** Serialized form of a Rust `VfError`. */
export interface VfError {
  code: VfErrorCode;
  message: string;
  /** Optional remediation hint (e.g. expected model path / download command). */
  hint?: string | null;
}

/** Result returned from the full transcribe + rewrite pipeline. */
export interface PipelineResult {
  /** The raw whisper transcript (style-filtered). */
  raw_transcript: string;
  /** The final text for the selected output mode (style-filtered). */
  output: string;
  /** Which mode produced `output`. */
  mode: OutputMode;
}

export type RecordingState = "idle" | "recording" | "processing";

/** Transcription provider selector. Matches Rust `TranscriptionKind`. */
export type TranscriptionKind = "mock" | "local_whisper";
/** Rewrite provider selector. Matches Rust `RewriteKind`. */
export type RewriteKind = "mock" | "foundry_local";

/** User settings. Field names match Rust `Settings` (snake_case). */
export interface Settings {
  hotkey: string;
  default_mode: OutputMode;
  transcription_provider: TranscriptionKind;
  rewrite_provider: RewriteKind;
  whisper_model_path: string;
  foundry_model: string;
  foundry_endpoint: string | null;
  auto_copy: boolean;
}

/** Outcome of a single health check. Matches Rust `HealthStatus`. */
export type HealthStatus = "pass" | "fail" | "skipped";

/** A single health-check result. Matches Rust `HealthCheck`. */
export interface HealthCheck {
  id: string;
  label: string;
  status: HealthStatus;
  message: string;
}

/** Type guard so React can render VfError banners for command failures. */
export function isVfError(value: unknown): value is VfError {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "message" in value
  );
}

/** Friendly, human-readable remediation per error code (UI fallback). */
export const ERROR_REMEDIATION: Record<VfErrorCode, string> = {
  MicPermissionDenied:
    "Grant microphone access to VoiceFlow Local in Windows Settings → Privacy → Microphone.",
  NoMicrophone:
    "No microphone was found. Connect a mic or check your input device.",
  AudioStartFailed:
    "Recording could not start. Make sure the mic isn't in use by another app and try again.",
  AudioStopFailed:
    "Recording could not stop cleanly. Try recording again.",
  AudioCaptureFailed:
    "Audio capture failed. Check that your microphone is working and not in use by another app.",
  InvalidAudioFile:
    "The captured audio was missing or invalid. Record again and speak close to the mic.",
  TempCleanupFailed:
    "Temporary audio files could not be removed. You can retry from Settings → Clear temp files.",
  ModelMissing:
    "The local Whisper model file is missing. See the hint for the expected path and download instructions.",
  ModelInvalid:
    "The Whisper model file is invalid or unreadable. Re-download a supported GGML model.",
  ModelLoadFailed:
    "The Whisper model could not be loaded. It may be corrupted — re-download it.",
  TranscriptionFailed: "Transcription failed. Try recording again.",
  FoundryNotInstalled:
    "Microsoft Foundry Local is not installed. Install it with `winget install Microsoft.FoundryLocal`, then run `foundry service start`. You can keep using Mock mode meanwhile.",
  FoundryServiceNotRunning:
    "The Foundry Local service is not running. Run `foundry service start` and retry.",
  FoundryPortNotDiscovered:
    "Could not discover the Foundry Local endpoint. Restart the service or set a manual endpoint override in Settings.",
  PhiNotInstalled:
    "The Phi model isn't installed in Foundry Local. Run `foundry model download phi-4-mini-instruct` then `foundry model load phi-4-mini-instruct`.",
  FoundryNoResponse:
    "Foundry Local did not respond. Make sure the service is running and reachable.",
  FoundryTimeout:
    "Foundry Local timed out. The model may still be loading — wait a moment and try again.",
  RewriteFailed:
    "Rewriting failed. The Foundry model may be busy or unloaded — try again or reload it.",
  AlreadyRecording: "A recording is already in progress.",
  NotRecording: "No recording is in progress.",
  SettingsError: "Settings could not be read or saved.",
  Internal: "An unexpected error occurred.",
};
