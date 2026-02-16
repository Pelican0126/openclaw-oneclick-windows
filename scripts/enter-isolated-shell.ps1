param(
  [string]$Distro = "Ubuntu",
  [int]$Port = 28789,
  [string]$BaseDir = '$HOME/openclaw-isolated'
)

$ErrorActionPreference = "Stop"

function Get-CleanDistroList {
  (& wsl -l -q) |
    ForEach-Object { ($_ -replace "`0", "").Trim() } |
    Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
}

$distros = Get-CleanDistroList
if ($distros -notcontains $Distro) {
  throw "WSL distro '$Distro' not found. Available: $($distros -join ', ')"
}

$homeDir = "$BaseDir/home"
$workspaceDir = "$BaseDir/workspace"
$logsDir = "$BaseDir/logs"

# Ensure folders exist so first shell entry is ready to use.
& wsl -d $Distro bash -lc "mkdir -p `"$homeDir`" `"$workspaceDir`" `"$logsDir`""

Write-Host "Entering isolated WSL shell..."
Write-Host "Distro : $Distro"
Write-Host "Home   : $homeDir"
Write-Host "Port   : $Port"

# Open an interactive shell with isolation variables preloaded.
& wsl -d $Distro bash -lc @"
export OPENCLAW_HOME="$homeDir"
export OPENCLAW_WORKSPACE="$workspaceDir"
export OPENCLAW_LOG_DIR="$logsDir"
export OPENCLAW_GATEWAY_PORT="$Port"
cd "$workspaceDir"
echo '[isolated] OPENCLAW_HOME='${OPENCLAW_HOME}
echo '[isolated] OPENCLAW_WORKSPACE='${OPENCLAW_WORKSPACE}
echo '[isolated] OPENCLAW_LOG_DIR='${OPENCLAW_LOG_DIR}
echo '[isolated] OPENCLAW_GATEWAY_PORT='${OPENCLAW_GATEWAY_PORT}
echo '[isolated] You can run: openclaw gateway --port $Port'
exec bash -i
"@
