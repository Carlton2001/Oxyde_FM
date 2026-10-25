# Check release-commit detection in a temporary Git repository, including a depth-2 clone.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$script = Join-Path $projectRoot 'scripts/detect-release-commit.ps1'
$testRoot = Join-Path $projectRoot "target/detect-release-commit-tests/$([Guid]::NewGuid().ToString('N'))"
$repo = Join-Path $testRoot 'repo'
New-Item -ItemType Directory -Path $repo -Force | Out-Null
$savedOutput = $env:GITHUB_OUTPUT
$env:GITHUB_OUTPUT = $null

function Invoke-Git { & git -C $repo @args | Out-Null; if ($LASTEXITCODE -ne 0) { throw "git $args failed." } }
function Set-Commit([string]$Version, [string]$Subject) {
    [IO.File]::WriteAllText((Join-Path $repo 'Cargo.toml'), "[package]`nname = `"oxyde`"`nversion = `"$Version`"`n")
    [IO.File]::WriteAllText((Join-Path $repo 'change.txt'), [Guid]::NewGuid().ToString())
    Invoke-Git add -A
    Invoke-Git -c user.name=test -c user.email=test@example.invalid commit -q -m $Subject
}
function Assert-Detected([string]$Expected, [string]$Name, [string]$Path = $repo) {
    Push-Location -LiteralPath $Path
    try { $actual = & $script | Select-Object -Last 1 } finally { Pop-Location }
    if ($actual -ne $Expected) { throw "$Name`: expected $Expected, got $actual." }
    Write-Host "PASS: $Name"
}

try {
    Invoke-Git init -q
    Set-Commit '1.0.0' 'Initial'
    Assert-Detected 'false' 'a root commit is not a release.'
    Set-Commit '1.0.1' 'Release v1.0.1'
    Assert-Detected 'true' 'a bump-version commit is a release.'
    Set-Commit '1.0.1' 'Release v1.0.1'
    Assert-Detected 'false' 'a release subject without version change is checked on main.'
    Set-Commit '1.0.2' 'Update dependencies'
    Assert-Detected 'false' 'a version change with another subject is checked on main.'
    Set-Commit '1.0.3' 'Release v1.0.4'
    Assert-Detected 'false' 'a subject naming another version is checked on main.'
    Set-Commit '1.0.5' 'Release v1.0.5'
    $clone = Join-Path $testRoot 'shallow'
    & git clone -q --depth 2 "file:///$($repo.Replace('\', '/'))" $clone
    if ($LASTEXITCODE -ne 0) { throw 'Shallow clone failed.' }
    Assert-Detected 'true' 'a depth-2 checkout detects the release.' $clone
} finally { $env:GITHUB_OUTPUT = $savedOutput }
Write-Host "Test fixtures: $testRoot"
