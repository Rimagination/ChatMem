param()

$ErrorActionPreference = "Stop"

$pluginRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$repoRootFile = Join-Path $pluginRoot "repo-root.txt"

if ($env:CHATMEM_REPO_ROOT) {
  $repoRoot = (Resolve-Path $env:CHATMEM_REPO_ROOT).Path
} elseif (Test-Path $repoRootFile) {
  $repoRoot = (Resolve-Path (Get-Content $repoRootFile -Raw).Trim()).Path
} else {
  $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
}

function Resolve-ChatMemMcpBinary {
  if ($env:CHATMEM_MCP_BIN -and (Test-Path $env:CHATMEM_MCP_BIN)) {
    return (Resolve-Path $env:CHATMEM_MCP_BIN).Path
  }

  $candidates = @(
    (Join-Path $repoRoot "src-tauri\target\release\chatmem-mcp.exe"),
    (Join-Path $repoRoot "src-tauri\target\debug\chatmem-mcp.exe")
  )

  foreach ($candidate in $candidates) {
    if (Test-Path $candidate) {
      return (Resolve-Path $candidate).Path
    }
  }

  return $null
}

$binary = Resolve-ChatMemMcpBinary
if (-not $binary) {
  throw "chatmem-mcp binary not found. Build D:\VSP\agentswap-gui\src-tauri first or set CHATMEM_MCP_BIN."
}

& $binary
exit $LASTEXITCODE
