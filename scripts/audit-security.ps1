# Run from any directory. Tools and the advisory database stay inside target/.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$auditExitCode = 1
Push-Location -LiteralPath $projectRoot
try {
    $toolRoot = Join-Path $projectRoot 'target/security-tools'
    $executableName = if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        'cargo-audit.exe'
    } else {
        'cargo-audit'
    }
    $auditExecutable = Join-Path $toolRoot "bin/$executableName"
    if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        & (Join-Path $PSScriptRoot 'install-cargo-audit.ps1') -InstallDirectory (Join-Path $toolRoot 'bin')
    } elseif (-not (Test-Path -LiteralPath $auditExecutable)) {
        & cargo install cargo-audit --version 0.22.2 --locked --root $toolRoot
        if ($LASTEXITCODE -ne 0) { throw 'cargo-audit installation failed' }
    }
    $reportPath = Join-Path $projectRoot 'target/security-audit-windows.json'
    $phaseTimer = [Diagnostics.Stopwatch]::StartNew()
    $auditOutput = & $auditExecutable audit --db target/security-advisory-db --target-os windows --target-arch x86_64 --deny yanked --json
    $auditExitCode = $LASTEXITCODE
    Write-Host ('Security timing: advisory/index refresh and audit {0:N1} s' -f $phaseTimer.Elapsed.TotalSeconds)
    if (-not $auditOutput) { throw 'cargo-audit did not produce a report' }
    $json = $auditOutput -join [Environment]::NewLine
    $report = $json | ConvertFrom-Json
    [IO.File]::WriteAllText($reportPath, $json, [Text.UTF8Encoding]::new($false))
    Write-Host "Windows x86_64: $($report.vulnerabilities.count) published vulnerabilities."
    foreach ($vulnerability in $report.vulnerabilities.list) {
        $advisory = $vulnerability.advisory
        Write-Host "VULNERABILITY: $($vulnerability.package.name) $($vulnerability.package.version) $($advisory.id) - $($advisory.title)"
        Write-Host "  Advisory: https://rustsec.org/advisories/$($advisory.id).html"
        Write-Host "  Patched versions: $($vulnerability.versions.patched -join ', ')"
    }
    foreach ($category in $report.warnings.PSObject.Properties) {
        foreach ($warning in $category.Value) {
            Write-Host "$($category.Name): $($warning.package.name) $($warning.package.version) $($warning.advisory.id)"
        }
    }
    Write-Host "JSON report: $reportPath"
} finally {
    Pop-Location
}
exit $auditExitCode
