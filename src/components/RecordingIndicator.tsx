import { useEffect, useState } from "react";
import type { RecordingState } from "../lib/types";

interface Props {
  state: RecordingState;
  hotkey: string;
  /** True when a real local model (Whisper/Foundry) is the active provider. */
  localModels?: boolean;
  /** Live input level in [0, 1] emitted while recording (for the mic meter). */
  level?: number;
}

/**
 * Visible recording indicator. Shows a pulsing red dot while capturing, a
 * spinner with a live elapsed timer while processing, and idle guidance
 * otherwise. The elapsed timer reassures the user that slow local-model
 * inference is still running (not frozen).
 */
export function RecordingIndicator({ state, hotkey, localModels, level = 0 }: Props) {
  const [elapsed, setElapsed] = useState(0);

  // Run a 1s ticker only while processing so the user sees progress.
  useEffect(() => {
    if (state !== "processing") {
      setElapsed(0);
      return;
    }
    const started = Date.now();
    const id = setInterval(() => {
      setElapsed(Math.floor((Date.now() - started) / 1000));
    }, 1000);
    return () => clearInterval(id);
  }, [state]);

  // Scale RMS to a friendlier meter width; speech rarely exceeds ~0.4 RMS.
  const meterPct = Math.min(100, Math.round(Math.min(level / 0.4, 1) * 100));

  return (
    <div className={`indicator indicator--${state}`} role="status" aria-live="polite">
      <span className="indicator__dot" aria-hidden="true" />
      <span className="indicator__label">
        {state === "recording" && (
          <>
            Recording… press hotkey to stop
            <span className="indicator__meter" aria-hidden="true">
              <span className="indicator__meter-fill" style={{ width: `${meterPct}%` }} />
            </span>
          </>
        )}
        {state === "processing" && (
          <>
            Transcribing &amp; rewriting… {elapsed > 0 && <strong>{elapsed}s</strong>}
            {localModels && (
              <span className="indicator__hint">
                {" "}
                Local models can take a while on the first run while they load.
              </span>
            )}
          </>
        )}
        {state === "idle" && (
          <>
            Idle — press <kbd>{hotkey || "…"}</kbd> or the button to start
          </>
        )}
      </span>
    </div>
  );
}
