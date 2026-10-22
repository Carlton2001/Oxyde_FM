# Install the official Windows binary without compiling cargo-about and its dependencies.
[CmdletBinding()]
param([string]$InstallDirectory)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This installer requires Windows x64.'
}
$timer = [Diagnostics.Stopwatch]::StartNew()
$version = '0.9.2'
$package = "cargo-about-$version-x86_64-pc-windows-msvc"
# From the official release's .tar.gz.sha256; pin locally, never trust a downloaded checksum alone.
$archiveHash = '1C03E5890238562497C2D89A3B75B02560AF349C1FC3E713D3284F532A5CD748'
# Also verify restored cache contents before running them.
$binaryHash = '7923CA2D7082C609F27C2FC599366C7B6157F58164B278C8D984653022B8969A'
if (-not $InstallDirectory) {
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    $InstallDirectory = Join-Path $cargoHome 'bin'
}
$executable = Join-Path $InstallDirectory 'cargo-about.exe'
$cached = (Test-Path -LiteralPath $executable -PathType Leaf) -and
    (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash -eq $binaryHash

if (-not $cached) {
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    $temporaryName = "oxyde-cargo-about-$([Guid]::NewGuid().ToString('N'))"
    $temporaryDirectory = Join-Path $temporaryRoot $temporaryName
    New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
    try {
        $archive = Join-Path $temporaryDirectory "$package.tar.gz"
        $uri = "https://github.com/EmbarkStudios/cargo-about/releases/download/$version/$package.tar.gz"
        Write-Host "Downloading official cargo-about $version for Windows x64."
        Invoke-WebRequest -Uri $uri -OutFile $archive -UseBasicParsing
        if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $archiveHash) {
            throw 'cargo-about archive SHA-256 mismatch; installation refused.'
        }
        # Extract only the expected binary, after verifying the entire archive.
        & tar -xzf $archive -C $temporaryDirectory "$package/cargo-about.exe"
        if ($LASTEXITCODE -ne 0) { throw 'cargo-about extraction failed.' }
        $extracted = Join-Path $temporaryDirectory "$package/cargo-about.exe"
        if ((Get-FileHash -LiteralPath $extracted -Algorithm SHA256).Hash -ne $binaryHash) {
            throw 'cargo-about binary SHA-256 mismatch; installation refused.'
        }
        $actualVersion = & $extracted --version
        if ($LASTEXITCODE -ne 0 -or "$actualVersion".Trim() -ne "cargo-about $version") {
            throw 'Unexpected cargo-about version; installation refused.'
        }
        New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
        Copy-Item -LiteralPath $extracted -Destination $executable -Force
    } finally {
        $resolved = [IO.Path]::GetFullPath($temporaryDirectory)
        if ($resolved -ne [IO.Path]::GetFullPath((Join-Path $temporaryRoot $temporaryName))) {
            throw 'Unexpected cargo-about temporary path.'
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
} else {
    Write-Host "Reusing verified cargo-about $version from cache."
}
$actualVersion = & $executable --version
if ($LASTEXITCODE -ne 0 -or "$actualVersion".Trim() -ne "cargo-about $version") {
    throw 'Installed cargo-about version check failed.'
}
Write-Host $actualVersion
Write-Host ('Distribution timing: cargo-about installation {0:N1} s (cached: {1})' -f $timer.Elapsed.TotalSeconds, $cached)
