// Thin, typed wrapper around Tauri IPC (`invoke`) and event listeners.
// All backend calls go through here so components never touch raw invoke.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  isVfError,
  type HealthCheck,
  type OutputMode,
  type PipelineResult,
  type Settings,
  type VfError,
} from "./types";

/** Normalize any thrown value from `invoke` into a `VfError`. */
function toVfError(err: unknown): VfError {
  if (isVfError(err)) return err;
  if (typeof err === "string") {
    return { code: "Internal", message: err };
  }
  return { code: "Internal", message: "Unexpected error", hint: null };
}

/** Begin capturing microphone audio to a temp WAV. */
export async function startRecording(): Promise<void> {
  try {
    await invoke("start_recording");
  } catch (err) {
    throw toVfError(err);
  }
}

/**
 * Stop recording, then run the full transcribe + rewrite pipeline for the
 * selected mode. The temp WAV is always deleted by the backend afterwards.
 */
export async function stopAndProcess(
  mode: OutputMode,
): Promise<PipelineResult> {
  try {
    return await invoke<PipelineResult>("stop_and_process", { mode });
  } catch (err) {
    throw toVfError(err);
  }
}

/** Cancel an in-progress recording and delete the temp WAV without processing. */
export async function cancelRecording(): Promise<void> {
  try {
    await invoke("cancel_recording");
  } catch (err) {
    throw toVfError(err);
  }
}

/** Copy the given text to the OS clipboard. */
export async function copyToClipboard(text: string): Promise<void> {
  try {
    await writeText(text);
  } catch (err) {
    throw toVfError(err);
  }
}

/** Read the currently-configured global hotkey (accelerator string). */
export async function getHotkey(): Promise<string> {
  try {
    return await invoke<string>("get_hotkey");
  } catch (err) {
    throw toVfError(err);
  }
}

/** Update the global hotkey. Returns the accepted accelerator string. */
export async function setHotkey(accelerator: string): Promise<string> {
  try {
    return await invoke<string>("set_hotkey", { accelerator });
  } catch (err) {
    throw toVfError(err);
  }
}

/** Read the persisted user settings. */
export async function getSettings(): Promise<Settings> {
  try {
    return await invoke<Settings>("get_settings");
  } catch (err) {
    throw toVfError(err);
  }
}

/** Persist user settings. Returns the saved settings. */
export async function saveSettings(settings: Settings): Promise<Settings> {
  try {
    return await invoke<Settings>("save_settings", { settings });
  } catch (err) {
    throw toVfError(err);
  }
}

/** Run all provider/environment health checks. */
export async function runHealthChecks(): Promise<HealthCheck[]> {
  try {
    return await invoke<HealthCheck[]>("run_health_checks");
  } catch (err) {
    throw toVfError(err);
  }
}

/** Delete leftover `voiceflow-*.wav` temp files. Returns the count removed. */
export async function clearTempFiles(): Promise<number> {
  try {
    return await invoke<number>("clear_temp_files");
  } catch (err) {
    throw toVfError(err);
  }
}

/**
 * Subscribe to the backend "hotkey-toggle" event, emitted whenever the user
 * presses the global shortcut. The handler should toggle recording state.
 */
export async function onHotkeyToggle(
  handler: () => void,
): Promise<UnlistenFn> {
  return listen("hotkey-toggle", () => handler());
}

/**
 * Subscribe to the backend "input-level" event, emitted ~10×/second while
 * recording with the live microphone RMS level in `[0, 1]` for the mic meter.
 */
export async function onInputLevel(
  handler: (level: number) => void,
): Promise<UnlistenFn> {
  return listen<number>("input-level", (e) => handler(e.payload));
}
