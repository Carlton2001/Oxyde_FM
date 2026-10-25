# Check that a release only reuses validated files of its exact commit and version.
# The GitHub API is simulated; no workflow, artifact or release is touched.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$waitScript = Join-Path $projectRoot 'scripts/wait-release-checks.ps1'
$infoScript = Join-Path $projectRoot 'scripts/release-build-info.ps1'
$testRoot = Join-Path $projectRoot "target/release-reuse-tests/$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$commit = 'a' * 40
$other = 'b' * 40
$savedOutput = $env:GITHUB_OUTPUT

function Assert-Fails([scriptblock]$Action, [string]$Pattern, [string]$Name) {
    try { & $Action } catch {
        if ($_.Exception.Message -notmatch $Pattern) { throw "$Name failed for another reason: $($_.Exception.Message)" }
        Write-Host "PASS: $Name"
        return
    }
    throw "$Name was accepted."
}

# Each scenario maps a workflow file to the successive API answers returned by polling.
$global:releaseTestResponses = @{}
function gh {
    $global:LASTEXITCODE = 0
    if ($args[0] -ne 'api' -or $args[1] -notmatch "workflows/(?<file>[^/]+)/runs\?head_sha=$commit&branch=main") {
        throw "Unexpected gh call: $args"
    }
    $queue = $global:releaseTestResponses[$Matches.file]
    $runs = if ($queue.Count -gt 1) { $queue[0]; $global:releaseTestResponses[$Matches.file] = @($queue | Select-Object -Skip 1) } else { $queue[0] }
    @{ workflow_runs = @($runs) } | ConvertTo-Json -Depth 4
}
function New-Run($Id, $Status = 'completed', $Conclusion = 'success', $Sha = $commit, $Branch = 'main', $Event = 'push', $Created = '2026-10-25T10:00:00Z') {
    [ordered]@{ id = $Id; status = $Status; conclusion = $Conclusion; head_sha = $Sha; head_branch = $Branch
        event = $Event; created_at = $Created; run_attempt = 1; html_url = "https://example.invalid/$Id" }
}
function Invoke-Wait { & $waitScript -Commit $commit -Repository 'owner/repo' -PollSeconds 0 -MissingGraceMinutes 0 -TimeoutMinutes 1 }

try {
    # Waiting: in-progress runs are awaited, then the distribution run of this commit is reported.
    $env:GITHUB_OUTPUT = Join-Path $testRoot 'output.txt'
    $global:releaseTestResponses = @{
        'tests.yml' = @(, @(New-Run 1 'in_progress' $null), @(New-Run 1))
        'security.yml' = @(, @(New-Run 2))
        'distribution.yml' = @(, @(New-Run 3 'queued' $null), @(New-Run 3 'in_progress' $null), @(New-Run 3))
    }
    & $waitScript -Commit $commit -Repository 'owner/repo' -PollSeconds 0 -TimeoutMinutes 1
    if ((Get-Content -LiteralPath $env:GITHUB_OUTPUT -Raw).Trim() -ne 'distribution-run-id=3') { throw 'Distribution run id was not exported.' }
    Write-Host 'PASS: in-progress validations are awaited and the distribution run is exported.'

    $global:releaseTestResponses = @{
        'tests.yml' = @(, @((New-Run 10 -Conclusion 'failure' -Created '2026-10-25T10:00:00Z'), (New-Run 11 -Created '2026-10-25T11:00:00Z')))
        'security.yml' = @(, @(New-Run 2)); 'distribution.yml' = @(, @(New-Run 3))
    }
    Remove-Item -LiteralPath $env:GITHUB_OUTPUT
    Invoke-Wait
    Write-Host 'PASS: a later successful re-run of the same commit replaces an earlier failure.'

    $global:releaseTestResponses = @{ 'tests.yml' = @(, @(New-Run 1)); 'security.yml' = @(, @(New-Run 2 -Conclusion 'failure')); 'distribution.yml' = @(, @(New-Run 3)) }
    Assert-Fails { Invoke-Wait } 'security.yml concluded failure' 'a failed audit blocks the release.'
    $global:releaseTestResponses = @{ 'tests.yml' = @(, @(New-Run 1 -Conclusion 'cancelled')); 'security.yml' = @(, @(New-Run 2)); 'distribution.yml' = @(, @(New-Run 3)) }
    Assert-Fails { Invoke-Wait } 'tests.yml concluded cancelled' 'cancelled native tests block the release.'
    # Runs of another commit, another branch or a pull request never count.
    $global:releaseTestResponses = @{
        'tests.yml' = @(, @((New-Run 4 -Sha $other), (New-Run 5 -Branch 'feature'), (New-Run 6 -Event 'pull_request')))
        'security.yml' = @(, @(New-Run 2)); 'distribution.yml' = @(, @(New-Run 3))
    }
    Assert-Fails { Invoke-Wait } 'No tests.yml run on main' 'runs of other commits, branches or pull requests are ignored.'
    $global:releaseTestResponses = @{ 'tests.yml' = @(, @(New-Run 1 'in_progress' $null)); 'security.yml' = @(, @(New-Run 2)); 'distribution.yml' = @(, @(New-Run 3)) }
    Assert-Fails { & $waitScript -Commit $commit -Repository 'owner/repo' -PollSeconds 0 -TimeoutMinutes 0 } 'Timed out' 'an unfinished validation times out.'

    # Build info: exact commit, version and file hashes are required.
    $version = '9.8.7'
    $dir = Join-Path $testRoot 'dist'
    New-Item -ItemType Directory -Path $dir | Out-Null
    $setup = Join-Path $dir "Oxyde_FM_${version}_x64-setup.exe"
    $zip = Join-Path $dir "Oxyde_FM-$version-x64-portable.zip"
    [IO.File]::WriteAllText($setup, 'setup')
    [IO.File]::WriteAllText($zip, 'zip')
    $zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$zip.sha256", "$zipHash  $([IO.Path]::GetFileName($zip))`n")
    & $infoScript -Write -Directory $dir -Commit $commit -Version $version -RunId 3
    & $infoScript -Verify -Directory $dir -Commit $commit -Version $version
    Write-Host 'PASS: matching files are accepted.'
    Assert-Fails { & $infoScript -Verify -Directory $dir -Commit $other -Version $version } 'built from' 'files of another commit are rejected.'
    Assert-Fails { & $infoScript -Verify -Directory $dir -Commit $commit -Version '9.8.8' } 'Missing distribution file|version is' 'files of another version are rejected.'
    [IO.File]::WriteAllText($setup, 'tampered')
    Assert-Fails { & $infoScript -Verify -Directory $dir -Commit $commit -Version $version } 'SHA-256 mismatch' 'a modified setup is rejected.'
    [IO.File]::WriteAllText($setup, 'setup')
    [IO.File]::WriteAllText("$zip.sha256", "$('0' * 64)  $([IO.Path]::GetFileName($zip))`n")
    Assert-Fails { & $infoScript -Verify -Directory $dir -Commit $commit -Version $version } 'SHA-256 mismatch|checksum' 'a stale ZIP checksum file is rejected.'
    Remove-Item -LiteralPath (Join-Path $dir 'build-info.json')
    Assert-Fails { & $infoScript -Verify -Directory $dir -Commit $commit -Version $version } 'Missing build-info' 'files without build info are rejected.'
} finally {
    Remove-Item Function:\gh -ErrorAction SilentlyContinue
    $env:GITHUB_OUTPUT = $savedOutput
    Remove-Variable releaseTestResponses -Scope Global -ErrorAction SilentlyContinue
}
Write-Host "Test fixtures: $testRoot"
