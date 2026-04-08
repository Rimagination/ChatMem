param(
  [string]$Version = "0.1.0"
)

$Root = Split-Path -Parent $PSScriptRoot
$PortableRoot = Join-Path $Root "dist-portable\ChatMem"
$ZipPath = Join-Path $Root "dist-portable\ChatMem-v$Version-portable.zip"
$ExePath = Join-Path $Root "src-tauri\target\release\ChatMem.exe"
$UsagePath = Join-Path $Root "README.md"
$PortableUsagePath = Join-Path $PortableRoot "使用说明.txt"

New-Item -ItemType Directory -Force -Path $PortableRoot | Out-Null

Get-ChildItem -LiteralPath $PortableRoot -Force | Remove-Item -Recurse -Force

Copy-Item -LiteralPath $ExePath -Destination (Join-Path $PortableRoot "ChatMem.exe") -Force
Copy-Item -LiteralPath $UsagePath -Destination $PortableUsagePath -Force

if (Test-Path -LiteralPath $ZipPath) {
  Remove-Item -LiteralPath $ZipPath -Force
}

Compress-Archive -Path (Join-Path $PortableRoot "*") -DestinationPath $ZipPath
