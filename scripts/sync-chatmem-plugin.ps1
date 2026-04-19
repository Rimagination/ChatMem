param(
  [string]$CodexWorkspaceRoot,
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

function Update-CodexMarketplace {
  param(
    [Parameter(Mandatory = $true)]
    [string]$WorkspaceRoot
  )

  $MarketplacePath = Join-Path $WorkspaceRoot ".agents\plugins\marketplace.json"
  $marketplaceDir = Split-Path -Parent $MarketplacePath
  New-Item -ItemType Directory -Force -Path $marketplaceDir | Out-Null

  if (Test-Path $MarketplacePath) {
    $marketplace = Get-Content -Raw $MarketplacePath | ConvertFrom-Json
  } else {
    $marketplace = [pscustomobject]@{
      name = "chatmem-local"
      interface = [pscustomobject]@{
        displayName = "ChatMem Local Plugins"
      }
      plugins = @()
    }
  }

  if (-not ($marketplace.PSObject.Properties.Name -contains "name") -or [string]::IsNullOrWhiteSpace([string]$marketplace.name)) {
    $marketplace.name = "chatmem-local"
  }

  if (-not ($marketplace.PSObject.Properties.Name -contains "interface") -or $null -eq $marketplace.interface) {
    $marketplace | Add-Member -NotePropertyName interface -NotePropertyValue ([pscustomobject]@{}) -Force
  }

  if (-not ($marketplace.interface.PSObject.Properties.Name -contains "displayName") -or [string]::IsNullOrWhiteSpace([string]$marketplace.interface.displayName)) {
    $marketplace.interface.displayName = "ChatMem Local Plugins"
  }

  if (-not ($marketplace.PSObject.Properties.Name -contains "plugins") -or $null -eq $marketplace.plugins) {
    $marketplace | Add-Member -NotePropertyName plugins -NotePropertyValue @() -Force
  }

  $plugins = @($marketplace.plugins)

  $entry = [pscustomobject]@{
    name = "chatmem"
    source = [pscustomobject]@{
      source = "local"
      path = "./plugins/chatmem"
    }
    policy = [pscustomobject]@{
      installation = "AVAILABLE"
      authentication = "ON_INSTALL"
    }
    category = "Coding"
  }

  $existingIndex = -1
  for ($i = 0; $i -lt $plugins.Count; $i++) {
    if ($plugins[$i].name -eq "chatmem") {
      $existingIndex = $i
      break
    }
  }

  if ($existingIndex -ge 0) {
    $plugins[$existingIndex] = $entry
  } else {
    $plugins += $entry
  }

  $marketplace.plugins = $plugins
  $json = $marketplace | ConvertTo-Json -Depth 10
  Set-Content -Path $MarketplacePath -Value $json -Encoding utf8
  Write-Output "Registered ChatMem in Codex marketplace at $MarketplacePath"
}

function Sync-CodexWorkspacePlugin {
  param(
    [Parameter(Mandatory = $true)]
    [string]$WorkspaceRoot
  )

  Sync-Plugin -DestinationRoot (Join-Path $WorkspaceRoot "plugins")
  Update-CodexMarketplace -WorkspaceRoot $WorkspaceRoot
}

if (-not $CodexWorkspaceRoot -and -not $InstallClaude) {
  $CodexWorkspaceRoot = $repoRoot
  $InstallClaude = $true
}

if ($CodexWorkspaceRoot) {
  $resolvedWorkspaceRoot = (Resolve-Path $CodexWorkspaceRoot).Path
  Sync-CodexWorkspacePlugin -WorkspaceRoot $resolvedWorkspaceRoot
}

if ($InstallClaude) {
  Sync-Plugin -DestinationRoot (Join-Path $HOME ".claude\plugins")
}
