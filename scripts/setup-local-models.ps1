<#
.SYNOPSIS
    Prepare local models for VoiceFlow Local (optional — the app runs in mock
    mode without any of this).

.DESCRIPTION
    Checks prerequisites and guides you through getting the two local models:
      * Microsoft Foundry Local + phi-4-mini-instruct (rewrite provider)
      * A GGML Whisper model, e.g. ggml-base.en.bin (transcription provider)

    This script NEVER downloads Whisper weights for you (licensing/size) and
    never commits models or secrets. It prints clear, copy-pasteable steps.

.NOTES
    Safe to re-run. Read-only except for creating the models folder.
#>

[CmdletBinding()]
param(
    # Where the Whisper GGML model should live. Defaults to the app data dir.
    [string]$WhisperModelDir = (Join-Path $env:APPDATA "com.voiceflow.local\models"),
    # Whisper model filename the app expects by default.
    [string]$WhisperModelFile = "ggml-base.en.bin",
    # Foundry model id.
    [string]$FoundryModel = "phi-4-mini-instruct"
)

$ErrorActionPreference = "Stop"
function Write-Head($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }
function Write-Ok($t)   { Write-Host "  [OK]   $t" -ForegroundColor Green }
function Write-Warn($t) { Write-Host "  [WARN] $t" -ForegroundColor Yellow }
function Write-Info($t) { Write-Host "  [INFO] $t" -ForegroundColor Gray }

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

Write-Head "Foundry Local (rewrite provider)"

$foundry = Get-Command foundry -ErrorAction SilentlyContinue
if (-not $foundry) {
    Write-Warn "Foundry Local CLI not found."
    Write-Info "Install it, then re-run this script:"
    Write-Info "    winget install Microsoft.FoundryLocal"
    Write-Info "You can keep using Mock rewrite in the app until this is ready."
} else {
    Write-Ok "foundry CLI found"
    Write-Info "Starting the Foundry service (idempotent)..."
    try { foundry service start | Out-Null; Write-Ok "Service started (or already running)" }
    catch { Write-Warn "Could not start service: $($_.Exception.Message)" }

    Write-Info "Downloading model '$FoundryModel' (skips if present)..."
    try { foundry model download $FoundryModel; Write-Ok "Model '$FoundryModel' downloaded" }
    catch { Write-Warn "Download failed: $($_.Exception.Message)" }

    Write-Info "Loading model '$FoundryModel'..."
    try { foundry model load $FoundryModel; Write-Ok "Model '$FoundryModel' loaded" }
    catch { Write-Warn "Load failed: $($_.Exception.Message)" }
}

Write-Head "Whisper GGML model (transcription provider)"

if (-not (Test-Path $WhisperModelDir)) {
    New-Item -ItemType Directory -Path $WhisperModelDir -Force | Out-Null
    Write-Ok "Created model folder: $WhisperModelDir"
} else {
    Write-Ok "Model folder exists: $WhisperModelDir"
}

$modelPath = Join-Path $WhisperModelDir $WhisperModelFile
if (Test-Path $modelPath) {
    Write-Ok "Whisper model present: $modelPath"
} else {
    Write-Warn "Whisper model NOT found at: $modelPath"
    Write-Info "Download a supported GGML model and place it there. For example, the"
    Write-Info "official whisper.cpp models are published on Hugging Face:"
    Write-Info "    https://huggingface.co/ggerganov/whisper.cpp"
    Write-Info "Recommended for English: ggml-base.en.bin"
    Write-Info "This script does not fetch weights automatically (size + licensing)."
    Write-Info "In the app, set Settings -> Whisper model path to this file, or keep"
    Write-Info "using the Mock transcription provider."
}

Write-Head "Done"
Write-Info "Reminder: models and secrets are git-ignored and must never be committed."
