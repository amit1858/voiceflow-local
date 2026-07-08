import { useEffect } from "react";

interface Props {
  message: string | null;
  onDone: () => void;
}

/** Lightweight transient toast (e.g. "Copied"). Auto-dismisses. */
export function Toast({ message, onDone }: Props) {
  useEffect(() => {
    if (!message) return;
    const t = window.setTimeout(onDone, 1800);
    return () => window.clearTimeout(t);
  }, [message, onDone]);

  if (!message) return null;

  return (
    <div className="toast" role="status" aria-live="polite">
      {message}
    </div>
  );
}
