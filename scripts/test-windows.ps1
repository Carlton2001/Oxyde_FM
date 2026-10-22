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

    $phaseTimer = [Diagnostics.Stopwatch]::StartNew()
    & cargo check @cargoArguments --all-targets
    if ($LASTEXITCODE -ne 0) { throw 'All-target compilation failed.' }
    Write-Host ('Native timing: all-target check {0:N1} s' -f $phaseTimer.Elapsed.TotalSeconds)
    $phaseTimer.Restart()
    & cargo build @cargoArguments --bins
    if ($LASTEXITCODE -ne 0) { throw 'Worker/application build failed.' }
    Write-Host ('Native timing: worker/application build {0:N1} s' -f $phaseTimer.Elapsed.TotalSeconds)
    $phaseTimer.Restart()
    & cargo test @cargoArguments --all-targets -- --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Native tests failed.' }
    Write-Host ('Native timing: test compilation and execution {0:N1} s' -f $phaseTimer.Elapsed.TotalSeconds)
} finally {
    $env:TEMP = $savedTemp
    $env:TMP = $savedTmp
    Pop-Location
}
