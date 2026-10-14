param([string]$ProjectRoot, [string]$Destination)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$source = [Drawing.Image]::FromFile((Join-Path $ProjectRoot 'assets/icons/logo.png'))
$bitmap = [Drawing.Bitmap]::new(256, 256)
$graphics = [Drawing.Graphics]::FromImage($bitmap)
$png = [IO.MemoryStream]::new()
try {
    $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.DrawImage($source, 0, 0, 256, 256)
    $bitmap.Save($png, [Drawing.Imaging.ImageFormat]::Png)
    $writer = [IO.BinaryWriter]::new([IO.File]::Create($Destination))
    try {
        # ICO container around the existing logo; 0 denotes a 256 px dimension.
        $writer.Write([UInt16]0); $writer.Write([UInt16]1); $writer.Write([UInt16]1)
        $writer.Write([byte[]]@(0, 0, 0, 0))
        $writer.Write([UInt16]1); $writer.Write([UInt16]32)
        $writer.Write([UInt32]$png.Length); $writer.Write([UInt32]22)
        $writer.Write($png.ToArray())
    } finally { $writer.Dispose() }
} finally {
    $png.Dispose(); $graphics.Dispose(); $bitmap.Dispose(); $source.Dispose()
}
