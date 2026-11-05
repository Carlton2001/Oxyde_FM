# Write latest.json in the Tauri updater format, read by both the Tauri edition
# (1.x) and the GPUI updater. Every Windows key points to the Inno Setup file:
# the Tauri updater runs any signed .exe as an installer; its signature is the
# minisign .sig. Linux entries, ignored by Tauri: the per-user archive Oxyde installs
# itself, the DEB and the RPM it offers to download. Their signature is the base64 of
# the armored GPG signature (.asc) made by scripts/sign-linux-release.sh.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Destination,
    [string]$LinuxArchive,
    [string]$LinuxDeb,
    [string]$LinuxRpm,
    [string]$Repository = 'Carlton2001/Oxyde_FM',
    [string]$Notes = 'See the release notes on GitHub.'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-Entry([string]$Path, [string]$Signature) {
    [ordered]@{
        signature = $Signature
        url = "https://github.com/$Repository/releases/download/v$Version/$([IO.Path]::GetFileName($Path))"
    }
}

function Get-SignatureFile([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { throw "Missing updater signature $Path." }
    $Path
}

function Get-WindowsEntry([string]$Path) {
    Get-Entry $Path (Get-Content -LiteralPath (Get-SignatureFile "$Path.sig") -Raw).Trim()
}

function Get-LinuxEntry([string]$Path) {
    Get-Entry $Path ([Convert]::ToBase64String([IO.File]::ReadAllBytes((Get-SignatureFile "$Path.asc"))))
}

$platform = Get-WindowsEntry $Setup
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
if ($LinuxArchive) { $manifest.platforms['linux-x86_64'] = Get-LinuxEntry $LinuxArchive }
if ($LinuxDeb) { $manifest.platforms['linux-x86_64-deb'] = Get-LinuxEntry $LinuxDeb }
if ($LinuxRpm) { $manifest.platforms['linux-x86_64-rpm'] = Get-LinuxEntry $LinuxRpm }
[IO.File]::WriteAllText($Destination, ($manifest | ConvertTo-Json -Depth 4) + "`n")
Write-Host "Update manifest: $Destination"
