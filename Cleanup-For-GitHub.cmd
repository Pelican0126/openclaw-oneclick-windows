@echo off
setlocal enabledelayedexpansion

REM ======================================================
REM OpenClaw Installer - Cleanup For GitHub Upload
REM ======================================================
REM This script deletes build outputs / caches / smoke artifacts
REM so you don't accidentally upload huge folders or local logs.
REM
REM It does NOT touch your Windows %USERPROFILE%\.openclaw.
REM
REM Notes:
REM - The repo contains a folder literally named "%TEMP%".
REM   In batch files, "%TEMP%" is an environment variable, so we
REM   must refer to it as "%%TEMP%%" to mean the literal folder name.
REM ======================================================

cd /d "%~dp0"

echo [cleanup] Repo: %CD%
echo.
echo [cleanup] Removing generated folders (if present)...

if exist "node_modules" (
  echo   - node_modules
  rmdir /s /q "node_modules"
)

if exist "dist" (
  echo   - dist
  rmdir /s /q "dist"
)

if exist ".smoke" (
  echo   - .smoke
  rmdir /s /q ".smoke"
)

if exist ".smoke-temp" (
  echo   - .smoke-temp
  rmdir /s /q ".smoke-temp"
)

if exist "%%TEMP%%" (
  echo   - %%TEMP%%
  rmdir /s /q "%%TEMP%%"
)

if exist "~" (
  echo   - ~
  rmdir /s /q "~"
)

if exist "src-tauri\\target" (
  echo   - src-tauri\\target
  rmdir /s /q "src-tauri\\target"
)

if exist "src-tauri\\target-alt" (
  echo   - src-tauri\\target-alt
  rmdir /s /q "src-tauri\\target-alt"
)

if exist "src-tauri\\src-tauri\\target-smoke" (
  echo   - src-tauri\\src-tauri\\target-smoke
  rmdir /s /q "src-tauri\\src-tauri\\target-smoke"
)

echo.
echo [cleanup] Removing local logs / screenshots (if present)...

del /q ".tmp-*.log" 2>nul
del /q "ae1b0bc5ade4cecfdb722e09d6148ec8.jpg" 2>nul

echo.
echo [cleanup] Done.
echo.
echo Optional:
echo - If you do NOT want to upload extracted upstream source, you can delete:
echo   source\\openclaw-npm-*
echo   (it is already ignored by .gitignore)
echo.
pause

