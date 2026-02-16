@echo off
setlocal

cd /d "%~dp0"

echo Creating desktop shortcut...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\create_shortcut.ps1"

if errorlevel 1 (
  echo [ERROR] Failed to create shortcut.
  pause
  exit /b 1
)

echo [OK] Desktop shortcut created.
pause
endlocal
