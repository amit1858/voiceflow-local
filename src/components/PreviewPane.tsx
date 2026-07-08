import { useState } from "react";
import type { PipelineResult } from "../lib/types";

interface Props {
  result: PipelineResult | null;
  onCopy: (text: string) => Promise<void> | void;
}

/**
 * Preview of the pipeline output with a Copy button. Copy is the ONLY path
 * text reaches the clipboard — there is no auto-send. Optionally the raw
 * transcript can be revealed for comparison.
 */
export function PreviewPane({ result, onCopy }: Props) {
  const [copied, setCopied] = useState(false);
  const [showRaw, setShowRaw] = useState(false);

  if (!result) {
    return (
      <div className="preview preview--empty">
        <p>Your rewritten text will appear here after recording.</p>
      </div>
    );
  }

  const handleCopy = async () => {
    await onCopy(result.output);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };

  const showRawToggle =
    result.mode !== "raw" && result.raw_transcript !== result.output;

  return (
    <div className="preview">
      <div className="preview__header">
        <h2 className="preview__title">Preview</h2>
        <button type="button" className="btn btn--primary" onClick={handleCopy}>
          {copied ? "Copied ✓" : "Copy"}
        </button>
      </div>

      <textarea
        className="preview__text"
        readOnly
        value={result.output}
        aria-label="Output text"
      />

      {showRawToggle && (
        <div className="preview__raw">
          <button
            type="button"
            className="btn btn--link"
            onClick={() => setShowRaw((s) => !s)}
          >
            {showRaw ? "Hide raw transcript" : "Show raw transcript"}
          </button>
          {showRaw && (
            <textarea
              className="preview__text preview__text--raw"
              readOnly
              value={result.raw_transcript}
              aria-label="Raw transcript"
            />
          )}
        </div>
      )}
    </div>
  );
}
