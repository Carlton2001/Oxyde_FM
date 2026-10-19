# Build the classic installer and a folder-based portable ZIP.
[CmdletBinding()]
param(
    [string]$IsccPath,
    [string]$CrtDirectory,
    [switch]$SkipBuild,
    [switch]$PortableOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$target = 'x86_64-pc-windows-msvc'
$release = Join-Path $projectRoot "target/$target/release"
$work = Join-Path $projectRoot 'target/distribution'
$stage = Join-Path $work 'payload'
$output = Join-Path $work 'output'

if (-not $PortableOnly) {
if (-not $IsccPath) {
    $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($compiler) { $IsccPath = $compiler.Source }
    else {
        foreach ($root in @($env:LOCALAPPDATA, ${env:ProgramFiles(x86)}, $env:ProgramFiles)) {
            $candidate = if ($root -eq $env:LOCALAPPDATA) {
                Join-Path $root 'Programs/Inno Setup 6/ISCC.exe'
            } else { Join-Path $root 'Inno Setup 6/ISCC.exe' }
            if (Test-Path -LiteralPath $candidate) { $IsccPath = $candidate; break }
        }
    }
}
if (-not $IsccPath -or -not (Test-Path -LiteralPath $IsccPath)) {
    throw 'Install Inno Setup 6.3 or newer, or pass -IsccPath.'
}
}
if (-not $CrtDirectory) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Pass -CrtDirectory with the x64 Microsoft.VC143.CRT redistributable directory.' }
    $visualStudio = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or -not $visualStudio) { throw 'Visual C++ Build Tools not found.' }
    $redist = Join-Path $visualStudio 'VC/Redist/MSVC'
    $versions = Get-ChildItem -LiteralPath $redist -Directory |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } |
        Sort-Object { [version]$_.Name } -Descending
    foreach ($version in $versions) {
        $candidate = Join-Path $version.FullName 'x64/Microsoft.VC143.CRT'
        if (Test-Path -LiteralPath $candidate) { $CrtDirectory = $candidate; break }
    }
}
foreach ($required in @('vcruntime140.dll', 'msvcp140.dll')) {
    if (-not $CrtDirectory -or -not (Test-Path -LiteralPath (Join-Path $CrtDirectory $required))) {
        throw 'The x64 Microsoft.VC143.CRT redistributable files are missing.'
    }
}

Push-Location -LiteralPath $projectRoot
try {
    if (-not $SkipBuild) {
        & cargo build --locked --release --bins --target $target
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    }
    $metadata = & cargo metadata --offline --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read package version.' }
    $package = ($metadata | ConvertFrom-Json).packages | Where-Object name -eq 'oxyde'
    $version = $package.version
    if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Installer requires a numeric major.minor.patch version.' }
    # Only rebuild our known staging directory, never the install directory.
    if (Test-Path -LiteralPath $stage) {
        $resolved = [IO.Path]::GetFullPath($stage)
        if ($resolved -ne [IO.Path]::GetFullPath((Join-Path $projectRoot 'target/distribution/payload'))) {
            throw 'Unexpected staging path.'
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
    New-Item -ItemType Directory -Path $stage, $output -Force | Out-Null
    foreach ($obsolete in @("Oxyde_GPUI-$version-x64.exe", "Oxyde_GPUI-$version-x64.exe.sha256")) {
        $obsoletePath = Join-Path $output $obsolete
        if (Test-Path -LiteralPath $obsoletePath) { Remove-Item -LiteralPath $obsoletePath }
    }
    Copy-Item -LiteralPath (Join-Path $release 'oxyde.exe') -Destination (Join-Path $stage 'Oxyde_GPUI.exe')
    foreach ($worker in @('oxyde-image-worker', 'oxyde-archive-io-worker', 'oxyde-archive-list-worker')) {
        Copy-Item -LiteralPath (Join-Path $release "$worker.exe") -Destination $stage
    }
    Get-ChildItem -LiteralPath $CrtDirectory -Filter '*.dll' | Copy-Item -Destination $stage
    $syntaxNotices = Join-Path $stage 'licenses/syntaxes'
    New-Item -ItemType Directory -Path $syntaxNotices -Force | Out-Null
    foreach ($notice in @('README.md', 'PowerShell-LICENSE.txt', 'PowerShell-NOTICES.txt', 'SCSS-LICENSE.txt', 'bat-LICENSE-MIT.txt')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot "vendor/syntaxes/$notice") -Destination $syntaxNotices
    }
    & (Join-Path $PSScriptRoot 'distribution-icon.ps1') -ProjectRoot $projectRoot -Destination (Join-Path $stage 'oxyde.ico')
    if (-not $PortableOnly) {
        & $IsccPath "/DAppVersion=$version" "/DPayloadDir=$stage" "/O$output" (Join-Path $projectRoot 'installer/windows/oxyde.iss')
        if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $data = Join-Path $stage 'data'
    New-Item -ItemType Directory -Path $data -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $data 'README.txt'), "Ce dossier active le mode portable. Preferences, session, logs, caches et fichiers temporaires restent ici. Ne le supprimez pas pour conserver le mode portable.`r`n")
    $download = Join-Path $output "Oxyde_GPUI-$version-x64-portable.zip"
    if (Test-Path -LiteralPath $download) { Remove-Item -LiteralPath $download }
    # Include a named root so extraction never scatters executables into Downloads.
    $zip = [IO.Compression.ZipFile]::Open($download, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($file in (Get-ChildItem -LiteralPath $stage -Recurse -File)) {
            $relative = $file.FullName.Substring($stage.Length + 1).Replace('\', '/')
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, "Oxyde_GPUI/$relative", [IO.Compression.CompressionLevel]::Optimal) | Out-Null
        }
    } finally { $zip.Dispose() }
    $archive = [IO.Compression.ZipFile]::OpenRead($download)
    try {
        foreach ($name in @('Oxyde_GPUI.exe', 'oxyde-image-worker.exe', 'oxyde-archive-io-worker.exe', 'oxyde-archive-list-worker.exe', 'vcruntime140.dll', 'data/README.txt', 'licenses/syntaxes/PowerShell-LICENSE.txt', 'licenses/syntaxes/PowerShell-NOTICES.txt', 'licenses/syntaxes/SCSS-LICENSE.txt', 'licenses/syntaxes/bat-LICENSE-MIT.txt')) {
            if (-not $archive.GetEntry("Oxyde_GPUI/$name")) { throw "Portable archive is missing $name" }
        }
        if ($archive.Entries | Where-Object { $_.Name -match 'Setup|launcher' }) { throw 'Unexpected installer or launcher in portable archive.' }
    } finally { $archive.Dispose() }
    $hash = (Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$download.sha256", "$hash  $([IO.Path]::GetFileName($download))`n")
    Write-Host "Portable ZIP: $download"
} finally { Pop-Location }
