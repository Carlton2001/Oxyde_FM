# Write latest.json in the Tauri updater format, read by both the Tauri edition
# (1.x) and the GPUI updater. Every Windows key points to the Inno Setup file:
# the Tauri updater runs any signed .exe as an installer.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Destination,
    [string]$Repository = 'Carlton2001/Oxyde_FM',
    [string]$Notes = 'See the release notes on GitHub.'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$signaturePath = "$Setup.sig"
if (-not (Test-Path -LiteralPath $signaturePath)) { throw "Missing updater signature $signaturePath." }
$signature = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
$name = [IO.Path]::GetFileName($Setup)
$platform = [ordered]@{
    signature = $signature
    url = "https://github.com/$Repository/releases/download/v$Version/$name"
}
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
[IO.File]::WriteAllText($Destination, ($manifest | ConvertTo-Json -Depth 4) + "`n")
Write-Host "Update manifest: $Destination"
