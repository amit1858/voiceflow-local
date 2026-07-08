# VoiceFlow Local

**Local-first voice-to-clipboard desktop assistant.** Press a global hotkey,
speak, press it again, and get polished text on your clipboard — transcribed and
rewritten entirely on your own machine. Windows-first. Built with **Tauri v2 +
React + TypeScript** with a **Rust** backend.

No cloud. No API keys. No transcript history. No database. Nothing is sent
anywhere, and the temporary audio file is deleted after every use.

---

## What it is

VoiceFlow Local turns spoken words into clean, ready-to-paste text:

1. Press the global hotkey (**`Ctrl+Shift+Space`** by default) to start
   recording. A visible indicator shows you're live.
2. Press it again to stop.
3. The app captures your mic to a temporary 16 kHz mono WAV in the OS temp dir.
4. It **transcribes locally** with whisper.cpp (via `whisper-rs`).
5. It **rewrites locally** into your selected output mode using **Microsoft
   Foundry Local** running **`phi-4-mini-instruct`**.
6. You see a **preview**. Click **Copy** to put the text on your clipboard.
7. The temporary WAV is deleted.

### Output modes

| Mode | Description |
| --- | --- |
| **Raw transcript** | Cleaned transcript, bypasses the rewrite model (style filter still applies). |
| **Teams message** | Short, friendly chat message. |
| **Email** | Structured email with greeting and sign-off. |
| **Product note** | Concise product / engineering note. |
| **Executive summary** | Tight summary aimed at leadership. |

### Writing style

Every output — including the raw transcript — is passed through a central style
filter: simple, polished English; crisp and collaborative; and the word
"kindly" is never used (it is stripped/replaced). The style is also injected
into the rewrite model's system prompt.

---

## Architecture

All provider logic lives in Rust behind traits, so cloud providers can be added
later without touching the UI or pipeline:

- `TranscriptionProvider` → first impl `WhisperCppProvider` (`whisper-rs`).
- `RewriteProvider` → first impl `FoundryLocalProvider` (Foundry Local CLI
  bridge + OpenAI-compatible REST on a **dynamic** localhost port, discovered by
  parsing `foundry service status` — never hardcoded).

```
src/                     React + TypeScript frontend
  components/            RecordingIndicator, OutputModePicker, PreviewPane, ErrorBanner
  hooks/                 useRecorder, useHotkeyStatus
  lib/                   ipc.ts (typed invoke wrappers), types.ts
src-tauri/               Rust backend
  src/
    commands.rs          Tauri command handlers (IPC surface)
    state.rs             Shared app state + providers
    errors.rs            VfError enum → friendly UI messages
    pipeline.rs          transcribe → rewrite → style filter
    audio/               recorder.rs (cpal), wav.rs (hound), temp.rs (RAII cleanup)
    transcription/       mod.rs (trait), whisper_cpp.rs
    rewrite/             mod.rs (trait + OutputMode), foundry_local.rs, style.rs
```

---

## Setup

### 1. Prerequisites

- **Rust** (stable) — <https://rustup.rs>
- **Node.js 18+** and npm — <https://nodejs.org>
- **Tauri v2 system prerequisites** for Windows —
  <https://tauri.app/start/prerequisites/>:
  - **Microsoft Visual Studio C++ Build Tools** (the "Desktop development with
    C++" workload), which provides the MSVC compiler and linker.
  - **WebView2** runtime (preinstalled on Windows 11; installable on Windows 10).
- **CMake** and a **C/C++ compiler** — required to build the bundled
  whisper.cpp used by `whisper-rs`. Install CMake from
  <https://cmake.org/download/> (or the Visual Studio "C++ CMake tools"
  component) and make sure `cmake` is on your `PATH`.

> **Build note:** the default Cargo features compile whisper.cpp from source,
> which needs CMake + a C/C++ toolchain. If you only want to type-check the Rust
> without building native whisper, use `cargo check --no-default-features`
> (the `WhisperCppProvider` then returns a typed error at runtime instead of
> transcribing).

### 2. Install Microsoft Foundry Local

```powershell
winget install Microsoft.FoundryLocal
```

Then start the service and pull the model (the app also attempts this
automatically, but doing it once up front is faster):

```powershell
foundry service start
foundry model download phi-4-mini-instruct
foundry model load phi-4-mini-instruct
```

Foundry Local serves an OpenAI-compatible API on a dynamically-assigned
localhost port. VoiceFlow discovers it by parsing `foundry service status`, so
you never need to configure a port.

### 3. Get the Whisper model

Download a GGML English model — **`ggml-base.en.bin`** is a good default — from
the whisper.cpp model repository:

<https://huggingface.co/ggerganov/whisper.cpp/tree/main>

Place it in the app's data directory under `models/`:

```
%APPDATA%\com.voiceflow.local\models\ggml-base.en.bin
```

If the file is missing, the app does **not** crash — it shows an error banner
with the exact expected path and this download hint.

### 4. Run in development

```powershell
npm install
npm run tauri dev
```

To type-check / build just the frontend:

```powershell
npm run build
```

---

## Privacy model

- **All local.** Transcription (whisper.cpp) and rewriting (Foundry Local) run
  entirely on your machine.
- **No cloud, no API keys** in v1.
- **No database, no history.** Transcripts are never persisted.
- **Temporary audio is deleted** after processing — on success *and* on error
  paths — via an RAII guard (`Drop`) plus explicit cleanup. The WAV lives only
  in the OS temp dir as `voiceflow-<uuid>.wav` for the duration of processing.
- **No telemetry.**
- **No auto-send.** Text only reaches the clipboard when you click **Copy**.

---

## System requirements

- **Windows 10 or 11** (Windows-first; the code is portable but only tested on
  Windows).
- A working **microphone** (and microphone permission for the app).
- Enough **disk and RAM** for local models:
  - Whisper `base.en` ≈ 140 MB on disk; larger models need more RAM.
  - `phi-4-mini-instruct` via Foundry Local needs several GB of RAM/VRAM
    depending on the runtime.
- MSVC Build Tools + CMake to build from source (see Setup).

---

## Limitations (v1)

- **No transcript history** and no database.
- **No cloud providers** — local only (the provider traits make adding them
  later straightforward).
- **No auto-send** — copy-to-clipboard only.
- **English Whisper model** by default (`ggml-base.en.bin`).
- **Requires Foundry Local installed** and the `phi-4-mini-instruct` model for
  all non-Raw output modes.

---

## Development notes

- Rust unit tests cover the style post-filter and the Foundry endpoint parser:
  `cargo test --no-default-features`.
- The frontend is type-checked by `tsc` as part of `npm run build`.
- Building the full app (`npm run tauri dev` / `tauri build`) requires the
  native prerequisites above, including CMake for whisper.cpp.
