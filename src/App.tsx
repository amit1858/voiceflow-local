import { useCallback, useEffect, useRef, useState } from "react";
import { ErrorBanner } from "./components/ErrorBanner";
import { HealthPanel } from "./components/HealthPanel";
import { OutputModePicker } from "./components/OutputModePicker";
import { PreviewPane } from "./components/PreviewPane";
import { PrivacyNote } from "./components/PrivacyNote";
import { RecordingIndicator } from "./components/RecordingIndicator";
import { SettingsPanel } from "./components/SettingsPanel";
import { Toast } from "./components/Toast";
import { useHotkeyStatus } from "./hooks/useHotkeyStatus";
import { useRecorder } from "./hooks/useRecorder";
import {
  clearTempFiles,
  copyToClipboard,
  getSettings,
  onInputLevel,
  runHealthChecks,
  saveSettings,
  speak,
  stopSpeaking,
  onTtsFinished,
  listModels,
  downloadModel,
  onDownloadProgress,
  getCapabilities,
} from "./lib/ipc";
import type {
  Capabilities,
  HealthCheck,
  ModelInfo,
  OutputMode,
  PipelineResult,
  Settings,
  VfError,
} from "./lib/types";
import "./App.css";

type Tab = "main" | "settings" | "health";

export default function App() {
  const [tab, setTab] = useState<Tab>("main");
  const [mode, setMode] = useState<OutputMode>("raw");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [copyError, setCopyError] = useState<VfError | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [inputLevel, setInputLevel] = useState(0);
  const [speaking, setSpeaking] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [downloadLabel, setDownloadLabel] = useState<string | null>(null);
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);

  const [health, setHealth] = useState<HealthCheck[]>([]);
  const [healthLoading, setHealthLoading] = useState(false);
  const [savingSettings, setSavingSettings] = useState(false);

  // Debug/validation aid: track how many times the pipeline produced a result
  // and when, so it's visually clear each recording re-ran the pipeline — even
  // when a provider (e.g. mock) returns identical text every time.
  const [runLabel, setRunLabel] = useState<string | null>(null);
  const runCount = useRef(0);
  const lastLabelled = useRef<PipelineResult | null>(null);

  const recorder = useRecorder();

  // Load settings once on mount and seed the default output mode.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const s = await getSettings();
        if (cancelled) return;
        setSettings(s);
        setMode(s.default_mode);
      } catch (err) {
        if (!cancelled) setCopyError(err as VfError);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Subscribe to the live mic level while the app is open; reset to zero
  // whenever we leave the recording state so the meter settles.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      const u = await onInputLevel((level) => setInputLevel(level));
      if (cancelled) u();
      else unlisten = u;
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    if (recorder.state !== "recording") setInputLevel(0);
  }, [recorder.state]);

  // Reset the Speak/Stop button when playback finishes naturally.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      const u = await onTtsFinished(() => setSpeaking(false));
      if (cancelled) u();
      else unlisten = u;
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  // Load the model/voice list (with installed state) for the Settings download UI.
  const refreshModels = useCallback(async () => {
    try {
      setModels(await listModels());
    } catch {
      // Non-fatal: Settings falls back to showing the raw id.
    }
  }, []);
  const refreshCapabilities = useCallback(async () => {
    try {
      setCapabilities(await getCapabilities());
    } catch {
      // Non-fatal: the capability badge simply won't render.
    }
  }, []);
  useEffect(() => {
    void refreshModels();
    void refreshCapabilities();
  }, [refreshModels, refreshCapabilities]);

  // Live download progress → a compact "<file> NN%" label on the active button.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      const u = await onDownloadProgress((p) => {
        const pct =
          p.total && p.total > 0
            ? ` ${Math.round((p.received / p.total) * 100)}%`
            : "";
        setDownloadLabel(
          `${p.file} (${p.file_index + 1}/${p.file_count})${pct}`,
        );
      });
      if (cancelled) u();
      else unlisten = u;
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const handleDownload = useCallback(
    async (id: string) => {
      setCopyError(null);
      setDownloadingId(id);
      setDownloadLabel("Starting…");
      try {
        await downloadModel(id);
        setToast("Download complete");
        await refreshModels();
        await refreshCapabilities();
      } catch (err) {
        setCopyError(err as VfError);
      } finally {
        setDownloadingId(null);
        setDownloadLabel(null);
      }
    },
    [refreshModels, refreshCapabilities],
  );

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
      setToast("Copied to clipboard");
    } catch (err) {
      setCopyError(err as VfError);
    }
  }, []);

  // Auto-copy after processing (when enabled). Guard against copying the same
  // result object twice.
  const lastAutoCopied = useRef<PipelineResult | null>(null);
  useEffect(() => {
    if (!settings?.auto_copy) return;
    const res = recorder.result;
    if (!res || res === lastAutoCopied.current) return;
    lastAutoCopied.current = res;
    void handleCopy(res.output);
  }, [recorder.result, settings?.auto_copy, handleCopy]);

  // Speak the given text aloud (post-preview action, never in the pipeline).
  const handleSpeak = useCallback(async (text: string) => {
    if (!text.trim()) return;
    setCopyError(null);
    try {
      setSpeaking(true);
      await speak(text);
    } catch (err) {
      setSpeaking(false);
      setCopyError(err as VfError);
    }
  }, []);

  const handleStopSpeaking = useCallback(async () => {
    try {
      await stopSpeaking();
    } catch {
      // Best-effort; ignore stop errors.
    } finally {
      setSpeaking(false);
    }
  }, []);

  // Auto-speak the output after processing when enabled (default off).
  const lastAutoSpoken = useRef<PipelineResult | null>(null);
  useEffect(() => {
    if (!settings?.auto_speak) return;
    const res = recorder.result;
    if (!res || res === lastAutoSpoken.current) return;
    lastAutoSpoken.current = res;
    void handleSpeak(res.output);
  }, [recorder.result, settings?.auto_speak, handleSpeak]);

  // Stamp each new pipeline result with a time + run counter.
  useEffect(() => {
    const res = recorder.result;
    if (!res || res === lastLabelled.current) return;
    lastLabelled.current = res;
    runCount.current += 1;
    const now = new Date().toLocaleTimeString();
    setRunLabel(`Last processed at ${now} · run #${runCount.current}`);
  }, [recorder.result]);

  const handleSaveSettings = useCallback(async (next: Settings) => {
    setSavingSettings(true);
    setCopyError(null);
    try {
      const saved = await saveSettings(next);
      setSettings(saved);
      setToast("Settings saved");
      void refreshCapabilities();
    } catch (err) {
      setCopyError(err as VfError);
    } finally {
      setSavingSettings(false);
    }
  }, [refreshCapabilities]);

  const handleClearTemp = useCallback(async () => {
    setCopyError(null);
    try {
      const removed = await clearTempFiles();
      setToast(`Removed ${removed} temp file${removed === 1 ? "" : "s"}`);
    } catch (err) {
      setCopyError(err as VfError);
    }
  }, []);

  const handleRunHealth = useCallback(async () => {
    setHealthLoading(true);
    setCopyError(null);
    try {
      const checks = await runHealthChecks();
      setHealth(checks);
    } catch (err) {
      setCopyError(err as VfError);
    } finally {
      setHealthLoading(false);
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

  const providerBadge = settings
    ? `${settings.transcription_provider === "mock" ? "Mock STT" : "Sherpa STT"} · ${
        settings.rewrite_provider === "mock" ? "Mock rewrite" : "Foundry Local"
      }`
    : "…";

  return (
    <div className="app">
      <header className="app__header">
        <div>
          <h1 className="app__title">VoiceFlow Local</h1>
          <p className="app__subtitle">
            Local voice → clipboard. Nothing leaves your machine.
          </p>
        </div>
        <div className="app__badges">
          <span className="app__badge" title="Active providers">
            {providerBadge}
          </span>
          {capabilities && (
            <span
              className={
                "app__badge app__badge--cap " +
                (capabilities.speech_engine
                  ? "app__badge--ok"
                  : "app__badge--warn")
              }
              title={
                capabilities.speech_engine
                  ? "This build includes the real sherpa-onnx speech engine (STT + neural TTS)."
                  : "This build has no local speech engine. Mock STT/TTS work; selecting the local engine will report a clear error, not silently fake success."
              }
            >
              {capabilities.speech_engine
                ? "Speech engine: real"
                : "Speech engine: mock-only"}
            </span>
          )}
        </div>
      </header>

      <nav className="tabs" role="tablist">
        {(["main", "settings", "health"] as Tab[]).map((t) => (
          <button
            key={t}
            type="button"
            role="tab"
            aria-selected={tab === t}
            className={"tabs__tab" + (tab === t ? " tabs__tab--active" : "")}
            onClick={() => setTab(t)}
          >
            {t === "main" ? "Record" : t === "settings" ? "Settings" : "Health"}
          </button>
        ))}
      </nav>

      <ErrorBanner error={activeError} onDismiss={dismissError} />
      <PrivacyNote />

      {tab === "main" && (
        <>
          <section className="app__panel">
            <RecordingIndicator
              state={recorder.state}
              hotkey={hotkey}
              level={inputLevel}
              localModels={
                settings?.transcription_provider === "sherpa" ||
                settings?.rewrite_provider === "foundry_local"
              }
            />

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
            <PreviewPane
              result={recorder.result}
              onCopy={handleCopy}
              onSpeak={handleSpeak}
              onStopSpeaking={handleStopSpeaking}
              speaking={speaking}
              onClear={() => {
                recorder.clearResult();
                setRunLabel(null);
              }}
              runLabel={runLabel}
            />
          </section>
        </>
      )}

      {tab === "settings" && (
        <section className="app__panel">
          {settings ? (
            <SettingsPanel
              settings={settings}
              onSave={handleSaveSettings}
              onClearTemp={handleClearTemp}
              saving={savingSettings}
              models={models}
              onDownload={handleDownload}
              downloadingId={downloadingId}
              downloadLabel={downloadLabel}
              mockTranscriptionAvailable={
                capabilities?.mock_transcription_available ?? false
              }
            />
          ) : (
            <p>Loading settings…</p>
          )}
        </section>
      )}

      {tab === "health" && (
        <section className="app__panel">
          <HealthPanel
            checks={health}
            loading={healthLoading}
            onRun={handleRunHealth}
          />
        </section>
      )}

      <footer className="app__footer">
        <span>
          Hotkey: <kbd>{hotkey || "…"}</kbd>
        </span>
        <span>Temp audio is deleted after processing • No history • No cloud</span>
      </footer>

      <Toast message={toast} onDone={() => setToast(null)} />
    </div>
  );
}
