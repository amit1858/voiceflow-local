import { useEffect, useState } from "react";
import type { PipelineResult } from "../lib/types";

interface Props {
  result: PipelineResult | null;
  onCopy: (text: string) => Promise<void> | void;
  onClear: () => void;
  /**
   * Optional debug/validation aid: a label like "Last processed at 11:04:07 PM
   * · run #3" shown above the preview so it's visually obvious the pipeline
   * re-ran, even when a provider (e.g. mock) returns identical text each time.
   */
  runLabel?: string | null;
}

/**
 * Editable preview of the pipeline output with Copy and Clear buttons. The
 * textarea is user-editable BEFORE copy — Copy is the only path text reaches
 * the clipboard (no auto-send). Editing the text never mutates the underlying
 * transcript; a new recording result resets the editable buffer.
 */
export function PreviewPane({ result, onCopy, onClear, runLabel }: Props) {
  const [text, setText] = useState("");
  const [copied, setCopied] = useState(false);
  const [showRaw, setShowRaw] = useState(false);

  // Reset the editable buffer whenever a new result arrives.
  useEffect(() => {
    setText(result?.output ?? "");
    setShowRaw(false);
  }, [result]);

  if (!result) {
    return (
      <div className="preview preview--empty">
        <p>
          Your rewritten text will appear here after recording. You can edit it
          before copying.
        </p>
      </div>
    );
  }

  const handleCopy = async () => {
    await onCopy(text);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };

  const handleClear = () => {
    setText("");
    onClear();
  };

  const showRawToggle =
    result.mode !== "raw" && result.raw_transcript !== result.output;

  return (
    <div className="preview">
      <div className="preview__header">
        <h2 className="preview__title">Preview</h2>
        <div className="preview__actions">
          <button
            type="button"
            className="btn btn--primary"
            onClick={handleCopy}
            disabled={text.trim().length === 0}
          >
            {copied ? "Copied ✓" : "Copy"}
          </button>
          <button type="button" className="btn btn--ghost" onClick={handleClear}>
            Clear
          </button>
        </div>
      </div>

      <textarea
        className="preview__text"
        value={text}
        onChange={(e) => setText(e.target.value)}
        aria-label="Output text (editable)"
        spellCheck
      />

      {runLabel && (
        <p className="preview__run" aria-live="polite">
          {runLabel}
        </p>
      )}

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
