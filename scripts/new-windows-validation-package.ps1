<#
.SYNOPSIS
    Assemble unsigned Windows x64 installers and their provenance manifest.
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,
    [Parameter(Mandatory = $true)]
    [string]$Repository,
    [Parameter(Mandatory = $true)]
    [string]$Ref,
    [Parameter(Mandatory = $true)]
    [ValidatePattern("^[0-9a-fA-F]{40}$")]
    [string]$CommitSha,
    [string]$BuildTimeUtc = ([DateTime]::UtcNow.ToString("o")),
    [string]$ReleaseRoot = (Join-Path $PSScriptRoot "..\src-tauri\target\x86_64-pc-windows-msvc\release"),
    [string]$ModelRoot = (Join-Path $PSScriptRoot "..\src-tauri\resources\models\stt\whisper-tiny-en")
)

$ErrorActionPreference = "Stop"
$ExpectedModel = [ordered]@{
    Id = "whisper-tiny-en"
    Revision = "d026532c022fa99fd789d6b32446a1df7b6bfc43"
    Files = @(
        [ordered]@{ Name = "tiny.en-encoder.int8.onnx"; Sha256 = "0ce578b827c94a961aacb8fa14b02f096504b337e5c94be37c36238cbe3e8bc6" },
        [ordered]@{ Name = "tiny.en-decoder.int8.onnx"; Sha256 = "06c0e6ff6348d427e51839219d1c886c18cfdf411e629e33f5e1679bff9c1527" },
        [ordered]@{ Name = "tiny.en-tokens.txt"; Sha256 = "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930" }
    )
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-PeMachine([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $reader = [System.IO.BinaryReader]::new($stream)
        $stream.Position = 0x3c
        $peOffset = $reader.ReadInt32()
        $stream.Position = $peOffset + 4
        return $reader.ReadUInt16()
    } finally {
        $stream.Dispose()
    }
}

function Get-SingleInstaller([string]$Directory, [string]$Pattern) {
    $matches = @(Get-ChildItem -LiteralPath $Directory -Filter $Pattern -File)
    if ($matches.Count -ne 1) {
        throw "Expected exactly one $Pattern installer in $Directory; found $($matches.Count)."
    }
    return $matches[0]
}

$packageJson = Get-Content (Join-Path $PSScriptRoot "..\package.json") -Raw | ConvertFrom-Json
$version = [string]$packageJson.version
$shortSha = $CommitSha.Substring(0, 12).ToLowerInvariant()
$bundleRoot = Join-Path $ReleaseRoot "bundle"
$nsisSource = Get-SingleInstaller (Join-Path $bundleRoot "nsis") "*.exe"
$msiSource = Get-SingleInstaller (Join-Path $bundleRoot "msi") "*.msi"

$runtimeLibraries = @(Get-ChildItem -LiteralPath $ReleaseRoot -Filter "*.dll" -File | Sort-Object Name)
if ($runtimeLibraries.Count -lt 5) {
    throw "Expected at least five Sherpa/ONNX runtime DLLs in $ReleaseRoot; found $($runtimeLibraries.Count)."
}
foreach ($library in $runtimeLibraries) {
    $machine = Get-PeMachine $library.FullName
    if ($machine -ne 0x8664) {
        throw ("Runtime library {0} is not x64 (PE machine 0x{1:x4})." -f $library.Name, $machine)
    }
}

$modelFiles = foreach ($expected in $ExpectedModel.Files) {
    $path = Join-Path $ModelRoot $expected.Name
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Bundled STT file is missing: $path"
    }
    $actual = Get-Sha256 $path
    if ($actual -ne $expected.Sha256) {
        throw "Bundled STT hash mismatch for $($expected.Name): expected $($expected.Sha256), got $actual."
    }
    [ordered]@{
        name = $expected.Name
        sha256 = $actual
        sizeBytes = (Get-Item -LiteralPath $path).Length
    }
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$baseName = "VoiceFlow-Local-$version-windows-x64-validation-$shortSha"
$nsisName = "$baseName-setup.exe"
$msiName = "$baseName.msi"
$nsisPath = Join-Path $OutputDirectory $nsisName
$msiPath = Join-Path $OutputDirectory $msiName
Copy-Item -LiteralPath $nsisSource.FullName -Destination $nsisPath -Force
Copy-Item -LiteralPath $msiSource.FullName -Destination $msiPath -Force

foreach ($installer in @($nsisPath, $msiPath)) {
    $signature = Get-AuthenticodeSignature -LiteralPath $installer
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::NotSigned) {
        throw "Validation installer $installer must remain unsigned; signature status was $($signature.Status)."
    }
}

$installerEntries = @(
    [ordered]@{
        format = "NSIS"
        fileName = $nsisName
        sha256 = Get-Sha256 $nsisPath
        sizeBytes = (Get-Item -LiteralPath $nsisPath).Length
    },
    [ordered]@{
        format = "MSI"
        fileName = $msiName
        sha256 = Get-Sha256 $msiPath
        sizeBytes = (Get-Item -LiteralPath $msiPath).Length
    }
)

$manifest = [ordered]@{
    schemaVersion = 1
    validationOnly = $true
    unsignedValidationStatement = "UNSIGNED VALIDATION BUILD ONLY. Not for production distribution."
    repository = $Repository
    ref = $Ref
    commitSha = $CommitSha.ToLowerInvariant()
    buildTimeUtc = ([DateTime]::Parse($BuildTimeUtc).ToUniversalTime().ToString("o"))
    architecture = "x64"
    targetTriple = "x86_64-pc-windows-msvc"
    version = $version
    installers = $installerEntries
    bundledStt = [ordered]@{
        id = $ExpectedModel.Id
        revision = $ExpectedModel.Revision
        files = @($modelFiles)
    }
    packagedRuntimeLibraries = @(
        $runtimeLibraries | ForEach-Object {
            [ordered]@{
                name = $_.Name
                sha256 = Get-Sha256 $_.FullName
                architecture = "x64"
            }
        }
    )
}

$manifestName = "$baseName-provenance.json"
$manifestPath = Join-Path $OutputDirectory $manifestName
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifestPath -Encoding UTF8

$checksums = foreach ($installer in $installerEntries) {
    "$($installer.sha256)  $($installer.fileName)"
}
$checksums += "$(Get-Sha256 $manifestPath)  $manifestName"
$checksums | Set-Content -LiteralPath (Join-Path $OutputDirectory "SHA256SUMS.txt") -Encoding ASCII

Write-Host "Validation artifact contents:"
Get-ChildItem -LiteralPath $OutputDirectory -File | ForEach-Object {
    Write-Host ("  {0}  {1}" -f (Get-Sha256 $_.FullName), $_.Name)
}

if ($env:GITHUB_STEP_SUMMARY) {
    @"
## Unsigned Windows x64 validation package

- Source: ``$Repository@$($CommitSha.ToLowerInvariant())``
- Ref: ``$Ref``
- Artifact directory: ``$OutputDirectory``
- NSIS: ``$nsisName`` — ``$($installerEntries[0].sha256)``
- MSI: ``$msiName`` — ``$($installerEntries[1].sha256)``
- Provenance: ``$manifestName``
- Status: **UNSIGNED VALIDATION BUILD ONLY**
"@ | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Encoding UTF8
}
