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

function Update-CodexMarketplace {
  param(
    [Parameter(Mandatory = $true)]
    [string]$MarketplacePath
  )

  $marketplaceDir = Split-Path -Parent $MarketplacePath
  New-Item -ItemType Directory -Force -Path $marketplaceDir | Out-Null

  if (Test-Path $MarketplacePath) {
    $marketplace = Get-Content -Raw $MarketplacePath | ConvertFrom-Json -AsHashtable
  } else {
    $marketplace = @{
      name = "chatmem-local"
      interface = @{
        displayName = "ChatMem Local Plugins"
      }
      plugins = @()
    }
  }

  if (-not $marketplace.ContainsKey("name") -or [string]::IsNullOrWhiteSpace([string]$marketplace.name)) {
    $marketplace.name = "chatmem-local"
  }

  if (-not $marketplace.ContainsKey("interface") -or $null -eq $marketplace.interface) {
    $marketplace.interface = @{}
  }

  if (-not $marketplace.interface.ContainsKey("displayName") -or [string]::IsNullOrWhiteSpace([string]$marketplace.interface.displayName)) {
    $marketplace.interface.displayName = "ChatMem Local Plugins"
  }

  if (-not $marketplace.ContainsKey("plugins") -or $null -eq $marketplace.plugins) {
    $marketplace.plugins = @()
  }

  $entry = @{
    name = "chatmem"
    source = @{
      source = "local"
      path = "./plugins/chatmem"
    }
    policy = @{
      installation = "AVAILABLE"
      authentication = "ON_INSTALL"
    }
    category = "Coding"
  }

  $existingIndex = -1
  for ($i = 0; $i -lt $marketplace.plugins.Count; $i++) {
    if ($marketplace.plugins[$i].name -eq "chatmem") {
      $existingIndex = $i
      break
    }
  }

  if ($existingIndex -ge 0) {
    $marketplace.plugins[$existingIndex] = $entry
  } else {
    $marketplace.plugins += $entry
  }

  $json = $marketplace | ConvertTo-Json -Depth 10
  Set-Content -Path $MarketplacePath -Value $json -Encoding utf8
  Write-Output "Registered ChatMem in Codex marketplace at $MarketplacePath"
}

if (-not $InstallCodex -and -not $InstallClaude) {
  $InstallCodex = $true
  $InstallClaude = $true
}

if ($InstallCodex) {
  Sync-Plugin -DestinationRoot (Join-Path $HOME "plugins")
  Update-CodexMarketplace -MarketplacePath (Join-Path $HOME ".agents\plugins\marketplace.json")
}

if ($InstallClaude) {
  Sync-Plugin -DestinationRoot (Join-Path $HOME ".claude\plugins")
}
