// useHotkeyStatus — loads the configured global hotkey and wires the
// backend "hotkey-toggle" event to a caller-provided handler.

import { useEffect, useRef, useState } from "react";
import { getHotkey, onHotkeyToggle, setHotkey } from "../lib/ipc";
import type { VfError } from "../lib/types";

export interface UseHotkeyStatus {
  hotkey: string;
  updateHotkey: (accelerator: string) => Promise<void>;
  error: VfError | null;
}

/**
 * @param onToggle invoked each time the global shortcut fires. The latest
 *   handler is always used (kept in a ref) so callers can pass a closure that
 *   captures current state without re-subscribing.
 */
export function useHotkeyStatus(onToggle: () => void): UseHotkeyStatus {
  const [hotkey, setHotkeyState] = useState<string>("");
  const [error, setError] = useState<VfError | null>(null);
  const handlerRef = useRef(onToggle);
  handlerRef.current = onToggle;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    (async () => {
      try {
        const current = await getHotkey();
        if (!cancelled) setHotkeyState(current);
      } catch (err) {
        if (!cancelled) setError(err as VfError);
      }
      unlisten = await onHotkeyToggle(() => handlerRef.current());
    })();

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const updateHotkey = async (accelerator: string) => {
    setError(null);
    try {
      const accepted = await setHotkey(accelerator);
      setHotkeyState(accepted);
    } catch (err) {
      setError(err as VfError);
    }
  };

  return { hotkey, updateHotkey, error };
}
