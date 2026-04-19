param(
  [switch]$InstallCodex,
  [switch]$InstallClaude
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$pluginSource = Join-Path $repoRoot "plugins\chatmem"

function Sync-Plugin {
  param(
    [Parameter(Mandatory = $true)]
    [string]$DestinationRoot
  )

  $destination = Join-Path $DestinationRoot "chatmem"
  New-Item -ItemType Directory -Force -Path $DestinationRoot | Out-Null

  if (Test-Path $destination) {
    Remove-Item -Recurse -Force $destination
  }

  Copy-Item -Recurse -Force $pluginSource $destination
  Set-Content -Path (Join-Path $destination "repo-root.txt") -Value $repoRoot -NoNewline
  Write-Output "Synced ChatMem plugin to $destination"
}

if (-not $InstallCodex -and -not $InstallClaude) {
  $InstallCodex = $true
  $InstallClaude = $true
}

if ($InstallCodex) {
  Sync-Plugin -DestinationRoot (Join-Path $HOME ".codex\plugins")
}

if ($InstallClaude) {
  Sync-Plugin -DestinationRoot (Join-Path $HOME ".claude\plugins")
}
