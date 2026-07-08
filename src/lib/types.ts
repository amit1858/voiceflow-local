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
  | "ModelMissing"
  | "ModelLoadFailed"
  | "FoundryUnavailable"
  | "TranscriptionFailed"
  | "RewriteFailed"
  | "AudioCaptureFailed"
  | "AlreadyRecording"
  | "NotRecording"
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
  ModelMissing:
    "The local Whisper model file is missing. See the hint for the expected path and download instructions.",
  ModelLoadFailed:
    "The Whisper model could not be loaded. It may be corrupted — re-download it.",
  FoundryUnavailable:
    "Microsoft Foundry Local is not available. Install it with `winget install Microsoft.FoundryLocal`, then run `foundry service start`.",
  TranscriptionFailed: "Transcription failed. Try recording again.",
  RewriteFailed:
    "Rewriting failed. The Foundry model may be busy or unloaded — try again or reload it.",
  AudioCaptureFailed:
    "Audio capture failed. Check that your microphone is working and not in use by another app.",
  AlreadyRecording: "A recording is already in progress.",
  NotRecording: "No recording is in progress.",
  Internal: "An unexpected error occurred.",
};
