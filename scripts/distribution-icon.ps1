# Regenerate logo.png, the in-app logo PNGs, oxyde.ico and the Linux package PNGs from
# assets/icons/logo_oxyde.svg.
# The generated assets are committed: native windows use the PNG, while build.rs
# embeds the ICO for Explorer, the window class, the taskbar and the installer.
# Worker icons have their own source and are deliberately left untouched.
param([string]$ProjectRoot = (Join-Path $PSScriptRoot '..'), [string]$Destination)
$ErrorActionPreference = 'Stop'
$ProjectRoot = [IO.Path]::GetFullPath($ProjectRoot)
if (-not $Destination) { $Destination = Join-Path $ProjectRoot 'assets/icons/oxyde.ico' }
& cargo run --offline --locked --manifest-path (Join-Path $ProjectRoot 'Cargo.toml') --example generate_application_icons -- $ProjectRoot $Destination
if ($LASTEXITCODE -ne 0) { throw 'Application icon generation failed.' }
