param(
  [string]$Distro = "Ubuntu",
  [int]$Port = 28789,
  [string]$BaseDir = '$HOME/openclaw-isolated',
  [switch]$PrepareOnly,
  [switch]$SkipInstall
)

$ErrorActionPreference = "Stop"

function Get-CleanDistroList {
  # `wsl -l -q` can include embedded NUL bytes in some Windows terminals.
  (& wsl -l -q) |
    ForEach-Object { ($_ -replace "`0", "").Trim() } |
    Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
}

function Invoke-Wsl([string]$Name, [string]$Command) {
  Write-Host "[WSL:$Name] $Command"
  & wsl -d $Name bash -lc $Command
}

$distros = Get-CleanDistroList
if ($distros -notcontains $Distro) {
  throw "WSL distro '$Distro' not found. Available: $($distros -join ', ')"
}

$homeDir = "$BaseDir/home"
$workspaceDir = "$BaseDir/workspace"
$logsDir = "$BaseDir/logs"

Write-Host "== OpenClaw isolated test startup =="
Write-Host "Distro   : $Distro"
Write-Host "BaseDir  : $BaseDir"
Write-Host "Home     : $homeDir"
Write-Host "Workspace: $workspaceDir"
Write-Host "Logs     : $logsDir"
Write-Host "Port     : $Port"

Invoke-Wsl $Distro "mkdir -p `"$homeDir`" `"$workspaceDir`" `"$logsDir`""

$npmExists = (Invoke-Wsl $Distro "if command -v npm >/dev/null 2>&1; then echo yes; else echo no; fi").Trim()
if ($npmExists -ne "yes") {
  throw "npm is not available in WSL '$Distro'. Install Node.js in WSL first."
}

$openclawExists = (Invoke-Wsl $Distro "if command -v openclaw >/dev/null 2>&1; then echo yes; else echo no; fi").Trim()
if ($PrepareOnly) {
  if ($openclawExists -ne "yes") {
    Write-Host "[WARN] openclaw is not installed in WSL '$Distro' yet."
    Write-Host "       Run without -PrepareOnly to auto-install it."
  } else {
    Write-Host "[OK] openclaw already exists in WSL."
  }
  Write-Host "[OK] Isolated environment prepared. Gateway not started (-PrepareOnly)."
  Write-Host "Next: .\scripts\start-isolated-test.ps1 -Distro $Distro -Port $Port"
  exit 0
}

if ($openclawExists -ne "yes" -and -not $SkipInstall) {
  Write-Host "[INFO] openclaw not found in WSL. Installing..."
  Invoke-Wsl $Distro "npm i -g openclaw@latest"
} elseif ($openclawExists -ne "yes" -and $SkipInstall) {
  throw "openclaw not found in WSL and -SkipInstall was set."
}

Write-Host "[INFO] Starting isolated gateway in WSL (foreground). Press Ctrl+C to stop."
Invoke-Wsl $Distro "export OPENCLAW_HOME=`"$homeDir`"; export OPENCLAW_WORKSPACE=`"$workspaceDir`"; export OPENCLAW_LOG_DIR=`"$logsDir`"; openclaw gateway --port $Port"
