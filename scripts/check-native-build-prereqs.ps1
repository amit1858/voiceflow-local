<#
.SYNOPSIS
    Preflight the toolchain needed to build VoiceFlow Local with the default
    `whisper` feature (native whisper.cpp compilation). Prints PASS/FAIL.

.DESCRIPTION
    Building with the default features compiles whisper.cpp from C/C++ via
    whisper-rs -> whisper-rs-sys, which needs, in addition to Rust + Node:

      * CMake            - configures/builds the native whisper.cpp static lib.
      * A C/C++ compiler - MSVC (Visual Studio Build Tools, "Desktop
                           development with C++" workload) or clang-cl.
      * libclang         - bindgen parses the C headers to generate Rust FFI
                           bindings; it needs libclang.dll on LIBCLANG_PATH or
                           on PATH (ships with LLVM or the VS "C++ Clang tools"
                           component).

    Mock mode needs NONE of this: build with `--no-default-features` to skip
    native whisper entirely. This script only matters when you want real local
    transcription.

    Read-only. Never prints or requires secrets.

.EXAMPLE
    pwsh -File scripts/check-native-build-prereqs.ps1
#>

[CmdletBinding()]
param()

$script:failures = 0
$script:warnings = 0
function Pass($t) { Write-Host "  PASS  $t" -ForegroundColor Green }
function Fail($t) { Write-Host "  FAIL  $t" -ForegroundColor Red; $script:failures++ }
function Warn($t) { Write-Host "  WARN  $t" -ForegroundColor Yellow; $script:warnings++ }
function Info($t) { Write-Host "  ....  $t" -ForegroundColor Gray }
function Head($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }

Head "Rust toolchain"
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargo) {
    Pass "cargo found ($(cargo --version 2>$null))"
} else {
    Fail "cargo not found. Install Rust from https://rustup.rs"
}

Head "Node.js (frontend)"
$node = Get-Command node -ErrorAction SilentlyContinue
if ($node) { Pass "node found ($(node --version 2>$null))" }
else { Fail "node not found. Install Node.js LTS from https://nodejs.org" }

Head "CMake"
$cmake = Get-Command cmake -ErrorAction SilentlyContinue
if ($cmake) {
    Pass "cmake found ($((cmake --version 2>$null | Select-Object -First 1)))"
} else {
    Fail "cmake not found. Install via the Visual Studio Installer ('C++ CMake tools for Windows') or 'winget install Kitware.CMake'."
}

Head "C/C++ compiler (MSVC)"
$cl = Get-Command cl.exe -ErrorAction SilentlyContinue
if ($cl) {
    Pass "cl.exe on PATH (you are inside a Developer/vcvars shell)"
} else {
    # cl.exe is normally only on PATH inside a Developer Command Prompt. Detect
    # an installed Build Tools / VS with the C++ workload via vswhere.
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vc = & $vswhere -latest -products * `
            -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
            -property installationPath 2>$null
        if ($vc) {
            Pass "MSVC C++ tools installed at: $vc"
            Info "cl.exe is not on PATH here; run the build from a 'x64 Native"
            Info "Tools Command Prompt' or call vcvarsall.bat amd64 first."
        } else {
            Fail "MSVC C++ build tools not found. Install Visual Studio Build Tools with the 'Desktop development with C++' workload."
        }
    } else {
        Warn "Could not locate vswhere; unable to confirm MSVC. Install VS Build"
        Warn "Tools with 'Desktop development with C++' if the build fails."
    }
}

Head "libclang (for bindgen)"
$found = $null
if ($env:LIBCLANG_PATH -and (Test-Path (Join-Path $env:LIBCLANG_PATH "libclang.dll"))) {
    $found = Join-Path $env:LIBCLANG_PATH "libclang.dll"
} else {
    # Common install locations: LLVM, and the VS bundled clang.
    $candidates = @(
        "$env:ProgramFiles\LLVM\bin\libclang.dll",
        "${env:ProgramFiles(x86)}\LLVM\bin\libclang.dll"
    )
    $vsClang = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio\*\*\VC\Tools\Llvm\**\libclang.dll" -ErrorAction SilentlyContinue |
        Select-Object -First 1 -ExpandProperty FullName
    if ($vsClang) { $candidates += $vsClang }
    foreach ($c in $candidates) { if ($c -and (Test-Path $c)) { $found = $c; break } }
}
if ($found) {
    Pass "libclang found: $found"
    if (-not $env:LIBCLANG_PATH) {
        Info "Tip: set LIBCLANG_PATH to the folder above if bindgen can't find it."
    }
} else {
    Fail "libclang.dll not found. Install LLVM ('winget install LLVM.LLVM') or the VS 'C++ Clang tools for Windows' component, then set LIBCLANG_PATH."
}

Head "Summary"
if ($script:warnings -gt 0) { Write-Host "  $script:warnings warning(s)." -ForegroundColor Yellow }
if ($script:failures -eq 0) {
    Write-Host "  Native-build prerequisites look good." -ForegroundColor Green
    Write-Host "  Build with:  cargo build   (from src-tauri, in a VC dev shell)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "  $script:failures prerequisite(s) missing." -ForegroundColor Red
    Write-Host "  You can still run MOCK mode now: build with --no-default-features." -ForegroundColor Yellow
    exit 1
}
