# Tell main-branch workflows whether HEAD only bumps the package version for a release.
# The pushed v<x.y.z> tag runs every check for that commit, so main does not repeat them.
[CmdletBinding()]
param([string]$Revision = 'HEAD')

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-PackageVersion([string]$Commit) {
    $manifest = & git show "${Commit}:Cargo.toml" 2>$null
    if ($LASTEXITCODE -ne 0) { return $null }
    foreach ($line in $manifest) {
        if ($line -match '^version\s*=\s*"([^"]+)"') { return $Matches[1] }
    }
    throw "Package version not found in Cargo.toml at $Commit."
}

$current = Get-PackageVersion $Revision
if (-not $current) { throw "Cannot read Cargo.toml at $Revision." }
$previous = Get-PackageVersion "$Revision^"
$subject = & git log -1 --format=%s $Revision
if ($LASTEXITCODE -ne 0) { throw "Cannot read the subject of $Revision." }
# Both conditions match commits created by scripts/bump-version.ps1.
$release = $previous -and $previous -ne $current -and $subject -eq "Release v$current"
$value = if ($release) { 'true' } else { 'false' }
Write-Host "Release commit: $value (version $previous -> $current)"
if ($env:GITHUB_OUTPUT) { "release=$value" >> $env:GITHUB_OUTPUT }
$value
