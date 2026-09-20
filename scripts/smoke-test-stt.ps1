<#
.SYNOPSIS
    Run a real sherpa-onnx inference against a pinned upstream known WAV.
#>

[CmdletBinding()]
param(
    [string]$ModelsRoot = "",
    [string]$WavPath = ""
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($ModelsRoot)) {
    $ModelsRoot = Join-Path $PSScriptRoot "..\src-tauri\resources\models"
}
if ([string]::IsNullOrWhiteSpace($WavPath)) {
    $WavPath = Join-Path $env:TEMP "voiceflow-local-stt-smoke-0.wav"
    Invoke-WebRequest `
        -Uri "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/test_wavs/0.wav" `
        -OutFile $WavPath `
        -UseBasicParsing
}

$env:VOICEFLOW_STT_SMOKE_ROOT = (Resolve-Path $ModelsRoot).Path
$env:VOICEFLOW_STT_SMOKE_WAV = (Resolve-Path $WavPath).Path

try {
    cargo test --manifest-path (Join-Path $PSScriptRoot "..\src-tauri\Cargo.toml") `
        --features sherpa real_model_known_wav_smoke -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) {
        throw "Real STT smoke test failed with exit code $LASTEXITCODE."
    }
} finally {
    if ($WavPath -like (Join-Path $env:TEMP "voiceflow-local-stt-smoke-*")) {
        Remove-Item -LiteralPath $WavPath -Force -ErrorAction SilentlyContinue
    }
}
