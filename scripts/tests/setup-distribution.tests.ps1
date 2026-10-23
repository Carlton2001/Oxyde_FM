# Check the actual composite action without changing the user's Rustup installation.
# -RealToolchain also downloads the pinned toolchain into an isolated test directory.
[CmdletBinding()]
param([switch]$RealToolchain)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$actionPath = Join-Path $projectRoot '.github/actions/setup-distribution/action.yml'
$action = [IO.File]::ReadAllText($actionPath)
$match = [regex]::Match($action, '(?ms)^      run: \|\r?\n(?<script>(?:        [^\r\n]*\r?\n)+)')
if (-not $match.Success) { throw 'Distribution setup script not found in the composite action.' }
$setup = [scriptblock]::Create(($match.Groups['script'].Value -replace '(?m)^        ', ''))
$testRoot = Join-Path $projectRoot "target/distribution-setup-tests/$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$variables = @('RUNNER_TOOL_CACHE', 'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN', 'GITHUB_ENV')
$savedEnvironment = @{}
foreach ($name in $variables) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name) }
$toolchain = '1.98.1-x86_64-pc-windows-msvc'
$state = [pscustomobject]@{ GlobalVersion = ''; InstallFails = $false; ListFails = $false; ExtraToolchain = $false; Calls = @() }

function rustup {
    $state.Calls += ,@($args)
    $global:LASTEXITCODE = 0
    if ($env:RUSTUP_HOME -eq 'simulated-global-rustup') {
        throw 'The action accessed the runner default Rustup home.'
    }
    switch ($args[1]) {
        'install' {
            if (($args -join ' ') -ne "toolchain install $toolchain --profile minimal --no-self-update") {
                throw "Unexpected install arguments: $args"
            }
            if ($state.InstallFails) { $global:LASTEXITCODE = 1 }
        }
        'list' {
            if (($args -join ' ') -ne 'toolchain list --quiet') { throw "Unexpected list arguments: $args" }
            if ($state.ListFails) { $global:LASTEXITCODE = 1; return }
            $toolchain
            if ($state.ExtraToolchain) { "stable-$($state.GlobalVersion)" }
        }
        default { throw "Unexpected Rustup operation: $args" }
    }
}

function Reset-Fixture([string]$Name) {
    $env:RUNNER_TOOL_CACHE = Join-Path $testRoot 'runner tools with spaces'
    $env:RUSTUP_HOME = 'simulated-global-rustup'
    $env:RUSTUP_TOOLCHAIN = 'stable'
    $env:GITHUB_ENV = Join-Path $testRoot "$Name.env"
    $state.Calls = @()
}

try {
    $exports = @()
    foreach ($version in @('1.99.0', '1.99.1')) {
        Reset-Fixture $version
        $state.GlobalVersion = $version
        & $setup
        $values = [IO.File]::ReadAllText($env:GITHUB_ENV)
        $expectedHome = Join-Path $env:RUNNER_TOOL_CACHE "oxyde-distribution-rustup/$toolchain"
        if ($env:RUSTUP_HOME -ne $expectedHome -or $env:RUSTUP_TOOLCHAIN -ne $toolchain -or
            -not $values.Contains("RUSTUP_HOME=$expectedHome") -or
            -not $values.Contains("RUSTUP_TOOLCHAIN=$toolchain") -or $state.Calls.Count -ne 2) {
            throw 'Isolated Rustup environment was not selected and exported.'
        }
        $exports += $values
    }
    if ($exports[0] -ne $exports[1]) { throw 'Runner default Rust version changed the distribution environment.' }
    Write-Host 'PASS: runner defaults 1.99.0 and 1.99.1 produce identical isolated environments.'

    foreach ($failure in @('InstallFails', 'ListFails', 'ExtraToolchain')) {
        Reset-Fixture $failure
        $state.$failure = $true
        $rejected = $false
        try { & $setup }
        catch {
            if ($_.Exception.Message -notmatch 'installation failed|must contain only') { throw }
            $rejected = $true
        }
        $state.$failure = $false
        if (-not $rejected -or (Test-Path -LiteralPath $env:GITHUB_ENV)) {
            throw "Setup did not stop before exporting an invalid environment: $failure"
        }
        if ($failure -eq 'InstallFails' -and $state.Calls.Count -ne 1) { throw 'Failed installation continued.' }
        Write-Host "PASS: $failure stops setup before exporting the environment."
    }

    Reset-Fixture 'missing-runner-directory'
    $env:RUNNER_TOOL_CACHE = $null
    $rejected = $false
    try { & $setup } catch {
        if ($_.Exception.Message -ne 'Missing runner tool cache directory.') { throw }
        $rejected = $true
    }
    if (-not $rejected -or $state.Calls.Count -ne 0) { throw 'Missing runner directory did not stop setup.' }
    Write-Host 'PASS: missing runner directory fails before calling Rustup.'

    Remove-Item Function:\rustup
    if ($RealToolchain) {
        foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name]) }
        $before = @(rustup toolchain list --quiet)
        Reset-Fixture 'real'
        # Keep MSVC library paths below MAX_PATH, as on the hosted runner's short tool cache.
        $env:RUNNER_TOOL_CACHE = Join-Path $projectRoot "target/rt $([Guid]::NewGuid().ToString('N').Substring(0, 8))"
        # Reset-Fixture's global home is a sentinel used only by mocked calls.
        & $setup
        $installed = @(rustup toolchain list --quiet)
        $rustVersion = & rustc -V
        if ($LASTEXITCODE -ne 0 -or $installed.Count -ne 1 -or $installed[0] -ne $toolchain -or
            $rustVersion -notlike 'rustc 1.98.1 *') { throw 'Real isolated Rustup selection failed.' }
        $cargoVersion = & cargo -V
        if ($LASTEXITCODE -ne 0 -or $cargoVersion -notlike 'cargo 1.98.1 *') {
            throw 'Cargo did not use the isolated toolchain.'
        }
        $sysroot = & rustc --print sysroot
        if ($LASTEXITCODE -ne 0 -or -not $sysroot.StartsWith($env:RUSTUP_HOME, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'rustc resolved outside the isolated Rustup home.'
        }
        # A second setup must keep the same environment and reuse the installed toolchain.
        $firstExport = [IO.File]::ReadAllText($env:GITHUB_ENV)
        $env:GITHUB_ENV = Join-Path $testRoot 'real-second.env'
        & $setup
        if ([IO.File]::ReadAllText($env:GITHUB_ENV) -ne $firstExport) { throw 'Repeated setup changed the environment.' }
        Write-Host 'PASS: real pinned toolchain installation, rustc/Cargo invocation and repeated setup.'
        foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name]) }
        $after = @(rustup toolchain list --quiet)
        if (($before -join "`n") -ne ($after -join "`n")) { throw 'The default Rustup installation changed.' }
        Write-Host 'PASS: the default Rustup installation is unchanged.'
    }
} finally {
    Remove-Item Function:\rustup -ErrorAction SilentlyContinue
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name]) }
}
Write-Host "Test fixtures: $testRoot"
