# VoiceFlow Local

Windows-first, local speech-to-clipboard built with Tauri v2, React, TypeScript,
Rust, CPAL, and sherpa-onnx. The core path is:

```text
microphone -> CPAL capture -> 16 kHz mono WAV -> sherpa-onnx Whisper
           -> optional local rewrite -> editable preview -> clipboard
```

Speech, transcript, and output are not stored as history. Captured audio uses an
app-owned temporary directory and is removed after processing or cancellation.
Settings and installed models persist locally.

## Consumer and developer behavior

- **Consumer release:** compiled with `sherpa`, defaults to real local STT, and
  does not expose canned mock transcription. A release build without `sherpa`
  fails at compile time.
- **Developer debug:** keeps explicitly labelled mock STT/rewrite/TTS providers
  for UI development and deterministic tests. Mock STT returns canned text and
  is never a fallback for a failed real provider.
- **Rewrite:** Raw mode requires no LLM. Other modes can use deterministic
  development rewrite or Microsoft Foundry Local. Foundry endpoints are limited
  to loopback addresses; redirects and environment proxy routing are disabled.
- **TTS:** remains an optional experiment and is not part of the transcription
  pipeline or the Chunk 1 installer guarantee.

## Run and build

```powershell
npm ci

# Developer build; explicit mocks are available.
npm run tauri:dev

# Developer build with the real speech engine.
npm run tauri:dev:sherpa

# Consumer x64 installers. This stages and verifies the pinned default STT
# model, compiles Sherpa, and creates NSIS/MSI packages.
npm run tauri:build
```

The consumer build downloads about **99 MiB** of model data at package time.
The resulting installer is correspondingly larger, plus the application,
sherpa-onnx, and ONNX Runtime assets. The validated unsigned x64 outputs were
about **53.5 MiB (NSIS)** and **67.1 MiB (MSI)**; exact sizes can vary with
toolchain and signing. The packaging machine needs the normal Tauri Windows
prerequisites and a same-architecture `libclang.dll` because `sherpa-rs-sys
0.6.8` generates bindings during its build. Installed consumer machines do not
need Rust, Node, Visual Studio, CMake, or libclang.

## Speech models and integrity

Runtime models live under:

```text
%APPDATA%\com.voiceflow.local\models\stt\<id>\
%APPDATA%\com.voiceflow.local\models\tts\<id>\
```

| ID | Purpose | Consumer bundle | Immutable upstream revision | Approx. size |
|---|---|---:|---|---:|
| `whisper-tiny-en` | English STT | Yes | `d026532c022fa99fd789d6b32446a1df7b6bfc43` | 99 MiB |
| `whisper-base-en` | More accurate English STT | No | `59eea950fc76df2453efb57e6c0fd334548e8ffe` | 153 MiB |
| `vits-ljs` | Optional English TTS | No | `7ac337c834f318e45a34037cb3371cc3929187ff` | 113 MiB |

Every file has a required SHA-256 in `src-tauri/src/models/mod.rs`. Existing
files, downloads, package-staged resources, first-run copies, and provider
activation are verified. Downloads use `.part` files and never activate an
incomplete or mismatched artifact.

`scripts/prepare-release-models.ps1` stages only the default STT model under
`src-tauri/resources/models`. `build.rs` repeats the hash checks for release
builds, and the release-only `src-tauri/tauri.release.conf.json` packages that
resource directory plus the x64 Sherpa/ONNX runtime DLLs copied by
`sherpa-rs-sys`. On first consumer launch, the app verifies the bundled files
again before copying them into app data. `bundled: true` therefore means an
artifact participates in this concrete packaging path; it is not merely a
registry label.

For local development model installation:

```powershell
.\scripts\setup-local-models.ps1
.\scripts\check-local-models.ps1

# Optional TTS voice:
.\scripts\setup-local-models.ps1 -TtsVoice vits-ljs
```

## Audio and error behavior

CPAL captures the default input device in its native format. Interleaved input
is downmixed to mono, normalized to `f32`, linearly resampled to 16 kHz, checked
for minimum duration and silence, converted to 16-bit PCM, and written to a
temporary WAV for the current sherpa-rs file-oriented provider.

Temporary WAVs are restricted to:

```text
%TEMP%\voiceflow-local\audio\<uuid>.wav
```

Startup and the Settings cleanup action remove only UUID-named WAVs in that
directory. Unrelated temp files are never scanned or deleted. Explicit cleanup
failures are returned to the UI; startup and health diagnostics report stale
cleanup failures.

Typed UI errors cover microphone absence/permission, unsupported or failed
audio streams, short/silent input, temp creation/cleanup, missing/corrupt
models, unavailable engine builds, model load, inference, downloads, Foundry
availability, and rewrite failures. Real-provider failures never return canned
transcripts.

## Foundry Local boundary

Foundry Local is optional and used only after STT for non-Raw output modes.

- Auto-discovered and manual endpoints must resolve syntactically to
  `localhost`, `127.0.0.0/8`, or `::1`.
- Only HTTP(S) is accepted; credentials in URLs are rejected.
- The HTTP client disables redirects and proxy routing.
- CLI exit status is checked. Service/model failures are not treated as
  successful output.
- Runtime processing does not automatically download a Foundry model. Install
  it explicitly:

```powershell
winget install Microsoft.FoundryLocal
foundry service start
foundry model download phi-4-mini-instruct
foundry model load phi-4-mini-instruct
```

Remaining network behavior: model installation and release-model staging use
HTTPS against immutable Hugging Face revisions. Normal STT inference and Raw
output processing require no network, account, API key, or cloud inference.

## Health diagnostics

The Health panel independently reports:

- app-owned temp storage and stale cleanup;
- default microphone availability;
- whether sherpa-onnx is compiled in;
- selected STT file presence;
- selected STT SHA-256 verification;
- successful STT recognizer/model initialization;
- TTS output/voice/synthesis readiness when selected;
- Foundry CLI, service, endpoint, model, and completion readiness when selected.

A debug mock result does not make real STT healthy.

## Tests and real inference smoke

```powershell
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo test --manifest-path src-tauri\Cargo.toml --features sherpa
.\scripts\smoke-test-stt.ps1
```

The known-WAV smoke test uses upstream `test_wavs/0.wav` from the same immutable
Whisper tiny revision and asserts that real inference contains the expected
phrases “early nightfall” and “yellow lamps.” Unit tests and mock tests cannot
prove physical microphone capture, acoustic quality, device permissions, or
installer behavior on a separate clean machine.

## Clean Windows offline acceptance test

1. On the build machine, check out the release commit and run
   `npm ci`, then `npm run tauri:build`.
2. Confirm the staging command prints `Verified` for all three tiny.en files.
3. Inspect
   `src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\` and
   `...\msi\`; retain the generated installer and its SHA-256.
4. Use a clean Windows 10/11 x64 machine with no Rust, Node, Visual Studio, or
   model files. Keep it online only for installation if the WebView2
   bootstrapper is needed.
5. Install VoiceFlow Local. Grant microphone permission in Windows Settings if
   prompted.
6. Launch the app. Confirm the header says **Speech engine: real**, Settings
   selects **Local speech engine (sherpa-onnx)**, and the tiny model shows
   **verified**.
7. Open Health and verify microphone, speech engine, model presence, model hash,
   and model-load checks pass. Foundry and TTS may be skipped for Raw STT.
8. Select **Raw transcript**, select the intended Windows default microphone,
   hold `Ctrl+Shift+Space`, speak a unique English sentence, and press the
   shortcut again.
9. Confirm the preview contains the spoken sentence rather than the developer
   mock text (“This is a mock transcript…”). Copy it and paste into Notepad.
10. Exit the app, disconnect Ethernet/Wi-Fi, relaunch, repeat steps 7–9 with a
    different sentence, and confirm successful transcription.
11. Confirm `%TEMP%\voiceflow-local\audio` has no completed-recording WAV and no
    transcript/history file exists in app data.
12. Corruption check: reconnect, close the app, alter one byte in a tiny model
    file under `%APPDATA%\com.voiceflow.local\models\stt\whisper-tiny-en`, then
    relaunch. Confirm health reports a hash failure and recording does not
    produce canned text. Reinstall or delete the corrupt model and use the
    verified package copy/download to repair it.

Physical microphone and clean-machine offline acceptance must be performed by a
human on the generated Windows installer; repository builds and known-WAV tests
are not substitutes.

For the complete nondeveloper procedure, artifact hash commands, safe corruption
test, and required report format, use
[`docs/windows-chunk1-acceptance.md`](docs/windows-chunk1-acceptance.md).

## Licensing

`THIRD_PARTY_NOTICES.md` is packaged with consumer builds. sherpa-onnx and
sherpa-rs are Apache-2.0; ONNX Runtime is MIT; the bundled Whisper artifact is
derived from OpenAI Whisper (MIT) and pinned to the source revision shown above.
Review and preserve upstream notices before public redistribution.

## Chunk 1 boundaries

This repository intentionally does not add tray behavior, target-application
insertion, floating overlays, personalization, strict-offline UI controls, or
cross-platform product work. The current interaction remains record, optional
rewrite, preview, and copy.
