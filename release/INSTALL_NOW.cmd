@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"

set "SETUP="
for %%F in (OpenClawInstaller-v*-setup.exe) do (
  set "SETUP=%%F"
  goto run_setup
)

:run_setup
if defined SETUP (
  echo [install] Launching !SETUP! ...
  start "" "!SETUP!"
  exit /b 0
)

set "MSI="
for %%F in (OpenClawInstaller-v*.msi) do (
  set "MSI=%%F"
  goto run_msi
)

:run_msi
if defined MSI (
  echo [install] Launching !MSI! ...
  msiexec /i "!MSI!"
  exit /b 0
)

echo [error] Installer package not found in this folder.
echo [hint] Keep this cmd file with the .exe/.msi package files.
pause
