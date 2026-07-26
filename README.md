# VoiceFlow Local

**Local-first voice-to-clipboard desktop assistant.** Press a global hotkey,
speak, press it again, and get polished text on your clipboard — transcribed and
rewritten entirely on your own machine. You can also have the result **read back
aloud** with a local neural voice. Windows-first. Built with **Tauri v2 + React +
TypeScript** with a **Rust** backend.

No cloud. No API keys. No transcript history. No database. Nothing is sent
anywhere, and the temporary audio file is deleted after every use.

> **Mock-first:** a fresh checkout runs the whole record → transcribe → rewrite →
> preview → copy (→ speak) workflow **with zero setup**, using deterministic mock
> providers. The packaged build additionally ships a tiny real STT model and a
> default neural TTS voice so **real speech-to-text and text-to-speech work out of
> the box — with no C/C++ build toolchain required.**

---

## What's new in v2

- **Real local STT that actually works out of the box.** v1's local transcription
  required compiling whisper.cpp from source (CMake + MSVC C++ + a specific
  libclang) and the default build silently fell back to a canned mock. v2 replaces
  that with **[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx)** via the
  **[`sherpa-rs`](https://crates.io/crates/sherpa-rs)** crate. Its
  `download-binaries` feature fetches **prebuilt** ONNX Runtime + sherpa-onnx
  native libraries, so **end users need no CMake / C++ / libclang**.
- **Local neural text-to-speech.** A new `TtsProvider` trait with a mock voice
  (default) and a sherpa-onnx VITS voice. A **Speak / Stop** button on the preview
  reads the output aloud, and an **auto-speak** setting (default **off**) can speak
  automatically after processing.
- **Model manager.** A tiny STT model and one TTS voice are **bundled** for offline
  first run; larger STT models and extra voices are **optional, checksum-verified
  downloads** with live progress in Settings.
- **Audio guard.** A quick tap or a silent mic now returns a clear typed message
  (*recording too short* / *no speech detected*) instead of an empty transcript,
  and a **live input-level meter** shows your mic is being heard.
- **Capability badge + honest health checks.** The header shows whether the running
  build has the **real speech engine** or is **mock-only**, so selecting a local
  provider is never a silent trap.

The **Foundry Local + `phi-4-mini-instruct`** rewrite path is unchanged. Foundry /
Phi is a **text LLM** and is used **only** for the rewrite step — it does not (and
cannot) do speech-to-text or text-to-speech; those use dedicated ONNX speech
models.

---

## What it is

VoiceFlow Local turns spoken words into clean, ready-to-paste text:

1. Press the global hotkey (**`Ctrl+Shift+Space`** by default) to start recording.
   A visible indicator and a live level meter show you're being heard.
2. Press it again to stop.
3. The app captures your mic to a temporary 16 kHz mono WAV in the OS temp dir.
4. It **transcribes locally** (mock provider by default; sherpa-onnx Whisper when
   selected).
5. It **rewrites locally** into your selected output mode (mock provider by
   default; **Microsoft Foundry Local** running **`phi-4-mini-instruct`** when
   enabled).
6. You see an **editable preview**. Edit if you like, then click **Copy** (or
   enable auto-copy). Optionally click **Speak** to hear it. The temporary WAV is
   deleted.

### Output modes

| Mode | Behavior |
| --- | --- |
| **Raw transcript** | Fix punctuation + obvious speech errors only; add no new meaning. Bypasses the rewrite model (style filter still applies). |
| **Teams message** | Short, crisp, conversational, professional — usually a greeting + a clear ask/next step. |
| **Email** | Greeting, context, main point, ask/next step, closing. |
| **Product note** | Structured notes with headings where helpful; practical. |
| **Executive summary** | Context, key point, why it matters, risk/decision, next step. |

### Writing style

Every output — including the raw transcript — is passed through a central style
filter and the style is also injected into the rewrite model's system prompt:

- Simple, polished English; crisp and practical.
- Warm but professional; collaborative.
- **Never** the word "kindly" (stripped/replaced by a post-filter *and* forbidden
  in the prompt).
- Avoid overly formal or escalatory tone unless explicitly requested.
- Pastes cleanly into Teams, Outlook, OneNote, a PRD, or leadership notes.

---

## Mock mode vs local mode

Each provider is selected independently in **Settings**, and all default to
**Mock**.

| | Mock (default) | Local |
| --- | --- | --- |
| **Transcription** | `MockTranscriptionProvider` — deterministic sample transcript. No model. | `SherpaSttProvider` — sherpa-onnx Whisper ONNX model from the app-data models dir. |
| **Rewrite** | `MockRewriteProvider` — deterministic mode formatting + style rules. No model. | `FoundryLocalRewriteProvider` — Foundry Local + `phi-4-mini-instruct` over an OpenAI-compatible REST API. |
| **Text-to-speech** | `MockTtsProvider` — a short synthesized beep. No model. | `SherpaTtsProvider` — sherpa-onnx VITS voice (bundled `vits-ljs` or a selected one). |

Mock mode exists so the full UI and workflow can be validated end-to-end with zero
setup. Switch each provider to its local implementation in **Settings**, and use
the **Health** tab and the header **capability badge** to confirm the real engine
is present.

---

## The speech engine (sherpa-onnx)

STT and TTS are both served by **sherpa-onnx**, linked through the `sherpa-rs`
crate behind an **opt-in `sherpa` Cargo feature**:

- **Default / mock build** (`npm run tauri dev`, `cargo check`) links **no** native
  speech engine — it needs only the Tauri/Rust prerequisites and runs entirely in
  mock mode. This is what CI and a fresh clone build.
- **Real-engine build** (`--features sherpa`) pulls in `sherpa-rs` with its
  `download-binaries` + `tts` features, which **download prebuilt** ONNX Runtime +
  sherpa-onnx DLLs at build time. **No CMake, no C++ compiler, and no libclang are
  required from end users** — the prebuilt binaries are linked directly.

Because the engine is behind a feature and a capability check, selecting a local
provider on a mock-only build never silently fakes success: you get a typed
`SpeechEngineUnavailable` error and the header badge reads **"Speech engine:
mock-only."**

Provider APIs used:

- **STT:** `sherpa_rs::whisper::{WhisperConfig, WhisperRecognizer}` with the
  model's `encoder` / `decoder` / `tokens` files.
- **TTS:** `sherpa_rs::tts::{VitsTts, VitsTtsConfig}` with the voice's `model` /
  `tokens` / `lexicon` files. Synthesized audio is played through
  [`rodio`](https://crates.io/crates/rodio).

---

## Models: bundling + optional downloads

Models live under the app-data models dir, split by kind:

```
%APPDATA%\com.voiceflow.local\models\stt\<id>\...
%APPDATA%\com.voiceflow.local\models\tts\<id>\...
```

Built-in registry (`src-tauri/src/models/mod.rs`):

| Id | Kind | Bundled | ~Size | Notes |
| --- | --- | --- | --- | --- |
| `whisper-tiny-en` | STT | ✅ default | ~100 MB | Fast English STT. Ships for offline first run. |
| `whisper-base-en` | STT | optional | ~155 MB | More accurate English STT. Download on demand. |
| `vits-ljs` | TTS | ✅ default | ~115 MB | Natural English neural voice. Ships for offline first run. |

**Hybrid delivery.** Bundled models make first run work fully offline. Additional
models are **optional downloads** streamed with progress events and (when a
checksum is pinned) verified with SHA-256; a mismatch is rejected with a typed
`ModelChecksumMismatch`. Download from **Settings → Download** (per model), or with
the `download_model(id)` command. Downloads write to a `.part` file and are renamed
into place only on success.

> **Models and DLLs are never committed to git.** They are fetched into the app-data
> dir at runtime (via the in-app downloads / `setup-local-models.ps1`) and into
> `src-tauri/resources/models` at **package time**. `.gitignore` keeps `*.onnx`,
> `*.bin`, `tokens.txt`, `lexicon.txt`, voice dirs, and `*.dll` out of the repo. The
> Rust *source* module `src-tauri/src/models/` is code, not data, and stays tracked.

---

## Text-to-speech (Speak + auto-speak)

TTS is a **post-preview action** — it is **never** part of `run_pipeline`, so
processing latency is unaffected and nothing is spoken unless you ask for it.

- **Speak / Stop** button on the preview reads the current (possibly edited) output
  aloud via the selected TTS provider.
- **Auto-speak** (Settings, default **off**) speaks the output automatically once
  processing completes.
- Commands: `speak(text)`, `stop_speaking`, `list_voices`. The backend emits a
  `tts-finished` event when playback drains naturally so the UI can reset the Speak
  button.
- Typed errors (`TtsVoiceMissing`, `TtsSynthFailed`, `TtsPlaybackFailed`,
  `SpeechEngineUnavailable`) map to friendly messages. Mock TTS always works (a
  short beep), so the flow can be validated with no voice installed.

---

## Audio guard + live level

The capture path (`src-tauri/src/audio/recorder.rs`) enforces a **minimum duration**
and a **silence/level threshold** *after* resampling to 16 kHz mono, independent of
the transcription provider:

- Too short → typed `RecordingTooShort`.
- Long enough but effectively silent → typed `NoSpeechDetected`.

Both surface as clear banners instead of an empty transcript. A live **input-level**
value (mic RMS, ~10×/s) is emitted as an `input-level` event and drives the meter in
the recording indicator. The existing RAII temp-WAV cleanup is unchanged.

---

## Architecture

All provider logic lives in Rust behind async traits, so providers can be swapped
(or cloud ones added later) without touching the UI or pipeline:

- `TranscriptionProvider` → `MockTranscriptionProvider`, `SherpaSttProvider`.
- `RewriteProvider` → `MockRewriteProvider`, `FoundryLocalRewriteProvider`
  (Foundry Local CLI bridge + OpenAI-compatible REST on a **dynamic** localhost
  port, discovered by parsing `foundry service status` — never hardcoded).
- `TtsProvider` → `MockTtsProvider`, `SherpaTtsProvider` (VITS via sherpa-onnx,
  played through `rodio`).

Providers are built **per request** from the current settings, so switching in the
UI takes effect immediately.

```
src/                     React + TypeScript frontend
  components/            RecordingIndicator (+ level meter), OutputModePicker,
                         PreviewPane (editable + Speak/Stop), ErrorBanner,
                         SettingsPanel (STT/TTS/download/auto-speak), HealthPanel,
                         PrivacyNote, Toast
  hooks/                 useRecorder, useHotkeyStatus
  lib/                   ipc.ts (typed invoke wrappers + event listeners), types.ts
src-tauri/               Rust backend
  src/
    commands.rs          Tauri command handlers (IPC surface)
    state.rs             Shared app state + per-request provider factories + playback
    settings.rs          Settings persistence + provider selection (mock-first)
    health.rs            Typed health + capability checks (mic, temp, STT, TTS,
                         audio output, foundry)
    errors.rs            VfError enum → friendly UI messages
    pipeline.rs          transcribe → rewrite → style filter (TTS is NOT in here)
    models/              Model registry, presence, checksum, progress downloads
    audio/               recorder.rs (cpal + guard + level), wav.rs (hound),
                         temp.rs (RAII cleanup)
    transcription/       mod.rs (trait), mock.rs, sherpa.rs
    rewrite/             mod.rs (trait + OutputMode), mock.rs, foundry_local.rs, style.rs
    tts/                 mod.rs (trait), mock.rs, sherpa.rs, playback.rs (rodio)
scripts/                 setup-local-models.ps1, check-local-models.ps1,
                         check-native-build-prereqs.ps1, smoke-test-foundry.ps1
```

---

## Quick start (mock mode — zero setup)

```powershell
npm install
npm run tauri dev
```

That's it. All providers default to Mock and the `sherpa` feature is **off by
default**, so a fresh checkout builds and runs the full record → process → edit →
copy → speak loop with **no CMake, libclang, or models** — just the Tauri/Rust
prerequisites. Enable the real speech engine with `--features sherpa` (see below).

To type-check / build just the frontend:

```powershell
npm run build
```

---

## Setup for real local speech

### 1. Prerequisites

- **Rust** (stable) — <https://rustup.rs>
- **Node.js 18+** and npm — <https://nodejs.org>
- **Tauri v2 system prerequisites** for Windows —
  <https://tauri.app/start/prerequisites/>:
  - **Microsoft Visual Studio C++ Build Tools** (the "Desktop development with
    C++" workload) for the MSVC linker used to build the Rust app itself.
  - **WebView2** runtime (preinstalled on Windows 11; installable on Windows 10).

> **No speech toolchain needed.** Unlike v1, the real speech engine does **not**
> require CMake or libclang: `--features sherpa` downloads **prebuilt** ONNX
> Runtime + sherpa-onnx DLLs. You only need the standard MSVC linker that any Rust
> Windows build uses.

### 2. Build with the real engine (optional)

```powershell
npm run tauri:dev:sherpa      # dev
npm run tauri:build:sherpa    # release + installers
```

The first `--features sherpa` build downloads the prebuilt speech-engine DLLs
(cached afterwards).

### 3. Get the speech models

Easiest: run the app and use **Settings → Download** for the STT model and TTS
voice you want (live progress, checksum-verified). Or script it:

```powershell
# Downloads the tiny STT model + default TTS voice into the app-data dir,
# and (optionally) prepares Foundry + Phi for rewrite.
./scripts/setup-local-models.ps1

# Validate readiness and print PASS/FAIL (mirrors the in-app Health tab):
./scripts/check-local-models.ps1
```

Neither script commits models or secrets.

### 4. Install Microsoft Foundry Local (rewrite — optional)

```powershell
winget install Microsoft.FoundryLocal
foundry service start
foundry model download phi-4-mini-instruct
foundry model load phi-4-mini-instruct
```

Foundry Local serves an OpenAI-compatible API on a dynamically-assigned localhost
port. VoiceFlow discovers it by parsing `foundry service status`, so you never
configure a port. (A manual endpoint override is available in Settings for
debugging.) Without Foundry, non-Raw modes fall back to the mock rewriter.

### 5. Switch to local providers

Open **Settings** and set:

- **Transcription provider** → *Sherpa (local)* and pick an **STT model**.
- **TTS provider** → *Sherpa (local)* and pick a **voice**; toggle **auto-speak**
  if you want.
- **Rewrite provider** → *Foundry Local* (optional).

Save, then open the **Health** tab and run the checks. The header badge should show
the active providers plus **"Speech engine: real."**

---

## Health checks + capability badge

The **Health** tab (and `run_health_checks` command) reports typed pass/fail for:

- Temp audio folder writable
- Microphone available
- **STT model present & loads** (in `sherpa`-enabled builds; a tiny recognition
  smoke test)
- **TTS voice present & synthesizes** a tiny sample
- **Audio output device available**
- Foundry Local installed / service running / dynamic port discovered / Phi model
  available / chat-completion smoke test

Checks that don't apply to your current settings are reported as **skipped**. The
header **capability badge** independently shows whether the running build even
contains the real speech engine, so an unavailable provider is surfaced up front
rather than failing silently at runtime.

---

## Privacy model

- **All local.** Transcription, rewriting, and speech synthesis run entirely on
  your machine.
- **No cloud, no API keys.**
- **No database, no history.** Transcripts and synthesized audio live in memory
  only and are never persisted. Only your preferences are stored (in
  `settings.json`).
- **Temporary audio is deleted** after processing — on success *and* on error
  paths — via an RAII guard (`Drop`) plus explicit cleanup. The WAV lives only in
  the OS temp dir as `voiceflow-<uuid>.wav` for the duration of processing.
  Settings → **Clear temp files** removes any stragglers.
- **No telemetry.**
- **No auto-send.** Text reaches the clipboard only when you click **Copy** (or when
  you explicitly enable auto-copy, which is **off** by default). Nothing is spoken
  unless you click **Speak** or explicitly enable **auto-speak** (also **off** by
  default).
- **Enterprise note:** for confidential or regulated work, only use approved
  enterprise endpoints and follow your organization's data-handling policies. The
  app shows this reminder in-product too.

---

## System requirements

- **Windows 10 or 11** (Windows-first; the code is portable but only tested on
  Windows).
- A working **microphone** (and microphone permission), plus an **audio output
  device** for TTS playback.
- Enough **disk and RAM** for local models (only when using local providers):
  - STT `whisper-tiny-en` ≈ 100 MB, `whisper-base-en` ≈ 155 MB on disk.
  - TTS `vits-ljs` ≈ 115 MB on disk.
  - `phi-4-mini-instruct` via Foundry Local needs several GB of RAM/VRAM depending
    on the runtime.
- MSVC Build Tools to build the Rust app. **No CMake/libclang** needed for the
  speech engine (prebuilt DLLs).

---

## Packaging (shipping the DLLs + bundled models)

To ship a self-contained installer that works offline on first run:

1. **Build with the engine:** `npm run tauri:build:sherpa`. The first build
   downloads the prebuilt ONNX Runtime + sherpa-onnx DLLs; make sure they are
   included as resources / next to the executable so the app can load them at
   runtime.
2. **Stage the bundled models** into `src-tauri/resources/models/{stt,tts}/<id>/`
   at package time (e.g. by running `./scripts/setup-local-models.ps1` pointed at
   that dir, or a build step). The app copies/uses them on first run. **Do not
   commit them** — they are git-ignored and fetched at package time.
3. `tauri build --features sherpa` produces **NSIS** and **MSI** installers
   (configured in `src-tauri/tauri.conf.json`). WebView2 is delivered via the
   `downloadBootstrapper` install mode.

```powershell
npm run tauri:build:sherpa
# Installers:
#   src-tauri/target/release/bundle/nsis/*.exe
#   src-tauri/target/release/bundle/msi/*.msi
```

> Run installer generation on a native **x64 Windows** host. Code signing is not
> configured (unsigned installers trigger SmartScreen). The bundled icons are
> placeholders — replace `src-tauri/icons/*` before a public release.

---

## Licensing & attribution

- **sherpa-onnx** and **sherpa-rs** are Apache-2.0 licensed
  (<https://github.com/k2-fsa/sherpa-onnx>,
  <https://github.com/thewh1teagle/sherpa-rs>). The prebuilt ONNX Runtime is MIT
  licensed. Include their license notices when redistributing the DLLs.
- **STT models** (`sherpa-onnx-whisper-*`) are derived from OpenAI Whisper (MIT).
- **TTS voice** `vits-ljs` is trained on the **LJ Speech** dataset (public domain).
  Confirm the license/attribution of any additional voice you bundle before
  shipping it.
- Model files are hosted on Hugging Face by
  [csukuangfj](https://huggingface.co/csukuangfj); see each model card for details.

Keep these notices with any distributed build that includes the engine or models.

---

## Development notes

- Rust unit tests cover the mock STT/TTS/rewrite providers, the style post-filter,
  the model-manager checksum verification, the audio-guard helper, the Foundry
  endpoint parser, error mapping (including the new TTS/download errors), and that
  selecting a local provider on a mock-only build returns
  `SpeechEngineUnavailable`. Mock is the default build, so `cargo test` runs them
  with **no native toolchain**. Add `--features sherpa` to also compile the
  sherpa-backed providers.
- The frontend is type-checked by `tsc` as part of `npm run build`.
- Building the app in mock mode (`npm run tauri:dev` / `tauri build`) needs only the
  Tauri/Rust prerequisites; the speech engine's DLLs are fetched only for the opt-in
  `sherpa` feature.

---

## Limitations

- **No transcript history** and no database.
- **No cloud providers** — local only (the provider traits make adding them later
  straightforward).
- **No auto-send** — copy-to-clipboard only (auto-copy is opt-in). Auto-speak is
  opt-in too.
- **No always-on listening** — records only on explicit start, always with a visible
  indicator.
- **English** bundled STT model and TTS voice by default; other languages require
  downloading matching models.
- **Rewrite** for non-Raw modes needs Foundry Local + `phi-4-mini-instruct`; mock
  rewrite works with no setup.
