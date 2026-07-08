<#
.SYNOPSIS
    Validate local model readiness for VoiceFlow Local and print PASS/FAIL.

.DESCRIPTION
    Non-destructive checks that mirror the in-app Health panel:
      * Foundry Local CLI installed
      * Foundry service running + dynamic endpoint discovered
      * Phi model available
      * Whisper GGML model present at the configured path

    Exits with code 0 if all *applicable* checks pass, 1 otherwise.

.NOTES
    Read-only. Never prints or requires secrets.
#>

[CmdletBinding()]
param(
    [string]$WhisperModelDir = (Join-Path $env:APPDATA "com.voiceflow.local\models"),
    [string]$WhisperModelFile = "ggml-base.en.bin",
    [string]$FoundryModel = "phi-4-mini-instruct"
)

$script:failures = 0
function Pass($t) { Write-Host "  PASS  $t" -ForegroundColor Green }
function Fail($t) { Write-Host "  FAIL  $t" -ForegroundColor Red; $script:failures++ }
function Info($t) { Write-Host "  ....  $t" -ForegroundColor Gray }
function Head($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }

Head "Foundry Local"

$foundry = Get-Command foundry -ErrorAction SilentlyContinue
if (-not $foundry) {
    Fail "Foundry Local CLI not installed (winget install Microsoft.FoundryLocal)"
} else {
    Pass "Foundry Local CLI installed"

    $status = ""
    try { $status = (foundry service status 2>&1 | Out-String) } catch { $status = "" }

    # Foundry uses a DYNAMIC localhost port; parse it out rather than assume one.
    $match = [regex]::Match($status, "https?://[^\s""',]+")
    if ($match.Success) {
        $endpoint = $match.Value.TrimEnd('/')
        # Trim any trailing path so we keep scheme://host:port.
        $endpoint = [regex]::Match($endpoint, "^https?://[^/]+").Value
        Pass "Service running; endpoint discovered: $endpoint"

        # Probe the OpenAI-compatible models endpoint.
        try {
            $resp = Invoke-WebRequest -Uri "$endpoint/v1/models" -UseBasicParsing -TimeoutSec 10
            if ($resp.StatusCode -eq 200) { Pass "Local REST endpoint responded (/v1/models)" }
            else { Fail "Endpoint returned HTTP $($resp.StatusCode)" }
        } catch {
            Fail "Endpoint did not respond: $($_.Exception.Message)"
        }
    } else {
        Fail "Service not running or endpoint not found (run: foundry service start)"
    }

    # Phi model availability.
    try {
        $list = (foundry model list 2>&1 | Out-String)
        if ($list -match [regex]::Escape($FoundryModel)) {
            Pass "Model '$FoundryModel' is available"
        } else {
            Fail "Model '$FoundryModel' not found (run: foundry model download $FoundryModel)"
        }
    } catch {
        Fail "Could not list models: $($_.Exception.Message)"
    }
}

Head "Whisper model"

$modelPath = Join-Path $WhisperModelDir $WhisperModelFile
if (Test-Path $modelPath) {
    $bytes = (Get-Item $modelPath).Length
    $sizeMB = [math]::Round($bytes / 1MB, 1)
    if ($bytes -lt 1MB) {
        Fail "Whisper model at $modelPath is only $bytes bytes - likely a truncated download or a Git-LFS pointer. Re-download it."
    } else {
        Pass "Whisper model present ($sizeMB MB): $modelPath"
    }
} else {
    Info "Whisper model not found at: $modelPath"
    Info "This is only required if you use the Local Whisper provider."
    Info "Mock transcription needs no model. See scripts/setup-local-models.ps1."
}

Head "Summary"
if ($script:failures -eq 0) {
    Write-Host "  All applicable checks PASSED." -ForegroundColor Green
    exit 0
} else {
    Write-Host "  $script:failures check(s) FAILED." -ForegroundColor Red
    exit 1
}
