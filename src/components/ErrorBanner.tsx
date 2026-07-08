import { ERROR_REMEDIATION, type VfError } from "../lib/types";

interface Props {
  error: VfError | null;
  onDismiss: () => void;
}

/**
 * Error banner mapping a typed `VfError` to a friendly message plus
 * remediation text. Prefers a backend-supplied hint, otherwise falls back
 * to the per-code remediation table.
 */
export function ErrorBanner({ error, onDismiss }: Props) {
  if (!error) return null;

  const remediation = error.hint || ERROR_REMEDIATION[error.code] || "";

  return (
    <div className="banner banner--error" role="alert">
      <div className="banner__body">
        <strong className="banner__title">{error.message}</strong>
        {remediation && <p className="banner__hint">{remediation}</p>}
        <span className="banner__code">Code: {error.code}</span>
      </div>
      <button
        type="button"
        className="banner__close"
        aria-label="Dismiss"
        onClick={onDismiss}
      >
        ×
      </button>
    </div>
  );
}
