<#
.SYNOPSIS
    Prepare local speech models for VoiceFlow Local (optional - the app runs in
    mock mode with zero setup, and the packaged build ships a tiny STT model and
    a default TTS voice for offline first run).

.DESCRIPTION
    The real speech path uses sherpa-onnx (via the sherpa-rs crate). The prebuilt
    ONNX Runtime + sherpa-onnx native libraries are fetched automatically by the
    Rust build when compiled with `--features sherpa` (no CMake / C++ / libclang
    needed). This script only helps you place the *models*:

      * STT (speech-to-text): a sherpa-onnx Whisper model
          - whisper-tiny-en (bundled default, ~100 MB)
          - whisper-base-en (optional, more accurate, ~155 MB)
      * TTS (text-to-speech): a sherpa-onnx VITS voice
          - vits-ljs (bundled default English voice, ~115 MB)
      * Rewrite (unchanged): Microsoft Foundry Local + phi-4-mini-instruct

    Models are stored under the app-data dir in per-model folders:
        %APPDATA%\com.voiceflow.local\models\stt\<id>\...
        %APPDATA%\com.voiceflow.local\models\tts\<id>\...

    Normally you do NOT need this script: use the in-app Settings -> "Download"
    buttons, which show progress and verify files. It is provided for offline /
    scripted setup. It NEVER commits models or secrets.

.NOTES
    Safe to re-run (skips files already present). Downloads over HTTPS only.
#>

[CmdletBinding()]
param(
    # Root of the app-data models dir.
    [string]$ModelsRoot = (Join-Path $env:APPDATA "com.voiceflow.local\models"),
    # STT model id to fetch (whisper-tiny-en | whisper-base-en).
    [string]$SttModel = "whisper-tiny-en",
    # TTS voice id to fetch (vits-ljs).
    [string]$TtsVoice = "vits-ljs",
    # Foundry rewrite model.
    [string]$FoundryModel = "phi-4-mini-instruct",
    # Skip model downloads (only do prereq + Foundry checks).
    [switch]$SkipModels
)

$ErrorActionPreference = "Stop"
function Write-Head($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }
function Write-Ok($t)   { Write-Host "  [OK]   $t" -ForegroundColor Green }
function Write-Warn($t) { Write-Host "  [WARN] $t" -ForegroundColor Yellow }
function Write-Info($t) { Write-Host "  [INFO] $t" -ForegroundColor Gray }

# Registry MUST mirror src-tauri/src/models/mod.rs REGISTRY.
$Registry = @{
    "whisper-tiny-en" = @{
        Kind  = "stt"
        Files = @(
            @{ Name = "tiny.en-encoder.int8.onnx"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/main/tiny.en-encoder.int8.onnx" },
            @{ Name = "tiny.en-decoder.int8.onnx"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/main/tiny.en-decoder.int8.onnx" },
            @{ Name = "tiny.en-tokens.txt";        Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/main/tiny.en-tokens.txt" }
        )
    }
    "whisper-base-en" = @{
        Kind  = "stt"
        Files = @(
            @{ Name = "base.en-encoder.int8.onnx"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/main/base.en-encoder.int8.onnx" },
            @{ Name = "base.en-decoder.int8.onnx"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/main/base.en-decoder.int8.onnx" },
            @{ Name = "base.en-tokens.txt";        Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/main/base.en-tokens.txt" }
        )
    }
    "vits-ljs" = @{
        Kind  = "tts"
        Files = @(
            @{ Name = "vits-ljs.onnx"; Url = "https://huggingface.co/csukuangfj/vits-ljs/resolve/main/vits-ljs.onnx" },
            @{ Name = "tokens.txt";    Url = "https://huggingface.co/csukuangfj/vits-ljs/resolve/main/tokens.txt" },
            @{ Name = "lexicon.txt";   Url = "https://huggingface.co/csukuangfj/vits-ljs/resolve/main/lexicon.txt" }
        )
    }
}

function Get-ModelInto($Id) {
    $entry = $Registry[$Id]
    if (-not $entry) { Write-Warn "Unknown model id '$Id' - skipping."; return }
    $dir = Join-Path $ModelsRoot (Join-Path $entry.Kind $Id)
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    Write-Info "Target: $dir"
    foreach ($f in $entry.Files) {
        $dest = Join-Path $dir $f.Name
        if ((Test-Path $dest) -and ((Get-Item $dest).Length -gt 0)) {
            Write-Ok "Present: $($f.Name)"
            continue
        }
        Write-Info "Downloading $($f.Name) ..."
        try {
            Invoke-WebRequest -Uri $f.Url -OutFile $dest -UseBasicParsing
            $mb = [math]::Round((Get-Item $dest).Length / 1MB, 1)
            Write-Ok "Downloaded $($f.Name) ($mb MB)"
        } catch {
            Write-Warn "Failed to download $($f.Name): $($_.Exception.Message)"
        }
    }
}

Write-Head "Prerequisites"
foreach ($tool in @(
    @{ Name = "node";  Hint = "Install Node.js LTS from https://nodejs.org" },
    @{ Name = "cargo"; Hint = "Install Rust from https://rustup.rs" }
)) {
    if (Get-Command $tool.Name -ErrorAction SilentlyContinue) {
        Write-Ok "$($tool.Name) found"
    } else {
        Write-Warn "$($tool.Name) not found - $($tool.Hint)"
    }
}
Write-Info "The sherpa-onnx native libraries are fetched by the Rust build"
Write-Info "(cargo ... --features sherpa) - no CMake / C++ / libclang required."

Write-Head "Foundry Local (rewrite provider - optional)"
$foundry = Get-Command foundry -ErrorAction SilentlyContinue
if (-not $foundry) {
    Write-Warn "Foundry Local CLI not found."
    Write-Info "Install it, then re-run this script:"
    Write-Info "    winget install Microsoft.FoundryLocal"
    Write-Info "You can keep using Mock rewrite in the app until this is ready."
} else {
    Write-Ok "foundry CLI found"
    try { foundry service start | Out-Null; Write-Ok "Service started (or already running)" }
    catch { Write-Warn "Could not start service: $($_.Exception.Message)" }
    try { foundry model download $FoundryModel; Write-Ok "Model '$FoundryModel' downloaded" }
    catch { Write-Warn "Download failed: $($_.Exception.Message)" }
    try { foundry model load $FoundryModel; Write-Ok "Model '$FoundryModel' loaded" }
    catch { Write-Warn "Load failed: $($_.Exception.Message)" }
}

if ($SkipModels) {
    Write-Head "Speech models"
    Write-Info "Skipped (-SkipModels). Use the in-app Settings -> Download buttons instead."
} else {
    Write-Head "STT model ($SttModel)"
    Get-ModelInto $SttModel

    Write-Head "TTS voice ($TtsVoice)"
    Get-ModelInto $TtsVoice
}

Write-Head "Done"
Write-Info "Set the STT model and TTS voice in the app under Settings."
Write-Info "Reminder: models, voices, and DLLs are git-ignored and must never be committed."
