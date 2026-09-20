<#
.SYNOPSIS
    Stage the exact default STT artifacts required by a consumer installer.

.DESCRIPTION
    Downloads immutable Hugging Face revisions into src-tauri/resources and
    verifies every file against its pinned SHA-256 before Tauri packaging.
    A release build also repeats these checks in build.rs.
#>

[CmdletBinding()]
param(
    [string]$Destination = ""
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($Destination)) {
    $Destination = Join-Path $PSScriptRoot "..\src-tauri\resources\models\stt\whisper-tiny-en"
}
$Revision = "d026532c022fa99fd789d6b32446a1df7b6bfc43"
$Base = "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/$Revision"
$Files = @(
    @{ Name = "tiny.en-encoder.int8.onnx"; Sha256 = "0ce578b827c94a961aacb8fa14b02f096504b337e5c94be37c36238cbe3e8bc6" },
    @{ Name = "tiny.en-decoder.int8.onnx"; Sha256 = "06c0e6ff6348d427e51839219d1c886c18cfdf411e629e33f5e1679bff9c1527" },
    @{ Name = "tiny.en-tokens.txt"; Sha256 = "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930" }
)

New-Item -ItemType Directory -Path $Destination -Force | Out-Null

foreach ($file in $Files) {
    $target = Join-Path $Destination $file.Name
    $valid = (Test-Path $target) -and
        ((Get-FileHash -Algorithm SHA256 $target).Hash.ToLowerInvariant() -eq $file.Sha256)
    if (-not $valid) {
        $partial = "$target.part"
        Remove-Item -LiteralPath $partial -Force -ErrorAction SilentlyContinue
        Write-Host "Downloading pinned $($file.Name)..."
        try {
            Invoke-WebRequest -Uri "$Base/$($file.Name)" -OutFile $partial -UseBasicParsing
            $actual = (Get-FileHash -Algorithm SHA256 $partial).Hash.ToLowerInvariant()
            if ($actual -ne $file.Sha256) {
                throw "SHA-256 mismatch for $($file.Name): expected $($file.Sha256), got $actual"
            }
            Move-Item -LiteralPath $partial -Destination $target -Force
        } finally {
            Remove-Item -LiteralPath $partial -Force -ErrorAction SilentlyContinue
        }
    }
    $sizeMb = [math]::Round((Get-Item $target).Length / 1MB, 1)
    Write-Host "Verified $($file.Name) ($sizeMb MB)"
}

Write-Host "Pinned Whisper tiny.en revision $Revision is ready at $Destination"
