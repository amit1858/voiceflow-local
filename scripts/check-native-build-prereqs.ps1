<#
.SYNOPSIS
    Check the Windows prerequisites for a real Sherpa consumer build.
#>

[CmdletBinding()]
param()

$failures = 0
function Test-Tool([string]$Name, [string]$Hint) {
    if (Get-Command $Name -ErrorAction SilentlyContinue) {
        Write-Host "PASS  $Name is available" -ForegroundColor Green
    } else {
        Write-Host "FAIL  $Name is missing. $Hint" -ForegroundColor Red
        $script:failures++
    }
}

Test-Tool "node" "Install Node.js LTS."
Test-Tool "npm" "Install Node.js LTS."
Test-Tool "cargo" "Install Rust stable."

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $install = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath 2>$null
    if ($install) {
        Write-Host "PASS  MSVC C++ linker tools installed at $install" -ForegroundColor Green
    } else {
        Write-Host "FAIL  Install Visual Studio Build Tools with Desktop development with C++." -ForegroundColor Red
        $failures++
    }

    $libclang = $null
    if ($env:LIBCLANG_PATH) {
        $candidate = Join-Path $env:LIBCLANG_PATH "libclang.dll"
        if (Test-Path $candidate) { $libclang = $candidate }
    }
    if (-not $libclang) {
        $candidate = Join-Path $env:ProgramFiles "LLVM\bin\libclang.dll"
        if (Test-Path $candidate) { $libclang = $candidate }
    }
    if ($libclang) {
        Write-Host "PASS  libclang found at $libclang" -ForegroundColor Green
        Write-Host "INFO  libclang must match the Rust build host architecture."
    } else {
        Write-Host "FAIL  libclang.dll was not found; sherpa-rs-sys 0.6.8 requires it for bindgen." -ForegroundColor Red
        $failures++
    }
} else {
    Write-Host "FAIL  Visual Studio Installer/vswhere was not found." -ForegroundColor Red
    $failures++
}

if ($failures -gt 0) { exit 1 }
Write-Host "PASS  Real Sherpa consumer build prerequisites are present." -ForegroundColor Green
