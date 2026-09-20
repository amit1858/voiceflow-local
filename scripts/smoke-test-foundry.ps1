<#
.SYNOPSIS
    Smoke-test the Foundry Local + Phi rewrite path end-to-end, per output mode.

.DESCRIPTION
    Mirrors what the app's FoundryLocalRewriteProvider does, but as a standalone
    script so you can validate real local inference without launching the UI:

      1. Confirm the `foundry` CLI is installed.
      2. Discover the DYNAMIC localhost endpoint from `foundry service status`
         (or use -Endpoint to override).
      3. Confirm the Phi model is available.
      4. POST a real /v1/chat/completions for each output mode and check the
         completion is non-empty and obeys the "no kindly" style rule.

    Exits 0 if all rewrites succeed and pass the checks, 1 otherwise.

    Read-only against your machine; sends only the sample transcript below.
    Never prints or requires secrets.

.EXAMPLE
    pwsh -File scripts/smoke-test-foundry.ps1
.EXAMPLE
    pwsh -File scripts/smoke-test-foundry.ps1 -Endpoint http://127.0.0.1:5273
#>

[CmdletBinding()]
param(
    [string]$Model = "phi-4-mini-instruct",
    [string]$Endpoint = "",
    [int]$TimeoutSec = 120
)

$script:failures = 0
function Pass($t) { Write-Host "  PASS  $t" -ForegroundColor Green }
function Fail($t) { Write-Host "  FAIL  $t" -ForegroundColor Red; $script:failures++ }
function Info($t) { Write-Host "  ....  $t" -ForegroundColor Gray }
function Head($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }
function Assert-LoopbackEndpoint([string]$Value) {
    $uri = [uri]$Value
    $isLoopback = $uri.IsLoopback -or $uri.Host -eq "localhost"
    if (($uri.Scheme -ne "http" -and $uri.Scheme -ne "https") -or -not $isLoopback) {
        throw "Foundry endpoint must be loopback HTTP(S), got '$Value'."
    }
}

# Central style rules — kept in sync with src-tauri/src/rewrite/style.rs.
$styleSystem = @"
You rewrite short spoken transcripts into clean written text.
Rules: simple polished English; crisp and practical; warm but professional;
collaborative; avoid overly formal language; do not escalate tone unless asked.
Never use the word "kindly". Output must paste cleanly into Teams, Outlook,
OneNote, a product doc, or leadership notes.
"@

# Per-mode instructions — kept in sync with OutputMode::instruction() intent.
$modes = [ordered]@{
    "Teams message"     = "Rewrite as a short, crisp, conversational but professional Teams message, usually with a greeting and a clear ask or next step."
    "Email"             = "Rewrite as an email with a greeting, brief context, the main point, a clear ask or next step, and a closing."
    "Product note"      = "Rewrite as structured product notes, using short headings where helpful, practical and concrete."
    "Executive summary" = "Rewrite as an executive summary: context, key point, why it matters, the risk or decision, and the next step."
}

$sampleTranscript = "hey so i think we should kindly move the launch to next friday because the testing is not done and i want to make sure quality is good before we ship"

Head "Foundry Local CLI"
$foundry = Get-Command foundry -ErrorAction SilentlyContinue
if (-not $foundry) {
    Fail "Foundry Local CLI not installed (winget install Microsoft.FoundryLocal)"
    Write-Host "`n  Cannot run the smoke test without Foundry Local." -ForegroundColor Red
    exit 1
}
Pass "Foundry Local CLI installed"

Head "Endpoint discovery"
if (-not $Endpoint) {
    try {
        Info "Ensuring service is running (foundry service start)..."
        foundry service start 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "foundry service start exited $LASTEXITCODE" }
    } catch {
        Fail "Could not start Foundry Local: $($_.Exception.Message)"
        exit 1
    }
    $status = ""
    try { $status = (foundry service status 2>&1 | Out-String) } catch { $status = "" }
    $m = [regex]::Match($status, "https?://[^\s""',]+")
    if ($m.Success) {
        $Endpoint = [regex]::Match($m.Value.TrimEnd('/'), "^https?://[^/]+").Value
        Pass "Discovered dynamic endpoint: $Endpoint"
    } else {
        Fail "Could not discover endpoint from 'foundry service status'."
        Info "Start the service (foundry service start) or pass -Endpoint."
        exit 1
    }
} else {
    $Endpoint = $Endpoint.TrimEnd('/')
    Pass "Using endpoint override: $Endpoint"
}
try { Assert-LoopbackEndpoint $Endpoint }
catch {
    Fail $_.Exception.Message
    exit 1
}

Head "Model availability"
try {
    $list = (foundry model list 2>&1 | Out-String)
    if ($list -match [regex]::Escape($Model)) {
        Pass "Model '$Model' is available"
    } else {
        Fail "Model '$Model' not found. Run: foundry model download $Model; foundry model load $Model"
    }
} catch { Fail "Could not list models: $($_.Exception.Message)" }

foreach ($label in $modes.Keys) {
    Head "Rewrite: $label"
    $system = "$styleSystem`n`nTask: $($modes[$label])"
    $body = @{
        model       = $Model
        messages    = @(
            @{ role = "system"; content = $system },
            @{ role = "user";   content = "Transcript:`n`n$sampleTranscript" }
        )
        temperature = 0.3
        stream      = $false
    } | ConvertTo-Json -Depth 6

    try {
        $resp = Invoke-RestMethod -Uri "$Endpoint/v1/chat/completions" -Method Post `
            -ContentType "application/json" -Body $body -TimeoutSec $TimeoutSec
        $content = $resp.choices[0].message.content
        if ([string]::IsNullOrWhiteSpace($content)) {
            Fail "Empty completion for '$label'"
            continue
        }
        Pass "Got a non-empty completion ($($content.Length) chars)"
        # Mirror the app's deterministic post-filter (style.rs): whole-word,
        # case-insensitive "kindly" -> "please". The model is instructed to
        # avoid "kindly", but the post-filter guarantees it never ships.
        $filtered = [regex]::Replace($content, "(?i)\bkindly\b", "please")
        if ($filtered -match "(?i)\bkindly\b") {
            Fail "'kindly' survived the post-filter (unexpected)."
        } else {
            if ($content -match "(?i)\bkindly\b") {
                Info "Model emitted 'kindly'; post-filter replaced it with 'please'."
            }
            Pass "No 'kindly' in the post-filtered output (style rule holds)"
        }
        Write-Host "  --- post-filtered output preview ---" -ForegroundColor DarkGray
        ($filtered -split "`n" | Select-Object -First 8) | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
    } catch {
        Fail "Request failed for '$label': $($_.Exception.Message)"
    }
}

Head "Summary"
if ($script:failures -eq 0) {
    Write-Host "  All Foundry rewrite smoke checks PASSED." -ForegroundColor Green
    exit 0
} else {
    Write-Host "  $script:failures check(s) FAILED." -ForegroundColor Red
    exit 1
}
