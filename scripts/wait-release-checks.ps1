# Wait for the validations already started by the push of main for the exact release commit.
# The release never reruns them and never takes the latest successful run of another commit.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Commit,
    [string]$Repository = $env:GITHUB_REPOSITORY,
    [string]$Branch = 'main',
    [string[]]$Workflows = @('tests.yml', 'security.yml', 'distribution.yml'),
    [int]$TimeoutMinutes = 120,
    # Runs of a simultaneous main and tag push can be listed a few seconds late.
    [int]$MissingGraceMinutes = 5,
    [int]$PollSeconds = 30
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($Commit -notmatch '^[0-9a-f]{40}$') { throw "Invalid commit SHA: $Commit" }

function Get-WorkflowRun([string]$Workflow) {
    $json = & gh api "repos/$Repository/actions/workflows/$Workflow/runs?head_sha=$Commit&branch=$Branch&per_page=50"
    if ($LASTEXITCODE -ne 0) { throw "Cannot list runs of $Workflow." }
    $runs = @(($json | ConvertFrom-Json).workflow_runs | Where-Object {
        $_.head_sha -eq $Commit -and $_.head_branch -eq $Branch -and $_.event -in @('push', 'workflow_dispatch')
    })
    if ($runs.Count -eq 0) { return $null }
    # A later manual run or re-run of the same commit replaces an earlier result.
    $runs | Sort-Object { [DateTime]$_.created_at }, run_attempt | Select-Object -Last 1
}

$started = [Diagnostics.Stopwatch]::StartNew()
$completed = @{}
while ($true) {
    $pending = @()
    foreach ($workflow in $Workflows) {
        if ($completed.ContainsKey($workflow)) { continue }
        $run = Get-WorkflowRun $workflow
        if (-not $run) {
            if ($started.Elapsed.TotalMinutes -ge $MissingGraceMinutes) {
                throw "No $workflow run on $Branch for $Commit. Push the release commit to $Branch, or run the workflow manually while $Branch points to it."
            }
            $pending += "$workflow (not started)"
            continue
        }
        if ($run.status -ne 'completed') { $pending += "$workflow ($($run.status), $($run.html_url))"; continue }
        if ($run.conclusion -ne 'success') { throw "$workflow concluded $($run.conclusion) for $Commit`: $($run.html_url)" }
        Write-Host "PASS: $workflow $($run.html_url)"
        $completed[$workflow] = $run
    }
    if ($pending.Count -eq 0) { break }
    if ($started.Elapsed.TotalMinutes -ge $TimeoutMinutes) { throw "Timed out waiting for: $($pending -join ', ')" }
    Write-Host ('Waiting {0:N0} min: {1}' -f $started.Elapsed.TotalMinutes, ($pending -join ', '))
    Start-Sleep -Seconds $PollSeconds
}
if ($env:GITHUB_OUTPUT -and $completed.ContainsKey('distribution.yml')) {
    "distribution-run-id=$($completed['distribution.yml'].id)" >> $env:GITHUB_OUTPUT
}
