$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$launcher = Join-Path $repoRoot "Launch-OpenClawInstaller.cmd"
$repoIcon = Join-Path $repoRoot "src-tauri\\icons\\icon.ico"

$installedUser = Join-Path $env:LOCALAPPDATA "Programs\\openclaw-installer\\openclaw-installer.exe"
$installedMachine = Join-Path $env:ProgramFiles "OpenClaw Installer\\openclaw-installer.exe"
$releaseExe = Join-Path $repoRoot "src-tauri\\target\\release\\openclaw-installer.exe"
$debugExe = Join-Path $repoRoot "src-tauri\\target\\debug\\openclaw-installer.exe"

$primaryTarget = $null
foreach ($candidate in @($installedUser, $installedMachine, $releaseExe, $debugExe)) {
  if (Test-Path $candidate) {
    $primaryTarget = $candidate
    break
  }
}

if (-not (Test-Path $launcher)) {
  Write-Error "Launcher not found: $launcher"
}

$desktop = [Environment]::GetFolderPath("Desktop")
$shortcutPath = Join-Path $desktop "OpenClaw Installer.lnk"
$preflightPath = Join-Path $desktop "OpenClaw Installer - Preflight.lnk"
$devPath = Join-Path $desktop "OpenClaw Installer (Dev).lnk"

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = if ($primaryTarget) { $primaryTarget } else { $launcher }
$shortcut.WorkingDirectory = if ($primaryTarget) { Split-Path $primaryTarget -Parent } else { $repoRoot }
$shortcut.WindowStyle = 1
$shortcut.Description = "OpenClaw Installer (runs in tray; close hides to tray)"
if (Test-Path $repoIcon) {
  $shortcut.IconLocation = "$repoIcon,0"
} else {
  $shortcut.IconLocation = "$env:SystemRoot\\System32\\shell32.dll,220"
}
$shortcut.Save()

$preflight = $shell.CreateShortcut($preflightPath)
$preflight.TargetPath = $launcher
$preflight.Arguments = "--preflight"
$preflight.WorkingDirectory = $repoRoot
$preflight.WindowStyle = 1
$preflight.Description = "OpenClaw Installer environment preflight"
$preflight.IconLocation = "$env:SystemRoot\System32\shell32.dll,23"
$preflight.Save()

$dev = $shell.CreateShortcut($devPath)
$dev.TargetPath = $launcher
$dev.Arguments = "--dev"
$dev.WorkingDirectory = $repoRoot
$dev.WindowStyle = 1
$dev.Description = "OpenClaw Installer dev mode (hot reload; terminal stays open)"
$dev.IconLocation = "$env:SystemRoot\\System32\\shell32.dll,220"
$dev.Save()

Write-Host "Shortcut created: $shortcutPath"
Write-Host "Shortcut created: $preflightPath"
Write-Host "Shortcut created: $devPath"
