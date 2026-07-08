import type { HealthCheck } from "../lib/types";

interface Props {
  checks: HealthCheck[];
  loading: boolean;
  onRun: () => Promise<void> | void;
}

const STATUS_ICON: Record<HealthCheck["status"], string> = {
  pass: "✓",
  fail: "✗",
  skipped: "–",
};

/** Health panel showing provider/environment readiness. Runs on demand. */
export function HealthPanel({ checks, loading, onRun }: Props) {
  return (
    <div className="health">
      <div className="health__header">
        <h2 className="app__panel-title">Health checks</h2>
        <button
          type="button"
          className="btn btn--primary"
          onClick={() => void onRun()}
          disabled={loading}
        >
          {loading ? "Running…" : "Run checks"}
        </button>
      </div>

      {checks.length === 0 && !loading && (
        <p className="health__empty">
          Run the checks to verify microphone, temp folder, Whisper model, and
          Foundry Local readiness.
        </p>
      )}

      <ul className="health__list">
        {checks.map((c) => (
          <li key={c.id} className={`health__item health__item--${c.status}`}>
            <span className="health__icon" aria-hidden>
              {STATUS_ICON[c.status]}
            </span>
            <div className="health__body">
              <span className="health__label">{c.label}</span>
              <span className="health__message">{c.message}</span>
            </div>
            <span className="health__status">{c.status}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
