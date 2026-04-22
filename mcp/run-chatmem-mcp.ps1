param()

$ErrorActionPreference = "Stop"

if ($env:CHATMEM_REPO_ROOT) {
  $repoRoot = (Resolve-Path $env:CHATMEM_REPO_ROOT).Path
} else {
  $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Resolve-ChatMemMcpBinary {
  if ($env:CHATMEM_MCP_BIN -and (Test-Path $env:CHATMEM_MCP_BIN)) {
    return (Resolve-Path $env:CHATMEM_MCP_BIN).Path
  }

  $candidates = @(
    (Join-Path $repoRoot "src-tauri\target\release\chatmem-mcp.exe"),
    (Join-Path $repoRoot ".tauri-target-build\release\chatmem-mcp.exe"),
    (Join-Path $repoRoot "src-tauri\target\debug\chatmem-mcp.exe")
  ) | Where-Object { Test-Path $_ } |
    Sort-Object { (Get-Item $_).LastWriteTimeUtc } -Descending

  foreach ($candidate in $candidates) {
    return (Resolve-Path $candidate).Path
  }

  return $null
}

$binary = Resolve-ChatMemMcpBinary
if (-not $binary) {
  throw "chatmem-mcp binary not found. Build the MCP server with cargo build --release --bin chatmem-mcp, or set CHATMEM_MCP_BIN."
}

& $binary
exit $LASTEXITCODE
