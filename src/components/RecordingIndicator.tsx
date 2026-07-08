import type { RecordingState } from "../lib/types";

interface Props {
  state: RecordingState;
  hotkey: string;
}

/**
 * Visible recording indicator. Shows a pulsing red dot while capturing,
 * a spinner while processing, and idle guidance otherwise.
 */
export function RecordingIndicator({ state, hotkey }: Props) {
  return (
    <div className={`indicator indicator--${state}`} role="status" aria-live="polite">
      <span className="indicator__dot" aria-hidden="true" />
      <span className="indicator__label">
        {state === "recording" && "Recording… press hotkey to stop"}
        {state === "processing" && "Transcribing & rewriting…"}
        {state === "idle" && (
          <>
            Idle — press <kbd>{hotkey || "…"}</kbd> or the button to start
          </>
        )}
      </span>
    </div>
  );
}
