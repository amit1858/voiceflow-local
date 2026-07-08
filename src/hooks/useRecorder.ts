// useRecorder — orchestrates the record → process → preview lifecycle.

import { useCallback, useRef, useState } from "react";
import {
  cancelRecording,
  startRecording,
  stopAndProcess,
} from "../lib/ipc";
import type {
  OutputMode,
  PipelineResult,
  RecordingState,
  VfError,
} from "../lib/types";

export interface UseRecorder {
  state: RecordingState;
  result: PipelineResult | null;
  error: VfError | null;
  /** Toggle: start if idle, stop+process if recording. No-op while processing. */
  toggle: (mode: OutputMode) => Promise<void>;
  start: () => Promise<void>;
  stop: (mode: OutputMode) => Promise<void>;
  cancel: () => Promise<void>;
  clearError: () => void;
  clearResult: () => void;
}

export function useRecorder(): UseRecorder {
  const [state, setState] = useState<RecordingState>("idle");
  const [result, setResult] = useState<PipelineResult | null>(null);
  const [error, setError] = useState<VfError | null>(null);

  // Guards against overlapping async transitions (e.g. rapid hotkey presses).
  const busy = useRef(false);

  const start = useCallback(async () => {
    if (busy.current || state !== "idle") return;
    busy.current = true;
    setError(null);
    try {
      await startRecording();
      setState("recording");
    } catch (err) {
      setError(err as VfError);
      setState("idle");
    } finally {
      busy.current = false;
    }
  }, [state]);

  const stop = useCallback(
    async (mode: OutputMode) => {
      if (busy.current || state !== "recording") return;
      busy.current = true;
      setState("processing");
      try {
        const res = await stopAndProcess(mode);
        setResult(res);
      } catch (err) {
        setError(err as VfError);
      } finally {
        setState("idle");
        busy.current = false;
      }
    },
    [state],
  );

  const toggle = useCallback(
    async (mode: OutputMode) => {
      if (state === "idle") {
        await start();
      } else if (state === "recording") {
        await stop(mode);
      }
      // While "processing" we intentionally ignore toggles.
    },
    [state, start, stop],
  );

  const cancel = useCallback(async () => {
    if (state !== "recording") return;
    busy.current = true;
    try {
      await cancelRecording();
    } catch (err) {
      setError(err as VfError);
    } finally {
      setState("idle");
      busy.current = false;
    }
  }, [state]);

  const clearError = useCallback(() => setError(null), []);
  const clearResult = useCallback(() => setResult(null), []);

  return {
    state,
    result,
    error,
    toggle,
    start,
    stop,
    cancel,
    clearError,
    clearResult,
  };
}
