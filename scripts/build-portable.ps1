param(
  [string]$Version = ""
)

$Root = Split-Path -Parent $PSScriptRoot

if ([string]::IsNullOrWhiteSpace($Version)) {
  $PackagePath = Join-Path $Root "package.json"
  $Version = (Get-Content -LiteralPath $PackagePath -Raw | ConvertFrom-Json).version
}

$PortableRoot = Join-Path $Root "dist-portable\ChatMem"
$ZipPath = Join-Path $Root "dist-portable\ChatMem-v$Version-portable.zip"
$ExePath = Join-Path $Root "src-tauri\target\release\chatmem.exe"
$McpExePath = Join-Path $Root "src-tauri\target\release\chatmem-mcp.exe"
$McpScriptPath = Join-Path $Root "mcp\run-chatmem-mcp.ps1"
$SkillPath = Join-Path $Root "skills\chatmem"
$McpDocsPath = Join-Path $Root "docs\CHATMEM_MCP_SETUP.md"
$UsagePath = Join-Path $Root "README.md"
$PortableUsagePath = Join-Path $PortableRoot "README.txt"
$PortableMcpRoot = Join-Path $PortableRoot "mcp"
$PortableSkillRoot = Join-Path $PortableRoot "skills\chatmem"

if (-not (Test-Path -LiteralPath $ExePath)) {
  throw "Release executable not found: $ExePath"
}

if (-not (Test-Path -LiteralPath $McpExePath)) {
  throw "Release MCP executable not found: $McpExePath"
}

if (-not (Test-Path -LiteralPath $McpScriptPath)) {
  throw "MCP launcher script not found: $McpScriptPath"
}

if (-not (Test-Path -LiteralPath $SkillPath)) {
  throw "ChatMem skill directory not found: $SkillPath"
}

New-Item -ItemType Directory -Force -Path $PortableRoot | Out-Null

Get-ChildItem -LiteralPath $PortableRoot -Force | Remove-Item -Recurse -Force

New-Item -ItemType Directory -Force -Path $PortableMcpRoot | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $PortableSkillRoot) | Out-Null

Copy-Item -LiteralPath $ExePath -Destination (Join-Path $PortableRoot "ChatMem.exe") -Force
Copy-Item -LiteralPath $UsagePath -Destination $PortableUsagePath -Force
Copy-Item -LiteralPath $McpExePath -Destination (Join-Path $PortableMcpRoot "chatmem-mcp.exe") -Force
Copy-Item -LiteralPath $McpScriptPath -Destination (Join-Path $PortableMcpRoot "run-chatmem-mcp.ps1") -Force
Copy-Item -LiteralPath $SkillPath -Destination $PortableSkillRoot -Recurse -Force

if (Test-Path -LiteralPath $McpDocsPath) {
  Copy-Item -LiteralPath $McpDocsPath -Destination (Join-Path $PortableRoot "MCP_SETUP.txt") -Force
}

if (Test-Path -LiteralPath $ZipPath) {
  Remove-Item -LiteralPath $ZipPath -Force
}

Compress-Archive -Path (Join-Path $PortableRoot "*") -DestinationPath $ZipPath

Write-Host "Portable package written to $ZipPath"
