<#
.SYNOPSIS
    Validate local speech readiness for VoiceFlow Local and print PASS/FAIL.

.DESCRIPTION
    Non-destructive checks that mirror the in-app Health panel:
      * sherpa-onnx native libraries present (only required for a real-engine
        build; fetched by `cargo ... --features sherpa`)
      * STT model files present under the app-data models dir
      * TTS voice files present under the app-data models dir
      * Foundry Local CLI installed, service running, endpoint discovered, and
        the phi model available (rewrite provider - optional)

    Exits 0 if all *applicable* checks pass, 1 otherwise. Mock STT/TTS and mock
    rewrite need none of this - the app runs with zero setup.

.NOTES
    Read-only. Never prints or requires secrets.
#>

[CmdletBinding()]
param(
    [string]$ModelsRoot = (Join-Path $env:APPDATA "com.voiceflow.local\models"),
    [string]$SttModel = "whisper-tiny-en",
    [string]$TtsVoice = "vits-ljs",
    [string]$FoundryModel = "phi-4-mini-instruct",
    # Optional: a build/target dir to scan for the sherpa-onnx / ONNX Runtime DLLs.
    [string]$TargetDir = (Join-Path $PSScriptRoot "..\src-tauri\target")
)

$script:failures = 0
function Pass($t) { Write-Host "  PASS  $t" -ForegroundColor Green }
function Fail($t) { Write-Host "  FAIL  $t" -ForegroundColor Red; $script:failures++ }
function Info($t) { Write-Host "  ....  $t" -ForegroundColor Gray }
function Head($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }

# Expected files per model - MUST mirror src-tauri/src/models/mod.rs REGISTRY.
$ModelFiles = @{
    "whisper-tiny-en" = @{ Kind = "stt"; Files = @("tiny.en-encoder.int8.onnx", "tiny.en-decoder.int8.onnx", "tiny.en-tokens.txt") }
    "whisper-base-en" = @{ Kind = "stt"; Files = @("base.en-encoder.int8.onnx", "base.en-decoder.int8.onnx", "base.en-tokens.txt") }
    "vits-ljs"        = @{ Kind = "tts"; Files = @("vits-ljs.onnx", "tokens.txt", "lexicon.txt") }
}

function Test-Model($Id) {
    $entry = $ModelFiles[$Id]
    if (-not $entry) { Fail "Unknown model id '$Id'"; return }
    $dir = Join-Path $ModelsRoot (Join-Path $entry.Kind $Id)
    if (-not (Test-Path $dir)) {
        Info "$Id not installed at: $dir"
        Info "  Install via the in-app Settings -> Download, or setup-local-models.ps1."
        return
    }
    $allOk = $true
    foreach ($name in $entry.Files) {
        $p = Join-Path $dir $name
        if ((Test-Path $p) -and ((Get-Item $p).Length -gt 0)) {
            $mb = [math]::Round((Get-Item $p).Length / 1MB, 2)
            Pass "$Id / $name present ($mb MB)"
        } else {
            Fail "$Id / $name MISSING or empty at $p"
            $allOk = $false
        }
    }
    if ($allOk) { Pass "$Id is fully installed" }
}

Head "sherpa-onnx native libraries (real-engine build only)"
$dllNames = @("onnxruntime.dll", "sherpa-onnx-c-api.dll")
if (Test-Path $TargetDir) {
    $found = @{}
    foreach ($n in $dllNames) {
        $hit = Get-ChildItem -Path $TargetDir -Recurse -Filter $n -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($hit) { $found[$n] = $hit.FullName }
    }
    if ($found.Count -gt 0) {
        foreach ($k in $found.Keys) { Pass "Found $k ($($found[$k]))" }
        if ($found.Count -lt $dllNames.Count) {
            Info "Some sherpa DLLs not seen yet - they are fetched on the first '--features sherpa' build."
        }
    } else {
        Info "No sherpa-onnx DLLs found under $TargetDir."
        Info "  They are downloaded automatically the first time you build with"
        Info "  '--features sherpa' (npm run tauri:build:sherpa). Not needed for mock mode."
    }
} else {
    Info "No target dir at $TargetDir (project not built yet)."
    Info "  Mock mode needs no DLLs; a real-engine build fetches them via --features sherpa."
}

Head "STT model ($SttModel)"
Test-Model $SttModel

Head "TTS voice ($TtsVoice)"
Test-Model $TtsVoice

Head "Foundry Local (rewrite provider - optional)"
$foundry = Get-Command foundry -ErrorAction SilentlyContinue
if (-not $foundry) {
    Info "Foundry Local CLI not installed (winget install Microsoft.FoundryLocal)."
    Info "  Optional - the app falls back to Mock rewrite."
} else {
    Pass "Foundry Local CLI installed"
    $status = ""
    try { $status = (foundry service status 2>&1 | Out-String) } catch { $status = "" }
    $match = [regex]::Match($status, "https?://[^\s""',]+")
    if ($match.Success) {
        $endpoint = [regex]::Match($match.Value.TrimEnd('/'), "^https?://[^/]+").Value
        Pass "Service running; endpoint discovered: $endpoint"
        try {
            $resp = Invoke-WebRequest -Uri "$endpoint/v1/models" -UseBasicParsing -TimeoutSec 10
            if ($resp.StatusCode -eq 200) { Pass "Local REST endpoint responded (/v1/models)" }
            else { Fail "Endpoint returned HTTP $($resp.StatusCode)" }
        } catch {
            Fail "Endpoint did not respond: $($_.Exception.Message)"
        }
    } else {
        Info "Service not running (run: foundry service start) - optional."
    }
    try {
        $list = (foundry model list 2>&1 | Out-String)
        if ($list -match [regex]::Escape($FoundryModel)) { Pass "Model '$FoundryModel' is available" }
        else { Info "Model '$FoundryModel' not downloaded (foundry model download $FoundryModel)" }
    } catch {
        Info "Could not list Foundry models: $($_.Exception.Message)"
    }
}

Head "Summary"
if ($script:failures -eq 0) {
    Write-Host "  All applicable checks PASSED." -ForegroundColor Green
    exit 0
} else {
    Write-Host "  $script:failures check(s) FAILED." -ForegroundColor Red
    exit 1
}
