# Run the same native checks locally and in CI. Workers need explicit builds.
[CmdletBinding()]
param([switch]$Offline)

$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'These tests require native Windows workers.'
}
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$savedTemp = $env:TEMP
$savedTmp = $env:TMP
$cargoArguments = @('--locked')
if ($Offline) { $cargoArguments += '--offline' }

Push-Location -LiteralPath $projectRoot
try {
    $fixtureRoot = Join-Path $projectRoot "target/native-tests/$([Guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $fixtureRoot -Force | Out-Null
    $env:TEMP = $fixtureRoot
    $env:TMP = $fixtureRoot
    Write-Host "Native test fixtures: $fixtureRoot"

    & cargo check @cargoArguments --all-targets
    if ($LASTEXITCODE -ne 0) { throw 'All-target compilation failed.' }
    & cargo build @cargoArguments --bins
    if ($LASTEXITCODE -ne 0) { throw 'Worker/application build failed.' }
    & cargo test @cargoArguments --all-targets -- --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Native tests failed.' }
} finally {
    $env:TEMP = $savedTemp
    $env:TMP = $savedTmp
    Pop-Location
}
