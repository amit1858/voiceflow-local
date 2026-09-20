<#
.SYNOPSIS
    Download checksum-pinned local speech models into VoiceFlow app data.

.DESCRIPTION
    All artifact URLs use immutable Hugging Face revisions. Every existing and
    downloaded file is SHA-256 verified before it is accepted. Partial files are
    removed on failure. Foundry setup is optional and every CLI exit code is
    checked.
#>

[CmdletBinding()]
param(
    [string]$ModelsRoot = (Join-Path $env:APPDATA "com.voiceflow.local\models"),
    [string]$SttModel = "whisper-tiny-en",
    [string]$TtsVoice = "",
    [string]$FoundryModel = "phi-4-mini-instruct",
    [switch]$SetupFoundry,
    [switch]$SkipModels
)

$ErrorActionPreference = "Stop"
$Registry = @{
    "whisper-tiny-en" = @{
        Kind = "stt"
        Files = @(
            @{ Name = "tiny.en-encoder.int8.onnx"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-encoder.int8.onnx"; Sha256 = "0ce578b827c94a961aacb8fa14b02f096504b337e5c94be37c36238cbe3e8bc6" },
            @{ Name = "tiny.en-decoder.int8.onnx"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-decoder.int8.onnx"; Sha256 = "06c0e6ff6348d427e51839219d1c886c18cfdf411e629e33f5e1679bff9c1527" },
            @{ Name = "tiny.en-tokens.txt"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-tokens.txt"; Sha256 = "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930" }
        )
    }
    "whisper-base-en" = @{
        Kind = "stt"
        Files = @(
            @{ Name = "base.en-encoder.int8.onnx"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-encoder.int8.onnx"; Sha256 = "ef6b936f4c9b1d90a3b68634b60c4ed8576b26172b33c2535ec0e933c9edb823" },
            @{ Name = "base.en-decoder.int8.onnx"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-decoder.int8.onnx"; Sha256 = "f7162ad6db2dbef16cfaeaa7f945b9d7dd9c1b8d472f6aca82f2273d185e4d41" },
            @{ Name = "base.en-tokens.txt"; Url = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-tokens.txt"; Sha256 = "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930" }
        )
    }
    "vits-ljs" = @{
        Kind = "tts"
        Files = @(
            @{ Name = "vits-ljs.onnx"; Url = "https://huggingface.co/csukuangfj/vits-ljs/resolve/7ac337c834f318e45a34037cb3371cc3929187ff/vits-ljs.onnx"; Sha256 = "5bbd273797a9ecf8d94bd6ec02ad16cb41cbb85f055ad98d528ced3e44c9b31a" },
            @{ Name = "tokens.txt"; Url = "https://huggingface.co/csukuangfj/vits-ljs/resolve/7ac337c834f318e45a34037cb3371cc3929187ff/tokens.txt"; Sha256 = "5fee2c6b238d712287f2ecb08f34a8a8b413bcb7390862ef6fb6fd6f0f8d3a17" },
            @{ Name = "lexicon.txt"; Url = "https://huggingface.co/csukuangfj/vits-ljs/resolve/7ac337c834f318e45a34037cb3371cc3929187ff/lexicon.txt"; Sha256 = "bdccfc6da71c45c48e2e0056fcf0aab760577c5f959f6c1b5eb3e3e916fd5a0e" }
        )
    }
}

function Install-Model([string]$Id) {
    if ([string]::IsNullOrWhiteSpace($Id)) { return }
    $entry = $Registry[$Id]
    if (-not $entry) { throw "Unknown model id '$Id'." }
    $dir = Join-Path $ModelsRoot (Join-Path $entry.Kind $Id)
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    foreach ($file in $entry.Files) {
        $target = Join-Path $dir $file.Name
        $valid = (Test-Path $target) -and
            ((Get-FileHash -Algorithm SHA256 $target).Hash.ToLowerInvariant() -eq $file.Sha256)
        if (-not $valid) {
            $partial = "$target.part"
            Remove-Item -LiteralPath $partial -Force -ErrorAction SilentlyContinue
            try {
                Write-Host "Downloading $Id / $($file.Name)..."
                Invoke-WebRequest -Uri $file.Url -OutFile $partial -UseBasicParsing
                $actual = (Get-FileHash -Algorithm SHA256 $partial).Hash.ToLowerInvariant()
                if ($actual -ne $file.Sha256) {
                    throw "SHA-256 mismatch: expected $($file.Sha256), got $actual"
                }
                Move-Item -LiteralPath $partial -Destination $target -Force
            } finally {
                Remove-Item -LiteralPath $partial -Force -ErrorAction SilentlyContinue
            }
        }
        Write-Host "Verified $Id / $($file.Name)"
    }
}

if (-not $SkipModels) {
    Install-Model $SttModel
    Install-Model $TtsVoice
}

if ($SetupFoundry) {
    if (-not (Get-Command foundry -ErrorAction SilentlyContinue)) {
        throw "Foundry Local is not installed. Install Microsoft.FoundryLocal first."
    }
    foreach ($args in @(
        @("service", "start"),
        @("model", "download", $FoundryModel),
        @("model", "load", $FoundryModel)
    )) {
        & foundry @args
        if ($LASTEXITCODE -ne 0) {
            throw "'foundry $($args -join ' ')' exited with code $LASTEXITCODE."
        }
    }
}

Write-Host "Local model setup completed with checksum verification."
