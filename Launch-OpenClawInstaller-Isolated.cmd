@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul

cd /d "%~dp0"

rem ============================================================================
rem OpenClaw Installer - Isolated Launcher
rem ----------------------------------------------------------------------------
rem Purpose:
rem - Run the Installer GUI with *isolated* state/log paths so it won't mix with
rem   any existing OpenClaw installation on this machine.
rem
rem What is isolated:
rem - Installer data dir (logs/backups/state): OPENCLAW_INSTALLER_DATA_DIR
rem - Default OpenClaw home override (backend safety): OPENCLAW_INSTALLER_OPENCLAW_HOME
rem
rem Notes:
rem - The GUI wizard still lets you choose an install directory. Keeping the
rem   default (%LOCALAPPDATA%\OpenClawInstaller\openclaw) is already isolated
rem   from %USERPROFILE%\.openclaw.
rem ============================================================================

set "ISO_ROOT=%~dp0.smoke-temp\gui-isolated"
set "OPENCLAW_INSTALLER_DATA_DIR=%ISO_ROOT%\appdata"
set "OPENCLAW_INSTALLER_OPENCLAW_HOME=%ISO_ROOT%\openclaw-home"

if not exist "%OPENCLAW_INSTALLER_DATA_DIR%" mkdir "%OPENCLAW_INSTALLER_DATA_DIR%" >nul 2>nul
if not exist "%OPENCLAW_INSTALLER_OPENCLAW_HOME%" mkdir "%OPENCLAW_INSTALLER_OPENCLAW_HOME%" >nul 2>nul

echo ======================================================
echo OpenClaw Installer - Isolated Launch
echo ======================================================
echo [INFO] OPENCLAW_INSTALLER_DATA_DIR      = %OPENCLAW_INSTALLER_DATA_DIR%
echo [INFO] OPENCLAW_INSTALLER_OPENCLAW_HOME = %OPENCLAW_INSTALLER_OPENCLAW_HOME%
echo.

rem Default to dev mode so we always run the latest workspace UI/backend
rem (instead of accidentally launching an older installed EXE).
set "MODE=%~1"
if "%MODE%"=="" (
  call "%~dp0Launch-OpenClawInstaller.cmd" dev
) else (
  call "%~dp0Launch-OpenClawInstaller.cmd" %*
)

endlocal
