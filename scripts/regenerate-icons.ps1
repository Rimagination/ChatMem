Add-Type -AssemblyName System.Drawing

$Root = Split-Path -Parent $PSScriptRoot
$IconDir = Join-Path $Root "src-tauri\icons"
$SourceIconPath = Join-Path $IconDir "icon-source-v0.1.5-memory-ring.png"

function New-ChatMemBitmap {
  param([Parameter(Mandatory = $true)][int]$Size)

  if (-not (Test-Path $SourceIconPath)) {
    throw "Missing ChatMem icon source: $SourceIconPath"
  }

  $source = [System.Drawing.Image]::FromFile($SourceIconPath)
  $bitmap = New-Object System.Drawing.Bitmap $Size, $Size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $graphics.Clear([System.Drawing.Color]::Transparent)
  $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
  $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
  $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $graphics.DrawImage($source, 0, 0, $Size, $Size)
  $graphics.Dispose()
  $source.Dispose()
  return $bitmap
}

function Save-Png {
  param(
    [Parameter(Mandatory = $true)][int]$Size,
    [Parameter(Mandatory = $true)][string]$Name
  )

  $bitmap = New-ChatMemBitmap -Size $Size
  $path = Join-Path $IconDir $Name
  $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bitmap.Dispose()
}

Save-Png -Size 32 -Name "32x32.png"
Save-Png -Size 128 -Name "128x128.png"
Save-Png -Size 256 -Name "128x128@2x.png"
Save-Png -Size 512 -Name "icon.png"
Save-Png -Size 30 -Name "Square30x30Logo.png"
Save-Png -Size 44 -Name "Square44x44Logo.png"
Save-Png -Size 50 -Name "StoreLogo.png"
Save-Png -Size 71 -Name "Square71x71Logo.png"
Save-Png -Size 89 -Name "Square89x89Logo.png"
Save-Png -Size 107 -Name "Square107x107Logo.png"
Save-Png -Size 142 -Name "Square142x142Logo.png"
Save-Png -Size 150 -Name "Square150x150Logo.png"
Save-Png -Size 284 -Name "Square284x284Logo.png"
Save-Png -Size 310 -Name "Square310x310Logo.png"

$icoSizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
$pngPayloads = New-Object System.Collections.Generic.List[byte[]]
foreach ($size in $icoSizes) {
  $bitmap = New-ChatMemBitmap -Size $size
  $stream = New-Object System.IO.MemoryStream
  $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
  $pngPayloads.Add($stream.ToArray())
  $stream.Dispose()
  $bitmap.Dispose()
}

$icoPath = Join-Path $IconDir "icon.ico"
$file = [System.IO.File]::Open($icoPath, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write)
$writer = New-Object System.IO.BinaryWriter $file
$writer.Write([UInt16]0)
$writer.Write([UInt16]1)
$writer.Write([UInt16]$icoSizes.Length)
$offset = 6 + (16 * $icoSizes.Length)
for ($i = 0; $i -lt $icoSizes.Length; $i++) {
  $size = [int]$icoSizes[$i]
  $payload = $pngPayloads[$i]
  if ($size -eq 256) {
    $sizeByte = [byte]0
  } else {
    $sizeByte = [byte]$size
  }
  $writer.Write($sizeByte)
  $writer.Write($sizeByte)
  $writer.Write([byte]0)
  $writer.Write([byte]0)
  $writer.Write([UInt16]1)
  $writer.Write([UInt16]32)
  $writer.Write([UInt32]$payload.Length)
  $writer.Write([UInt32]$offset)
  $offset += $payload.Length
}
foreach ($payload in $pngPayloads) {
  $writer.Write($payload)
}
$writer.Flush()
$writer.Dispose()
$file.Dispose()

Write-Host "Regenerated ChatMem icons in $IconDir"
