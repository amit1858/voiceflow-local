# Windows Chunk 1 acceptance

This procedure validates the unsigned VoiceFlow Local Chunk 1 package on a
clean Windows 10 or Windows 11 x64 computer. The tester does not need Git,
Node.js, Rust, Visual Studio, libclang, or separate model downloads.

This is human validation, not a production release. Do not use the package if
its hashes do not match the provenance manifest. Do not bypass organization
security policy to run unsigned software.

## Get and verify the package

1. Open PR #11 in GitHub, select **Checks**, and open the successful
   **Windows Chunk 1 validation** run for the commit being accepted.
2. At the bottom of the run summary, download the artifact named
   `voiceflow-local-windows-x64-validation-<12-character-SHA>`.
3. Extract the downloaded ZIP to a new folder. It contains an NSIS `.exe`, an
   `.msi`, a `*-provenance.json` file, and `SHA256SUMS.txt`.
4. Open the provenance JSON in Notepad. Confirm:
   - `validationOnly` is `true`;
   - `architecture` is `x64`;
   - `commitSha` is the full SHA shown by the GitHub run;
   - `unsignedValidationStatement` says the build is unsigned and validation
     only;
   - the bundled STT model is `whisper-tiny-en` at revision
     `d026532c022fa99fd789d6b32446a1df7b6bfc43`.
5. In the extracted folder, right-click empty space and choose **Open in
   Terminal**, then run:

   ```powershell
   $manifest = Get-ChildItem *-provenance.json | Get-Content -Raw | ConvertFrom-Json
   $manifest.installers | ForEach-Object {
     $actual = (Get-FileHash -Algorithm SHA256 $_.fileName).Hash.ToLowerInvariant()
     [pscustomobject]@{ File = $_.fileName; Expected = $_.sha256; Actual = $actual; Match = ($actual -eq $_.sha256) }
   } | Format-Table -AutoSize
   ```

6. Both rows must show `Match` as `True`. Stop and report failure if either
   hash differs.

## Install the unsigned validation build

1. Prefer the NSIS file ending in `-setup.exe`; retain the MSI for packaging
   verification or managed-install testing.
2. Double-click the installer. Windows will identify it as an unsigned or
   unknown-publisher validation build. Continue only on a test computer you
   control, after the hashes above match, and only if local policy permits.
   If Windows or organization policy blocks it, stop and record the exact
   message instead of weakening security settings.
3. Complete installation with the default options and launch VoiceFlow Local.
4. If Windows requests microphone access, allow it for this test. No developer
   tools or model setup should be requested.

## Initial health checks

1. Confirm the app identifies the speech engine as **real** and Settings shows
   **Local speech engine (sherpa-onnx)**.
2. Confirm `whisper-tiny-en` is selected and reports **verified**.
3. Select **Raw transcript** so Foundry is not used.
4. Open Health and confirm the microphone, Sherpa runtime, model presence,
   model hash, and model-load checks pass.
5. Foundry must be skipped or non-required in Raw mode. TTS may be unavailable
   or skipped and must not block recording or transcription.

## Online microphone and no-mock tests

1. While online, start recording with `Ctrl+Shift+Space`, speak exactly:

   > Cedar lantern seven records a quiet harbor at nineteen minutes past dawn.

2. Stop recording with `Ctrl+Shift+Space`. Confirm the preview meaningfully
   matches that sentence. Copy it and paste it into Notepad.
3. Record a second time and speak exactly:

   > Violet engines count forty-two paper kites beside the winter station.

4. Confirm the second result meaningfully matches the second sentence and is
   different from the first.
5. Fail the test if either result contains the developer mock phrase
   `This is a mock transcript from VoiceFlow Local` or any repeated canned text.

## Offline restart and transcription

1. Exit VoiceFlow Local completely.
2. Disconnect Wi-Fi and Ethernet.
3. Restart VoiceFlow Local. It must start without downloading a model.
4. Repeat the Health checks. The bundled model must remain verified.
5. Record and transcribe exactly:

   > Marble compass nine follows the silent river beyond the copper bridge.

6. Confirm the offline result meaningfully matches the sentence and contains no
   mock phrase. Keep the computer offline through the privacy checks.

## Privacy, temporary files, and history

1. After transcription finishes, open
   `%TEMP%\voiceflow-local\audio` in File Explorer. It may be absent or empty;
   it must not retain a completed-recording WAV.
2. Open `%APPDATA%\com.voiceflow.local`. Model and settings files are expected.
   Confirm there is no transcript database, transcript log, recording archive,
   or content-history file.
3. Search that folder for distinctive words from the three test sentences,
   such as `lantern`, `kites`, and `compass`. None should be found in file
   contents or names.
4. Confirm Raw transcription still works while Foundry is not installed or not
   running. Confirm unavailable TTS does not block the STT result.

## Safe corrupt-model and no-fallback test

Keep VoiceFlow Local running after a successful health check. This makes the
test observe runtime hash enforcement before the next startup can restore the
bundled model.

1. Open PowerShell and run:

   ```powershell
   $model = "$env:APPDATA\com.voiceflow.local\models\stt\whisper-tiny-en\tiny.en-tokens.txt"
   $backup = "$model.acceptance-backup"
   Copy-Item -LiteralPath $model -Destination $backup -Force
   $bytes = [IO.File]::ReadAllBytes($model)
   $bytes[0] = $bytes[0] -bxor 1
   [IO.File]::WriteAllBytes($model, $bytes)
   ```

2. Return to the still-running app, refresh Health, and attempt another Raw
   recording. Health/transcription must report a corrupt or invalid model.
3. Confirm no transcript is produced and especially no mock/canned transcript
   appears.
4. Exit the app, then restore the exact original file:

   ```powershell
   $model = "$env:APPDATA\com.voiceflow.local\models\stt\whisper-tiny-en\tiny.en-tokens.txt"
   $backup = "$model.acceptance-backup"
   Copy-Item -LiteralPath $backup -Destination $model -Force
   Remove-Item -LiteralPath $backup -Force
   ```

5. Restart the app while still offline. Confirm Health returns to verified and
   Raw transcription works again. Exit and restart once more, then confirm the
   same result to complete the restart test.

## Acceptance report template

Copy this template exactly into the PR report and replace every `<...>` value.
Do not mark Chunk 1 accepted until all physical-microphone and offline checks
pass.

```text
Windows Chunk 1 human acceptance report

Tester: <name>
Test date/time (UTC): <YYYY-MM-DDTHH:MM:SSZ>
Windows edition/version: <value>
Machine architecture: x64
Clean machine or clean VM: <yes/no; details>
Workflow run URL: <URL>
Workflow run ID: <ID>
Artifact name: <name>
Repository: amit1858/voiceflow-local
Branch/ref: <ref>
Full commit SHA: <40-character SHA>
App version: <version>
NSIS installer: <filename>
NSIS SHA-256 expected: <hash>
NSIS SHA-256 actual: <hash>
NSIS hash matched: <pass/fail>
MSI installer: <filename>
MSI SHA-256 expected: <hash>
MSI SHA-256 actual: <hash>
MSI hash matched: <pass/fail>
Unsigned warning observed: <yes/no; exact message>
Installed without Rust/Node/VS/Git/model setup: <pass/fail>
Real Sherpa engine shown: <pass/fail>
Bundled whisper-tiny-en verified: <pass/fail>
Initial health checks: <pass/fail; details>
Raw mode worked without Foundry: <pass/fail>
TTS was nonblocking: <pass/fail/not tested; details>
Online sentence 1 transcription: <actual text>
Online sentence 1 result: <pass/fail>
Online sentence 2 transcription: <actual text>
Online sentence 2 was different/no mock: <pass/fail>
Offline restart succeeded: <pass/fail>
Offline transcription: <actual text>
Offline transcription result: <pass/fail>
Temporary WAV cleanup: <pass/fail; details>
No transcript/content history found: <pass/fail; details>
Corrupt-model detection: <pass/fail; exact error>
No fallback/canned transcript on corruption: <pass/fail>
Model restored successfully: <pass/fail>
Post-restore restart/transcription: <pass/fail>
Second restart test: <pass/fail>
Overall Chunk 1 acceptance: <PASS/FAIL>
Blocking issues: <none or details>
Additional notes: <details>
```

