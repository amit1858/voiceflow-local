<#
.SYNOPSIS
    Verify local or package-staged speech model files against pinned SHA-256.
#>

[CmdletBinding()]
param(
    [string]$ModelsRoot = (Join-Path $env:APPDATA "com.voiceflow.local\models"),
    [string]$SttModel = "whisper-tiny-en",
    [string]$TtsVoice = ""
)

$Registry = @{
    "whisper-tiny-en" = @{
        Kind = "stt"
        Revision = "d026532c022fa99fd789d6b32446a1df7b6bfc43"
        Files = @(
            @{ Name = "tiny.en-encoder.int8.onnx"; Sha256 = "0ce578b827c94a961aacb8fa14b02f096504b337e5c94be37c36238cbe3e8bc6" },
            @{ Name = "tiny.en-decoder.int8.onnx"; Sha256 = "06c0e6ff6348d427e51839219d1c886c18cfdf411e629e33f5e1679bff9c1527" },
            @{ Name = "tiny.en-tokens.txt"; Sha256 = "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930" }
        )
    }
    "whisper-base-en" = @{
        Kind = "stt"
        Revision = "59eea950fc76df2453efb57e6c0fd334548e8ffe"
        Files = @(
            @{ Name = "base.en-encoder.int8.onnx"; Sha256 = "ef6b936f4c9b1d90a3b68634b60c4ed8576b26172b33c2535ec0e933c9edb823" },
            @{ Name = "base.en-decoder.int8.onnx"; Sha256 = "f7162ad6db2dbef16cfaeaa7f945b9d7dd9c1b8d472f6aca82f2273d185e4d41" },
            @{ Name = "base.en-tokens.txt"; Sha256 = "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930" }
        )
    }
    "vits-ljs" = @{
        Kind = "tts"
        Revision = "7ac337c834f318e45a34037cb3371cc3929187ff"
        Files = @(
            @{ Name = "vits-ljs.onnx"; Sha256 = "5bbd273797a9ecf8d94bd6ec02ad16cb41cbb85f055ad98d528ced3e44c9b31a" },
            @{ Name = "tokens.txt"; Sha256 = "5fee2c6b238d712287f2ecb08f34a8a8b413bcb7390862ef6fb6fd6f0f8d3a17" },
            @{ Name = "lexicon.txt"; Sha256 = "bdccfc6da71c45c48e2e0056fcf0aab760577c5f959f6c1b5eb3e3e916fd5a0e" }
        )
    }
}

$failures = 0
function Test-Model([string]$Id) {
    if ([string]::IsNullOrWhiteSpace($Id)) { return }
    $entry = $Registry[$Id]
    if (-not $entry) {
        Write-Host "FAIL  Unknown model '$Id'." -ForegroundColor Red
        $script:failures++
        return
    }
    $dir = Join-Path $ModelsRoot (Join-Path $entry.Kind $Id)
    foreach ($file in $entry.Files) {
        $path = Join-Path $dir $file.Name
        if (-not (Test-Path $path)) {
            Write-Host "FAIL  Missing $path" -ForegroundColor Red
            $script:failures++
            continue
        }
        $actual = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLowerInvariant()
        if ($actual -ne $file.Sha256) {
            Write-Host "FAIL  Corrupt $path (expected $($file.Sha256), got $actual)" -ForegroundColor Red
            $script:failures++
        } else {
            Write-Host "PASS  $Id / $($file.Name)" -ForegroundColor Green
        }
    }
    Write-Host "INFO  Source revision: $($entry.Revision)"
}

Test-Model $SttModel
Test-Model $TtsVoice
if ($failures -gt 0) { exit 1 }
Write-Host "PASS  All selected model files are present and verified." -ForegroundColor Green
