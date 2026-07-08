import { useCallback, useState } from "react";
import { ErrorBanner } from "./components/ErrorBanner";
import { OutputModePicker } from "./components/OutputModePicker";
import { PreviewPane } from "./components/PreviewPane";
import { RecordingIndicator } from "./components/RecordingIndicator";
import { useHotkeyStatus } from "./hooks/useHotkeyStatus";
import { useRecorder } from "./hooks/useRecorder";
import { copyToClipboard } from "./lib/ipc";
import type { OutputMode, VfError } from "./lib/types";
import "./App.css";

export default function App() {
  const [mode, setMode] = useState<OutputMode>("raw");
  const [copyError, setCopyError] = useState<VfError | null>(null);

  const recorder = useRecorder();

  // The hotkey handler must always use the latest `mode`; the hook keeps the
  // handler in a ref so this closure stays current without re-subscribing.
  const handleToggle = useCallback(() => {
    void recorder.toggle(mode);
  }, [recorder, mode]);

  const { hotkey } = useHotkeyStatus(handleToggle);

  const handleCopy = useCallback(async (text: string) => {
    setCopyError(null);
    try {
      await copyToClipboard(text);
    } catch (err) {
      setCopyError(err as VfError);
    }
  }, []);

  const activeError = recorder.error ?? copyError;
  const dismissError = () => {
    recorder.clearError();
    setCopyError(null);
  };

  const isBusy = recorder.state !== "idle";
  const buttonLabel =
    recorder.state === "recording"
      ? "Stop & process"
      : recorder.state === "processing"
        ? "Working…"
        : "Start recording";

  return (
    <div className="app">
      <header className="app__header">
        <h1 className="app__title">VoiceFlow Local</h1>
        <p className="app__subtitle">
          Local voice → clipboard. Nothing leaves your machine.
        </p>
      </header>

      <ErrorBanner error={activeError} onDismiss={dismissError} />

      <section className="app__panel">
        <RecordingIndicator state={recorder.state} hotkey={hotkey} />

        <div className="app__controls">
          <button
            type="button"
            className={
              "btn btn--record" +
              (recorder.state === "recording" ? " btn--record-active" : "")
            }
            disabled={recorder.state === "processing"}
            onClick={() => void recorder.toggle(mode)}
          >
            {buttonLabel}
          </button>
          {recorder.state === "recording" && (
            <button
              type="button"
              className="btn btn--ghost"
              onClick={() => void recorder.cancel()}
            >
              Cancel
            </button>
          )}
        </div>
      </section>

      <section className="app__panel">
        <h2 className="app__panel-title">Output mode</h2>
        <OutputModePicker value={mode} onChange={setMode} disabled={isBusy} />
      </section>

      <section className="app__panel">
        <PreviewPane result={recorder.result} onCopy={handleCopy} />
      </section>

      <footer className="app__footer">
        <span>Hotkey: <kbd>{hotkey || "…"}</kbd></span>
        <span>Temp audio is deleted after processing • No history • No cloud</span>
      </footer>
    </div>
  );
}
