import { useMemo, useState } from "react";
import {
  OUTPUT_MODES,
  type ModelInfo,
  type RewriteKind,
  type Settings,
  type TranscriptionKind,
  type TtsKind,
} from "../lib/types";

interface Props {
  settings: Settings;
  onSave: (next: Settings) => Promise<void> | void;
  onClearTemp: () => Promise<void> | void;
  saving?: boolean;
  /** Registered models/voices with installed state (for the download buttons). */
  models?: ModelInfo[];
  /** Download the model/voice with the given registry id. */
  onDownload?: (id: string) => Promise<void> | void;
  /** Registry id currently downloading (disables its button). */
  downloadingId?: string | null;
  /** Human progress label for the active download, e.g. "tiny.en-encoder 40%". */
  downloadLabel?: string | null;
}

/** Settings form: providers, hotkey, defaults, model paths, auto-copy, and a
 * "Clear temp files" action. Edits a local draft, then persists on Save. */
export function SettingsPanel({
  settings,
  onSave,
  onClearTemp,
  saving,
  models = [],
  onDownload,
  downloadingId,
  downloadLabel,
}: Props) {
  const [draft, setDraft] = useState<Settings>(settings);

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setDraft((d) => ({ ...d, [key]: value }));

  const sttModels = useMemo(() => models.filter((m) => m.kind === "stt"), [models]);
  const ttsVoices = useMemo(() => models.filter((m) => m.kind === "tts"), [models]);

  const selectedStt = sttModels.find((m) => m.id === draft.stt_model);
  const selectedVoice = ttsVoices.find((m) => m.id === draft.tts_voice);

  return (
    <div className="settings">
      <h2 className="app__panel-title">Settings</h2>

      <label className="field">
        <span className="field__label">Global hotkey</span>
        <input
          className="field__input"
          value={draft.hotkey}
          onChange={(e) => set("hotkey", e.target.value)}
          placeholder="Ctrl+Shift+Space"
        />
        <span className="field__hint">
          Accelerator like <code>Ctrl+Shift+Space</code>. Re-registered on save.
        </span>
      </label>

      <label className="field">
        <span className="field__label">Default output mode</span>
        <select
          className="field__input"
          value={draft.default_mode}
          onChange={(e) =>
            set("default_mode", e.target.value as Settings["default_mode"])
          }
        >
          {OUTPUT_MODES.map((m) => (
            <option key={m.value} value={m.value}>
              {m.label}
            </option>
          ))}
        </select>
      </label>

      <label className="field">
        <span className="field__label">Transcription provider</span>
        <select
          className="field__input"
          value={draft.transcription_provider}
          onChange={(e) =>
            set("transcription_provider", e.target.value as TranscriptionKind)
          }
        >
          <option value="mock">Mock (no models needed)</option>
          <option value="sherpa">Local speech engine (sherpa-onnx)</option>
        </select>
      </label>

      <label className="field">
        <span className="field__label">Rewrite provider</span>
        <select
          className="field__input"
          value={draft.rewrite_provider}
          onChange={(e) =>
            set("rewrite_provider", e.target.value as RewriteKind)
          }
        >
          <option value="mock">Mock (deterministic)</option>
          <option value="foundry_local">Foundry Local (phi-4-mini-instruct)</option>
        </select>
      </label>

      <label className="field">
        <span className="field__label">Speech-to-text model</span>
        <select
          className="field__input"
          value={draft.stt_model}
          onChange={(e) => set("stt_model", e.target.value)}
        >
          {sttModels.length === 0 && (
            <option value={draft.stt_model}>{draft.stt_model}</option>
          )}
          {sttModels.map((m) => (
            <option key={m.id} value={m.id}>
              {m.display_name} {m.bundled ? "(bundled)" : `(~${m.approx_mb} MB)`}
              {m.installed ? " ✓" : ""}
            </option>
          ))}
        </select>
        <span className="field__hint">
          The bundled tiny model works offline out of the box. Larger models are
          optional downloads.
          {selectedStt && !selectedStt.installed && onDownload && (
            <>
              {" "}
              <button
                type="button"
                className="btn btn--link"
                disabled={downloadingId === selectedStt.id}
                onClick={() => void onDownload(selectedStt.id)}
              >
                {downloadingId === selectedStt.id
                  ? downloadLabel ?? "Downloading…"
                  : `Download ${selectedStt.display_name}`}
              </button>
            </>
          )}
        </span>
      </label>

      <label className="field">
        <span className="field__label">Text-to-speech provider</span>
        <select
          className="field__input"
          value={draft.tts_provider}
          onChange={(e) => set("tts_provider", e.target.value as TtsKind)}
        >
          <option value="mock">Mock (beep, no model needed)</option>
          <option value="sherpa">Local neural voice (sherpa-onnx)</option>
        </select>
      </label>

      <label className="field">
        <span className="field__label">Voice</span>
        <select
          className="field__input"
          value={draft.tts_voice}
          onChange={(e) => set("tts_voice", e.target.value)}
          disabled={draft.tts_provider !== "sherpa"}
        >
          {ttsVoices.length === 0 && (
            <option value={draft.tts_voice}>{draft.tts_voice}</option>
          )}
          {ttsVoices.map((m) => (
            <option key={m.id} value={m.id}>
              {m.display_name} {m.bundled ? "(bundled)" : `(~${m.approx_mb} MB)`}
              {m.installed ? " ✓" : ""}
            </option>
          ))}
        </select>
        <span className="field__hint">
          Used when the TTS provider is the local neural engine. The bundled
          default voice works offline.
          {selectedVoice && !selectedVoice.installed && onDownload && (
            <>
              {" "}
              <button
                type="button"
                className="btn btn--link"
                disabled={downloadingId === selectedVoice.id}
                onClick={() => void onDownload(selectedVoice.id)}
              >
                {downloadingId === selectedVoice.id
                  ? downloadLabel ?? "Downloading…"
                  : `Download ${selectedVoice.display_name}`}
              </button>
            </>
          )}
        </span>
      </label>

      <label className="field">
        <span className="field__label">Foundry model name</span>
        <input
          className="field__input"
          value={draft.foundry_model}
          onChange={(e) => set("foundry_model", e.target.value)}
          placeholder="phi-4-mini-instruct"
        />
      </label>

      <label className="field">
        <span className="field__label">Foundry endpoint override (optional)</span>
        <input
          className="field__input"
          value={draft.foundry_endpoint ?? ""}
          onChange={(e) =>
            set("foundry_endpoint", e.target.value.trim() === "" ? null : e.target.value)
          }
          placeholder="Leave blank to auto-discover the dynamic port"
        />
        <span className="field__hint">
          For debugging only. Foundry Local uses a dynamic localhost port that is
          discovered automatically; set this to force a specific endpoint.
        </span>
      </label>

      <label className="field field--checkbox">
        <input
          type="checkbox"
          checked={draft.auto_copy}
          onChange={(e) => set("auto_copy", e.target.checked)}
        />
        <span className="field__label">
          Auto-copy output to clipboard after processing
        </span>
      </label>

      <label className="field field--checkbox">
        <input
          type="checkbox"
          checked={draft.auto_speak}
          onChange={(e) => set("auto_speak", e.target.checked)}
        />
        <span className="field__label">
          Auto-speak the output aloud after processing (default off)
        </span>
      </label>

      <div className="settings__actions">
        <button
          type="button"
          className="btn btn--primary"
          disabled={saving}
          onClick={() => void onSave(draft)}
        >
          {saving ? "Saving…" : "Save settings"}
        </button>
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() => void onClearTemp()}
        >
          Clear temp files
        </button>
      </div>
    </div>
  );
}
