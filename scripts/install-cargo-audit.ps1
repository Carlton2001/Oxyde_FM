# Install the official Windows binary without compiling cargo-audit and its dependencies.
[CmdletBinding()]
param([string]$InstallDirectory)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This installer requires Windows x64.'
}
$timer = [Diagnostics.Stopwatch]::StartNew()
$version = '0.22.2'
$package = "cargo-audit-x86_64-pc-windows-msvc-v$version"
# Archive digest published by the official RustSec release; also pin the extracted executable.
$archiveHash = '0A7316540862C13D954F648917CEACCA593747BAED6EEC180FAFA590BE2710AB'
$binaryHash = '0157F5CE1CE9FD4FB0A1F7C79AF1229771D1F80B6C2613DDB0D9200A8BA73946'
if (-not $InstallDirectory) {
    $InstallDirectory = Join-Path $PSScriptRoot '../target/security-tools/bin'
}
$executable = Join-Path $InstallDirectory 'cargo-audit.exe'
$cached = (Test-Path -LiteralPath $executable -PathType Leaf) -and
    (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash -eq $binaryHash

if (-not $cached) {
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    $temporaryName = "oxyde-cargo-audit-$([Guid]::NewGuid().ToString('N'))"
    $temporaryDirectory = Join-Path $temporaryRoot $temporaryName
    New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
    try {
        $archive = Join-Path $temporaryDirectory "$package.zip"
        $uri = "https://github.com/rustsec/rustsec/releases/download/cargo-audit/v$version/$package.zip"
        Write-Host "Downloading official cargo-audit $version for Windows x64."
        Invoke-WebRequest -Uri $uri -OutFile $archive -UseBasicParsing
        if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $archiveHash) {
            throw 'cargo-audit archive SHA-256 mismatch; installation refused.'
        }
        # Read only the expected binary, after verifying the entire ZIP.
        $extracted = Join-Path $temporaryDirectory 'cargo-audit.exe'
        $zip = [IO.Compression.ZipFile]::OpenRead($archive)
        try {
            $entry = $zip.GetEntry("$package/cargo-audit.exe")
            if (-not $entry) { throw 'cargo-audit binary missing from the official archive.' }
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $extracted)
        } finally { $zip.Dispose() }
        if ((Get-FileHash -LiteralPath $extracted -Algorithm SHA256).Hash -ne $binaryHash) {
            throw 'cargo-audit binary SHA-256 mismatch; installation refused.'
        }
        $actualVersion = & $extracted --version
        if ($LASTEXITCODE -ne 0 -or "$actualVersion".Trim() -ne "cargo-audit $version") {
            throw 'Unexpected cargo-audit version; installation refused.'
        }
        New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
        Copy-Item -LiteralPath $extracted -Destination $executable -Force
    } finally {
        $resolved = [IO.Path]::GetFullPath($temporaryDirectory)
        if ($resolved -ne [IO.Path]::GetFullPath((Join-Path $temporaryRoot $temporaryName))) {
            throw 'Unexpected cargo-audit temporary path.'
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
} else {
    Write-Host "Reusing verified cargo-audit $version."
}
$actualVersion = & $executable --version
if ($LASTEXITCODE -ne 0 -or "$actualVersion".Trim() -ne "cargo-audit $version") {
    throw 'Installed cargo-audit version check failed.'
}
Write-Host $actualVersion
Write-Host ('Security timing: cargo-audit installation {0:N1} s (cached: {1})' -f $timer.Elapsed.TotalSeconds, $cached)
