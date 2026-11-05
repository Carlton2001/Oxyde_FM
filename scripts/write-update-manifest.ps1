# Write latest.json in the Tauri updater format, read by both the Tauri edition
# (1.x) and the GPUI updater. Every Windows key points to the Inno Setup file:
# the Tauri updater runs any signed .exe as an installer. Linux entries, ignored by
# Tauri: the per-user archive Oxyde installs itself, and the DEB it offers to download.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Destination,
    [string]$LinuxArchive,
    [string]$LinuxDeb,
    [string]$Repository = 'Carlton2001/Oxyde_FM',
    [string]$Notes = 'See the release notes on GitHub.'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-Entry([string]$Path) {
    $signaturePath = "$Path.sig"
    if (-not (Test-Path -LiteralPath $signaturePath)) { throw "Missing updater signature $signaturePath." }
    [ordered]@{
        signature = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
        url = "https://github.com/$Repository/releases/download/v$Version/$([IO.Path]::GetFileName($Path))"
    }
}

$platform = Get-Entry $Setup
$manifest = [ordered]@{
    version = $Version
    notes = $Notes
    pub_date = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ss.fffZ')
    platforms = [ordered]@{
        'windows-x86_64' = $platform
        'windows-x86_64-msi' = $platform
        'windows-x86_64-nsis' = $platform
    }
}
if ($LinuxArchive) { $manifest.platforms['linux-x86_64'] = Get-Entry $LinuxArchive }
if ($LinuxDeb) { $manifest.platforms['linux-x86_64-deb'] = Get-Entry $LinuxDeb }
[IO.File]::WriteAllText($Destination, ($manifest | ConvertTo-Json -Depth 4) + "`n")
Write-Host "Update manifest: $Destination"
