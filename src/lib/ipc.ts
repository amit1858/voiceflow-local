// Thin, typed wrapper around Tauri IPC (`invoke`) and event listeners.
// All backend calls go through here so components never touch raw invoke.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  isVfError,
  type OutputMode,
  type PipelineResult,
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

/**
 * Subscribe to the backend "hotkey-toggle" event, emitted whenever the user
 * presses the global shortcut. The handler should toggle recording state.
 */
export async function onHotkeyToggle(
  handler: () => void,
): Promise<UnlistenFn> {
  return listen("hotkey-toggle", () => handler());
}
