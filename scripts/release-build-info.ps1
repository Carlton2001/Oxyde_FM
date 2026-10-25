# Describe the distribution files built for one commit, or check them before a release.
# The release reuses these files only when commit, version and SHA-256 all match.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, ParameterSetName = 'Write')][switch]$Write,
    [Parameter(Mandatory = $true, ParameterSetName = 'Verify')][switch]$Verify,
    [Parameter(Mandatory = $true)][string]$Directory,
    [Parameter(Mandatory = $true)][string]$Commit,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(ParameterSetName = 'Write')][string]$RunId = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($Commit -notmatch '^[0-9a-f]{40}$') { throw "Invalid commit SHA: $Commit" }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Invalid version: $Version" }
$infoPath = Join-Path $Directory 'build-info.json'
$names = @(
    "Oxyde_FM_${Version}_x64-setup.exe",
    "Oxyde_FM-$Version-x64-portable.zip",
    "Oxyde_FM-$Version-x64-portable.zip.sha256"
)

function Get-Sha256([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Missing distribution file: $Path" }
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

if ($Write) {
    $files = [ordered]@{}
    foreach ($name in $names) { $files[$name] = Get-Sha256 (Join-Path $Directory $name) }
    $info = [ordered]@{ commit = $Commit; version = $Version; run_id = $RunId; files = $files }
    [IO.File]::WriteAllText($infoPath, ($info | ConvertTo-Json -Depth 3) + "`n")
    Write-Host "Build info: $infoPath"
    return
}

if (-not (Test-Path -LiteralPath $infoPath -PathType Leaf)) { throw 'Missing build-info.json in the distribution artifacts.' }
$info = Get-Content -LiteralPath $infoPath -Raw | ConvertFrom-Json
if ($info.commit -ne $Commit) { throw "Distribution was built from $($info.commit), expected $Commit." }
if ($info.version -ne $Version) { throw "Distribution version is $($info.version), expected $Version." }
$listed = @($info.files.PSObject.Properties.Name | Sort-Object)
if (($listed -join '|') -ne (($names | Sort-Object) -join '|')) { throw 'Build info does not list the expected distribution files.' }
foreach ($name in $names) {
    if ((Get-Sha256 (Join-Path $Directory $name)) -ne $info.files.$name) { throw "SHA-256 mismatch for $name." }
}
# The published checksum file must describe the ZIP actually downloaded.
$zipName = $names[1]
$expectedLine = "$($info.files.$zipName)  $zipName"
if ((Get-Content -LiteralPath (Join-Path $Directory "$zipName.sha256") -Raw).Trim() -ne $expectedLine) {
    throw 'Portable ZIP checksum file does not match the ZIP.'
}
Write-Host "Distribution files match commit $Commit and version $Version."
