# Start the local diagnostic build without changing the caller's environment.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$executablePath = Join-Path $projectRoot 'target\debug\oxyde.exe'
if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
    throw 'Build Oxyde first: cargo build --offline --bins'
}
$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $executablePath
$startInfo.WorkingDirectory = $projectRoot
$startInfo.UseShellExecute = $false
$startInfo.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
$startInfo.EnvironmentVariables['OXYDE_MEMORY_DIAGNOSTICS'] = '1'
$startInfo.EnvironmentVariables['RUST_LOG'] = 'warn,gpui_windows::directx_atlas=debug'
$process = [System.Diagnostics.Process]::Start($startInfo)
Write-Output "Oxyde memory diagnostics started; PID=$($process.Id). Samples are in oxyde.log."
