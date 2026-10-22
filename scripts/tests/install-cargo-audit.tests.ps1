# Use an unmodified official ZIP as an offline fixture; no Pester dependency.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ArchivePath)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$installer = Join-Path $projectRoot 'scripts/install-cargo-audit.ps1'
$fixture = (Resolve-Path -LiteralPath $ArchivePath).Path
$testRoot = Join-Path $projectRoot "target/cargo-audit-tests/$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$downloadState = [pscustomobject]@{ Count = 0; Corrupt = $false; Disabled = $false; Fail = $false }

function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
    if ($downloadState.Disabled) { throw 'Unexpected network request on cache hit.' }
    if ($Uri -ne 'https://github.com/rustsec/rustsec/releases/download/cargo-audit/v0.22.2/cargo-audit-x86_64-pc-windows-msvc-v0.22.2.zip') {
        throw "Unexpected download URL: $Uri"
    }
    $downloadState.Count++
    if ($downloadState.Fail) {
        [IO.File]::WriteAllText($OutFile, 'partial download')
        throw 'Simulated download failure.'
    } elseif ($downloadState.Corrupt) { [IO.File]::WriteAllText($OutFile, 'invalid archive') }
    else { Copy-Item -LiteralPath $fixture -Destination $OutFile }
}

function Assert-RejectedInstallation([string]$Script, [string]$Directory, [string]$ErrorPattern) {
    $rejected = $false
    try { & $Script -InstallDirectory $Directory }
    catch {
        if ($_.Exception.Message -notlike $ErrorPattern) { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw "Installation unexpectedly succeeded: $Directory" }
}

$savedTemp = $env:TEMP
$savedTmp = $env:TMP
$temporaryRoot = Join-Path $testRoot 'temporary'
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
try {
    $env:TEMP = $temporaryRoot
    $env:TMP = $temporaryRoot
    $installDirectory = Join-Path $testRoot 'bin with spaces'
    & $installer -InstallDirectory $installDirectory
    $executable = Join-Path $installDirectory 'cargo-audit.exe'
    if ($downloadState.Count -ne 1 -or (& $executable --version) -ne 'cargo-audit 0.22.2') {
        throw 'Cold installation did not provide cargo-audit 0.22.2.'
    }
    Write-Host 'PASS: cold installation and official binary version, path with spaces.'

    $downloadState.Disabled = $true
    & $installer -InstallDirectory $installDirectory
    if ($downloadState.Count -ne 1) { throw 'Cache hit unexpectedly downloaded an archive.' }
    Write-Host 'PASS: verified binary is reused without network access.'

    $downloadState.Disabled = $false
    $downloadState.Corrupt = $true
    $rejectedDirectory = Join-Path $testRoot 'rejected'
    Assert-RejectedInstallation $installer $rejectedDirectory '*archive SHA-256 mismatch*'
    if (Test-Path -LiteralPath (Join-Path $rejectedDirectory 'cargo-audit.exe')) {
        throw 'Invalid archive still installed an executable.'
    }
    Write-Host 'PASS: incorrect archive hash blocks installation.'

    $downloadState.Corrupt = $false
    [IO.File]::WriteAllText($executable, 'invalid executable')
    & $installer -InstallDirectory $installDirectory
    if ($downloadState.Count -ne 3 -or (& $executable --version) -ne 'cargo-audit 0.22.2') {
        throw 'Corrupt cache was not repaired.'
    }
    Write-Host 'PASS: corrupt cached executable is repaired before execution.'

    $downloadState.Fail = $true
    $previousHash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash
    # Force a download while preserving the previous file to check failure atomicity.
    $installerText = [IO.File]::ReadAllText($installer)
    $badBinaryHashScript = Join-Path $testRoot 'bad-binary-hash.ps1'
    [IO.File]::WriteAllText($badBinaryHashScript, $installerText.Replace(
        '0157F5CE1CE9FD4FB0A1F7C79AF1229771D1F80B6C2613DDB0D9200A8BA73946', ('0' * 64)))
    Assert-RejectedInstallation $badBinaryHashScript $installDirectory '*Simulated download failure*'
    if ((Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash -ne $previousHash) {
        throw 'Interrupted download modified the previous installation.'
    }
    Write-Host 'PASS: interrupted download preserves the previous installation.'

    $downloadState.Fail = $false
    Assert-RejectedInstallation $badBinaryHashScript $rejectedDirectory '*binary SHA-256 mismatch*'
    if (Test-Path -LiteralPath (Join-Path $rejectedDirectory 'cargo-audit.exe')) {
        throw 'Incorrect extracted binary hash still installed an executable.'
    }
    Write-Host 'PASS: extracted executable hash is checked before execution.'

    # Default destination stays under the project, even when called from another directory.
    $fixtureScripts = Join-Path $testRoot 'default-project/scripts'
    New-Item -ItemType Directory -Path $fixtureScripts -Force | Out-Null
    $fixtureInstaller = Join-Path $fixtureScripts 'install-cargo-audit.ps1'
    Copy-Item -LiteralPath $installer -Destination $fixtureInstaller
    Push-Location -LiteralPath $temporaryRoot
    try { & $fixtureInstaller } finally { Pop-Location }
    $defaultExecutable = Join-Path $testRoot 'default-project/target/security-tools/bin/cargo-audit.exe'
    if ((& $defaultExecutable --version) -ne 'cargo-audit 0.22.2') {
        throw 'Default project-local installation failed.'
    }
    Write-Host 'PASS: default project-local destination is independent of the current directory.'

    if (@(Get-ChildItem -LiteralPath $temporaryRoot -Force).Count -ne 0) {
        throw 'Installation left temporary files behind.'
    }
    Write-Host 'PASS: successful and failed installations clean their temporary files.'
} finally {
    $env:TEMP = $savedTemp
    $env:TMP = $savedTmp
}
Write-Host "Test fixtures: $testRoot"
