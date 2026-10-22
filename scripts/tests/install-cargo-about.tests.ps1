# Use an unmodified official archive as an offline fixture; no Pester dependency.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ArchivePath)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$installer = Join-Path $projectRoot 'scripts/install-cargo-about.ps1'
$fixture = (Resolve-Path -LiteralPath $ArchivePath).Path
$testRoot = Join-Path $projectRoot "target/cargo-about-tests/$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$downloadState = [pscustomobject]@{ Count = 0; Corrupt = $false; Disabled = $false; Fail = $false }

function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
    if ($downloadState.Disabled) { throw 'Unexpected network request on cache hit.' }
    if ($Uri -ne 'https://github.com/EmbarkStudios/cargo-about/releases/download/0.9.2/cargo-about-0.9.2-x86_64-pc-windows-msvc.tar.gz') {
        throw "Unexpected download URL: $Uri"
    }
    $downloadState.Count++
    if ($downloadState.Fail) {
        [IO.File]::WriteAllText($OutFile, 'partial download')
        throw 'Simulated download failure.'
    }
    elseif ($downloadState.Corrupt) { [IO.File]::WriteAllText($OutFile, 'invalid archive') }
    else { Copy-Item -LiteralPath $fixture -Destination $OutFile }
}

# A cold install must extract and run the real official binary.
$installDirectory = Join-Path $testRoot 'bin'
& $installer -InstallDirectory $installDirectory
$executable = Join-Path $installDirectory 'cargo-about.exe'
if ($downloadState.Count -ne 1 -or (& $executable --version) -ne 'cargo-about 0.9.2') {
    throw 'Cold installation did not provide cargo-about 0.9.2.'
}
Write-Host 'PASS: cold installation and official binary version.'

# A restored binary must work without any download.
$downloadState.Disabled = $true
& $installer -InstallDirectory $installDirectory
if ($downloadState.Count -ne 1) { throw 'Cache hit unexpectedly downloaded an archive.' }
Write-Host 'PASS: cache hit without network access.'

# Invalid downloaded content must fail before extraction or installation.
$downloadState.Disabled = $false
$downloadState.Corrupt = $true
$rejectedDirectory = Join-Path $testRoot 'rejected'
$rejected = $false
try { & $installer -InstallDirectory $rejectedDirectory }
catch {
    if ($_.Exception.Message -notlike '*archive SHA-256 mismatch*') { throw }
    $rejected = $true
}
if (-not $rejected -or (Test-Path -LiteralPath (Join-Path $rejectedDirectory 'cargo-about.exe'))) {
    throw 'Invalid archive was not rejected before installation.'
}
Write-Host 'PASS: incorrect archive hash blocks installation.'

# A corrupt cache must not be executed and must be repaired from a verified archive.
$downloadState.Corrupt = $false
[IO.File]::WriteAllText($executable, 'invalid executable')
& $installer -InstallDirectory $installDirectory
if ($downloadState.Count -ne 3 -or (& $executable --version) -ne 'cargo-about 0.9.2') {
    throw 'Corrupt cache was not repaired.'
}
Write-Host 'PASS: corrupt cache is replaced by the verified official binary.'

# Exercise the CI default destination without changing the user's Cargo installation.
$savedCargoHome = $env:CARGO_HOME
try {
    $env:CARGO_HOME = Join-Path $testRoot 'cargo-home'
    & $installer
    $defaultExecutable = Join-Path $env:CARGO_HOME 'bin/cargo-about.exe'
    if (-not (Test-Path -LiteralPath $defaultExecutable) -or
        (& $defaultExecutable --version) -ne 'cargo-about 0.9.2' -or $LASTEXITCODE -ne 0) {
        throw 'Default installation did not provide cargo-about in CARGO_HOME/bin.'
    }
    Write-Host 'PASS: default CARGO_HOME installation destination and version.'
} finally { $env:CARGO_HOME = $savedCargoHome }

$savedUserProfile = $env:USERPROFILE
try {
    $env:CARGO_HOME = $null
    $env:USERPROFILE = Join-Path $testRoot 'fallback profile with spaces'
    & $installer
    $fallbackExecutable = Join-Path $env:USERPROFILE '.cargo/bin/cargo-about.exe'
    if (-not (Test-Path -LiteralPath $fallbackExecutable) -or
        (& $fallbackExecutable --version) -ne 'cargo-about 0.9.2' -or $LASTEXITCODE -ne 0) {
        throw 'Missing CARGO_HOME did not fall back to USERPROFILE/.cargo/bin.'
    }
    Write-Host 'PASS: missing CARGO_HOME falls back to USERPROFILE/.cargo/bin.'
} finally {
    $env:CARGO_HOME = $savedCargoHome
    $env:USERPROFILE = $savedUserProfile
}

# A cache restored into a fresh directory needs no cargo-install metadata or download.
$restoredDirectory = Join-Path $testRoot 'restored cache with spaces/bin'
New-Item -ItemType Directory -Path $restoredDirectory -Force | Out-Null
Copy-Item -LiteralPath $executable -Destination $restoredDirectory
$downloadState.Disabled = $true
& $installer -InstallDirectory $restoredDirectory
$downloadState.Disabled = $false
Write-Host 'PASS: restored binary works offline from a path containing spaces.'

function Assert-RejectedInstallation([string]$Directory, [string]$ErrorPattern) {
    $rejected = $false
    try { & $installer -InstallDirectory $Directory }
    catch {
        if ($_.Exception.Message -notlike $ErrorPattern) { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw "Installation unexpectedly succeeded: $Directory" }
}

$savedTemp = $env:TEMP
$savedTmp = $env:TMP
$failureTemp = Join-Path $testRoot 'failure-temp'
New-Item -ItemType Directory -Path $failureTemp | Out-Null
try {
    $env:TEMP = $failureTemp
    $env:TMP = $failureTemp
    # A failed download must preserve an older installation and clean partial files.
    $previousDirectory = Join-Path $testRoot 'previous-bin'
    New-Item -ItemType Directory -Path $previousDirectory | Out-Null
    $previousBinary = Join-Path $previousDirectory 'cargo-about.exe'
    [IO.File]::WriteAllText($previousBinary, 'previous installation')
    $previousHash = (Get-FileHash -LiteralPath $previousBinary -Algorithm SHA256).Hash
    $downloadState.Fail = $true
    Assert-RejectedInstallation $previousDirectory '*Simulated download failure*'
    $downloadState.Fail = $false
    if ((Get-FileHash -LiteralPath $previousBinary -Algorithm SHA256).Hash -ne $previousHash) {
        throw 'Failed download modified the previous installation.'
    }
    Write-Host 'PASS: interrupted download preserves the previous installation.'

    $nativeTar = (Get-Command tar -CommandType Application).Source
    $extractionState = [pscustomobject]@{ Fail = $true; Corrupt = $false }
    function tar {
        if ($extractionState.Fail) { $global:LASTEXITCODE = 23; return }
        & $nativeTar @args
        if ($extractionState.Corrupt -and $LASTEXITCODE -eq 0) {
            $destination = $args[[Array]::IndexOf($args, '-C') + 1]
            $extracted = Join-Path $destination 'cargo-about-0.9.2-x86_64-pc-windows-msvc/cargo-about.exe'
            [IO.File]::AppendAllText($extracted, 'tampered extraction')
        }
    }
    $extractionDirectory = Join-Path $testRoot 'extraction-rejected'
    Assert-RejectedInstallation $extractionDirectory '*extraction failed*'
    if (Test-Path -LiteralPath (Join-Path $extractionDirectory 'cargo-about.exe')) {
        throw 'Extraction failure still installed a binary.'
    }
    Write-Host 'PASS: extraction failure blocks installation.'

    $extractionState.Fail = $false
    $extractionState.Corrupt = $true
    $binaryDirectory = Join-Path $testRoot 'binary-rejected'
    Assert-RejectedInstallation $binaryDirectory '*binary SHA-256 mismatch*'
    if (Test-Path -LiteralPath (Join-Path $binaryDirectory 'cargo-about.exe')) {
        throw 'Tampered extraction still installed a binary.'
    }
    Write-Host 'PASS: extracted binary hash is checked before installation.'
    if (@(Get-ChildItem -LiteralPath $failureTemp -Force).Count -ne 0) {
        throw 'Failed installation left temporary files behind.'
    }
    Write-Host 'PASS: all failed installations clean their temporary files.'
} finally {
    $env:TEMP = $savedTemp
    $env:TMP = $savedTmp
}
Write-Host "Test fixtures: $testRoot"
