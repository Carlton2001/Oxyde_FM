# Regenerate assets/icons/oxyde.ico from the logo. The ICO is committed because
# build.rs embeds it in the executable for Explorer, the window and the taskbar.
param([string]$ProjectRoot = (Join-Path $PSScriptRoot '..'), [string]$Destination)
$ErrorActionPreference = 'Stop'
if (-not $Destination) { $Destination = Join-Path $ProjectRoot 'assets/icons/oxyde.ico' }
Add-Type -AssemblyName System.Drawing
$source = [Drawing.Image]::FromFile((Join-Path $ProjectRoot 'assets/icons/logo.png'))
$sizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
$images = @()
try {
    foreach ($size in $sizes) {
        $bitmap = [Drawing.Bitmap]::new($size, $size)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        $png = [IO.MemoryStream]::new()
        try {
            $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $graphics.DrawImage($source, 0, 0, $size, $size)
            $bitmap.Save($png, [Drawing.Imaging.ImageFormat]::Png)
            $images += , $png.ToArray()
        } finally { $png.Dispose(); $graphics.Dispose(); $bitmap.Dispose() }
    }
} finally { $source.Dispose() }
$writer = [IO.BinaryWriter]::new([IO.File]::Create($Destination))
try {
    # ICO header, one directory entry per PNG frame; 0 denotes a 256 px dimension.
    $writer.Write([UInt16]0); $writer.Write([UInt16]1); $writer.Write([UInt16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $dimension = [byte]($sizes[$i] % 256)
        $writer.Write([byte[]]@($dimension, $dimension, 0, 0))
        $writer.Write([UInt16]1); $writer.Write([UInt16]32)
        $writer.Write([UInt32]$images[$i].Length); $writer.Write([UInt32]$offset)
        $offset += $images[$i].Length
    }
    foreach ($image in $images) { $writer.Write($image) }
} finally { $writer.Dispose() }
