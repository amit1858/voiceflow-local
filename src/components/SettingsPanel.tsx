import { useState } from "react";
import {
  OUTPUT_MODES,
  type RewriteKind,
  type Settings,
  type TranscriptionKind,
} from "../lib/types";

interface Props {
  settings: Settings;
  onSave: (next: Settings) => Promise<void> | void;
  onClearTemp: () => Promise<void> | void;
  saving?: boolean;
}

/** Settings form: providers, hotkey, defaults, model paths, auto-copy, and a
 * "Clear temp files" action. Edits a local draft, then persists on Save. */
export function SettingsPanel({ settings, onSave, onClearTemp, saving }: Props) {
  const [draft, setDraft] = useState<Settings>(settings);

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setDraft((d) => ({ ...d, [key]: value }));

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
          <option value="local_whisper">Local Whisper (whisper.cpp)</option>
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
        <span className="field__label">Whisper model path</span>
        <input
          className="field__input"
          value={draft.whisper_model_path}
          onChange={(e) => set("whisper_model_path", e.target.value)}
          placeholder="…/models/ggml-base.en.bin"
        />
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
