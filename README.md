# VoiceFlow Local

**Local-first voice-to-clipboard desktop assistant.** Press a global hotkey,
speak, press it again, and get polished text on your clipboard — transcribed and
rewritten entirely on your own machine. Windows-first. Built with **Tauri v2 +
React + TypeScript** with a **Rust** backend.

No cloud. No API keys. No transcript history. No database. Nothing is sent
anywhere, and the temporary audio file is deleted after every use.

> **Mock-first:** a fresh checkout runs the whole record → transcribe → rewrite
> → preview → copy workflow **with zero local models installed**, using
> deterministic mock providers. Install Whisper and Foundry Local later and flip
> a setting to switch to real local inference.

---

## What it is

VoiceFlow Local turns spoken words into clean, ready-to-paste text:

1. Press the global hotkey (**`Ctrl+Shift+Space`** by default) to start
   recording. A visible indicator shows you're live.
2. Press it again to stop.
3. The app captures your mic to a temporary 16 kHz mono WAV in the OS temp dir.
4. It **transcribes locally** (mock provider by default; whisper.cpp via
   `whisper-rs` when enabled).
5. It **rewrites locally** into your selected output mode (mock provider by
   default; **Microsoft Foundry Local** running **`phi-4-mini-instruct`** when
   enabled).
6. You see an **editable preview**. Edit if you like, then click **Copy**
   (or enable auto-copy). The temporary WAV is deleted.

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
- **Never** the word "kindly" (stripped/replaced by a post-filter *and*
  forbidden in the prompt).
- Avoid overly formal or escalatory tone unless explicitly requested.
- Pastes cleanly into Teams, Outlook, OneNote, a PRD, or leadership notes.

---

## Mock mode vs local mode

Both providers are selected independently in **Settings**, and both default to
**Mock**.

| | Mock (default) | Local |
| --- | --- | --- |
| **Transcription** | `MockTranscriptionProvider` — returns a deterministic sample transcript. No model required. | `LocalWhisperTranscriptionProvider` — whisper.cpp via `whisper-rs`, loads a local GGML model. |
| **Rewrite** | `MockRewriteProvider` — deterministic, applies mode formatting + style rules. No model required. | `FoundryLocalRewriteProvider` — Foundry Local + `phi-4-mini-instruct` over an OpenAI-compatible REST API. |

Mock mode exists so the full UI and workflow can be validated end-to-end before
any models are installed. Switch each provider to its local implementation in
**Settings** once you've set the models up (see below), and use the **Health**
tab to confirm everything is ready.

---

## Validation matrix

Three ways to run the app, from zero-setup to fully local. Pick columns left to
right as you install more.

| | **Mock mode** (default) | **Local Whisper mode** | **Full local** (Whisper + Foundry/Phi) |
| --- | --- | --- | --- |
| **Providers** | Transcription = Mock<br>Rewrite = Mock | Transcription = Local Whisper<br>Rewrite = Mock | Transcription = Local Whisper<br>Rewrite = Foundry Local |
| **Prerequisites** | Rust + Node 18+.<br>No CMake, libclang, models, or Foundry. | Mock prereqs **plus** CMake, MSVC "Desktop development with C++", and libclang.<br>GGML model at `%APPDATA%\com.voiceflow.local\models\ggml-base.en.bin`. | Local Whisper prereqs **plus** Foundry Local installed, service running, and `phi-4-mini-instruct` downloaded + loaded. |
| **Setup / commands** | `npm install`<br>`npm run tauri dev -- --no-default-features` *(skips native whisper compile)* | `./scripts/check-native-build-prereqs.ps1`<br>`npm run tauri dev`<br>Settings → Transcription = **Local Whisper** | `winget install Microsoft.FoundryLocal`<br>`foundry service start`<br>`foundry model download phi-4-mini-instruct`<br>`foundry model load phi-4-mini-instruct`<br>`./scripts/check-local-models.ps1`<br>`./scripts/smoke-test-foundry.ps1`<br>Settings → both providers **Local**, then run **Health** tab |
| **Expected result** | Full hotkey → record → deterministic transcript → deterministic rewrite (mode formatting + style rules) → editable preview → copy. | Real speech → **real** transcript → deterministic mock rewrite → editable preview → copy. | Real speech → real transcript → **Phi** rewrite per output mode (Teams/Email/Product note/Exec summary) → editable preview → copy. No-"kindly" enforced. |
| **Known caveats** | Transcript & rewrite are canned/deterministic (no real ASR/LLM). Mic is still used if present; a missing mic is flagged in Health. | Native build needs the C/C++ toolchain; first model load is slow; English model by default. On ARM64 hosts use the emulation build recipe in **Native build & packaging**. | Foundry port is **dynamic** (discovered via `foundry service status`, never hardcoded); models need extra disk/RAM. **Not yet live-validated end-to-end — tracked in the v3 follow-up issue.** |

---

## Architecture

All provider logic lives in Rust behind async traits, so cloud providers can be
added later without touching the UI or pipeline:

- `TranscriptionProvider` → `MockTranscriptionProvider`,
  `LocalWhisperTranscriptionProvider` (`whisper-rs`).
- `RewriteProvider` → `MockRewriteProvider`, `FoundryLocalRewriteProvider`
  (Foundry Local CLI bridge + OpenAI-compatible REST on a **dynamic** localhost
  port, discovered by parsing `foundry service status` — never hardcoded).

Providers are built **per request** from the current settings, so switching in
the UI takes effect immediately.

```
src/                     React + TypeScript frontend
  components/            RecordingIndicator, OutputModePicker, PreviewPane (editable),
                         ErrorBanner, SettingsPanel, HealthPanel, PrivacyNote, Toast
  hooks/                 useRecorder, useHotkeyStatus
  lib/                   ipc.ts (typed invoke wrappers), types.ts
src-tauri/               Rust backend
  src/
    commands.rs          Tauri command handlers (IPC surface)
    state.rs             Shared app state + per-request provider factories
    settings.rs          Settings persistence + provider selection (mock-first)
    health.rs            Typed health checks (mic, temp, whisper, foundry)
    errors.rs            VfError enum → friendly UI messages
    pipeline.rs          transcribe → rewrite → style filter
    audio/               recorder.rs (cpal), wav.rs (hound), temp.rs (RAII cleanup)
    transcription/       mod.rs (trait), mock.rs, whisper_cpp.rs
    rewrite/             mod.rs (trait + OutputMode), mock.rs, foundry_local.rs, style.rs
scripts/                 setup-local-models.ps1, check-local-models.ps1,
                         check-native-build-prereqs.ps1, smoke-test-foundry.ps1
```

---

## Quick start (mock mode — zero setup)

```powershell
npm install
npm run tauri dev
```

That's it. Both providers default to Mock, so you can record, process, edit the
preview, and copy without installing any models. (Building the desktop app still
needs the Tauri/Rust prerequisites below; native whisper is only compiled when
you enable the `whisper` feature.)

To type-check / build just the frontend:

```powershell
npm run build
```

---

## Setup for local mode

### 1. Prerequisites

- **Rust** (stable) — <https://rustup.rs>
- **Node.js 18+** and npm — <https://nodejs.org>
- **Tauri v2 system prerequisites** for Windows —
  <https://tauri.app/start/prerequisites/>:
  - **Microsoft Visual Studio C++ Build Tools** (the "Desktop development with
    C++" workload), which provides the MSVC compiler and linker.
  - **WebView2** runtime (preinstalled on Windows 11; installable on Windows 10).
- **CMake** and a **C/C++ compiler** — required only to build the bundled
  whisper.cpp used by `whisper-rs` (i.e. the `whisper` Cargo feature). Install
  CMake from <https://cmake.org/download/> (or the Visual Studio "C++ CMake
  tools" component) and make sure `cmake` is on your `PATH`.
- **libclang** — required by `bindgen` (used by `whisper-rs-sys`) to generate the
  whisper.cpp FFI bindings. Install LLVM from <https://releases.llvm.org/> (or the
  Visual Studio **"C++ Clang tools for Windows"** component) and, if it isn't
  auto-detected, point `LIBCLANG_PATH` at the folder containing `libclang.dll`.

Run the preflight script to check all of the above at once:

```powershell
./scripts/check-native-build-prereqs.ps1
```

> **Build note:** the default Cargo features compile whisper.cpp from source,
> which needs CMake + a C/C++ toolchain. To type-check the Rust without building
> native whisper, use `cargo check --no-default-features`. In that build the
> local Whisper provider is unavailable and health reports it as skipped —
> mock transcription still works.

### 2. Install Microsoft Foundry Local

```powershell
winget install Microsoft.FoundryLocal
foundry service start
foundry model download phi-4-mini-instruct
foundry model load phi-4-mini-instruct
```

Foundry Local serves an OpenAI-compatible API on a dynamically-assigned
localhost port. VoiceFlow discovers it by parsing `foundry service status`, so
you never configure a port. (You can set a manual endpoint override in Settings
for debugging.)

### 3. Get the Whisper model

Download a GGML English model — **`ggml-base.en.bin`** is a good default — from
the whisper.cpp model repository:

<https://huggingface.co/ggerganov/whisper.cpp>

Place it in the app's data directory under `models/`:

```
%APPDATA%\com.voiceflow.local\models\ggml-base.en.bin
```

If the file is missing, the app does **not** crash — it shows an error banner
(and a failing health check) with the exact expected path and a download hint.

### 4. Helper scripts

```powershell
# Guided setup: checks prereqs, prepares Foundry + Phi, checks the model folder.
./scripts/setup-local-models.ps1

# Validate readiness and print PASS/FAIL (mirrors the in-app Health tab).
./scripts/check-local-models.ps1
```

Neither script downloads Whisper weights automatically, and neither commits
models or secrets.

### 5. Switch to local providers

Open **Settings**, set **Transcription provider** to *Local Whisper* and/or
**Rewrite provider** to *Foundry Local*, save, then open the **Health** tab and
run the checks.

---

## Health checks

The **Health** tab (and `run_health_checks` command) reports typed pass/fail for:

- Temp audio folder writable
- Microphone available
- Whisper model exists (and loads, in `whisper`-enabled builds)
- Foundry Local installed
- Foundry service running
- Foundry dynamic port discovered
- Phi model available
- Foundry chat-completion smoke test

Checks that don't apply to your current settings (e.g. Foundry checks while in
mock mode) are reported as **skipped**.

---

## Privacy model

- **All local.** Transcription and rewriting run entirely on your machine.
- **No cloud, no API keys** in v1.
- **No database, no history.** Transcripts live in memory only and are never
  persisted. Only your preferences are stored (in `settings.json`).
- **Temporary audio is deleted** after processing — on success *and* on error
  paths — via an RAII guard (`Drop`) plus explicit cleanup. The WAV lives only
  in the OS temp dir as `voiceflow-<uuid>.wav` for the duration of processing.
  Settings → **Clear temp files** removes any stragglers.
- **No telemetry.**
- **No auto-send.** Text reaches the clipboard only when you click **Copy** (or
  when you explicitly enable auto-copy, which is **off** by default).
- **Enterprise note:** for confidential or regulated work, only use approved
  enterprise endpoints and follow your organization's data-handling policies.
  The app shows this reminder in-product too.

---

## System requirements

- **Windows 10 or 11** (Windows-first; the code is portable but only tested on
  Windows).
- A working **microphone** (and microphone permission for the app).
- Enough **disk and RAM** for local models (only when using local providers):
  - Whisper `base.en` ≈ 140 MB on disk; larger models need more RAM.
  - `phi-4-mini-instruct` via Foundry Local needs several GB of RAM/VRAM
    depending on the runtime.
- MSVC Build Tools + CMake to build native whisper from source (see Setup).

---

## Limitations (v1)

- **No transcript history** and no database.
- **No cloud providers** — local only (the provider traits make adding them
  later straightforward).
- **No auto-send** — copy-to-clipboard only (auto-copy is opt-in).
- **No always-on listening** — records only on explicit start, always with a
  visible indicator.
- **English Whisper model** by default (`ggml-base.en.bin`).
- **Requires Foundry Local + `phi-4-mini-instruct`** for non-Raw modes when the
  rewrite provider is set to Foundry Local. Mock rewrite works with no setup.

---

## Development notes

- Rust unit tests cover the mock providers, the style post-filter, the Foundry
  endpoint parser, and mock error mapping: `cargo test --no-default-features`.
- The frontend is type-checked by `tsc` as part of `npm run build`.
- Building the full app (`npm run tauri dev` / `tauri build`) requires the
  native prerequisites above; CMake is only needed for the `whisper` feature.

---

## Native build & packaging (real local models)

The default Cargo features compile whisper.cpp from source and link it into the
app, enabling the **Local Whisper** provider. This has been validated end to end:

- `cargo check` / `cargo test` pass with the default `whisper` feature (whisper.cpp
  compiles and links; all unit tests green).
- A real transcription smoke test loads a GGML model and transcribes a known clip
  (see the ignored `smoke` test in `src-tauri/src/transcription/whisper_cpp.rs`,
  driven by the `WHISPER_SMOKE_MODEL` / `WHISPER_SMOKE_WAV` env vars).
- `tauri build --no-bundle` produces an optimized release binary
  (`voiceflow-local.exe`).

### Build the release binary

```powershell
# Ensure native prereqs first:
./scripts/check-native-build-prereqs.ps1

# Type-check + compile the optimized binary (no installer):
npm run tauri build -- --no-bundle
```

### Build a Windows installer

`tauri build` produces **NSIS** and **MSI** installers (configured in
`src-tauri/tauri.conf.json` under `bundle.targets`). WebView2 is delivered via the
`downloadBootstrapper` install mode, so the installer stays small and fetches the
runtime on first launch if it's missing.

```powershell
npm run tauri build
# Installers are written to:
#   src-tauri/target/release/bundle/nsis/*.exe
#   src-tauri/target/release/bundle/msi/*.msi
```

> Installer generation should be run on a native **x64 Windows** host. Code signing
> is not configured (unsigned installers trigger SmartScreen). The bundled icons are
> placeholders — replace `src-tauri/icons/*` before a public release.

### Foundry rewrite smoke test

With Foundry Local installed and running, validate the Phi rewrite path per output
mode (mirrors the in-app health check and the no-"kindly" post-filter):

```powershell
./scripts/smoke-test-foundry.ps1
```
