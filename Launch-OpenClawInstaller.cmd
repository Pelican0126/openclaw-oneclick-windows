@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul

cd /d "%~dp0"

set "MODE=%~1"
if "%MODE%"=="" set "MODE=run"

echo ======================================================
echo OpenClaw Installer - One Click Launch
echo ======================================================

set "RELEASE_EXE=%~dp0src-tauri\target\release\openclaw-installer.exe"
set "RELEASE_EXE_ALT=%~dp0src-tauri\target\release\OpenClaw Installer.exe"
set "RELEASE_EXE_TARGET_ALT=%~dp0src-tauri\target-alt\release\openclaw-installer.exe"
set "RELEASE_EXE_TARGET_ALT_NAME=%~dp0src-tauri\target-alt\release\OpenClaw Installer.exe"
set "DEBUG_EXE=%~dp0src-tauri\target\debug\openclaw-installer.exe"
set "INSTALLED_EXE_USER=%LOCALAPPDATA%\Programs\openclaw-installer\openclaw-installer.exe"
set "INSTALLED_EXE_MACHINE=%ProgramFiles%\OpenClaw Installer\openclaw-installer.exe"
set "MSI_BUNDLE_DIR=%~dp0src-tauri\target\release\bundle\msi"
set "MSI_BUNDLE_DIR_ALT=%~dp0src-tauri\target-alt\release\bundle\msi"
set "NSIS_BUNDLE_DIR=%~dp0src-tauri\target\release\bundle\nsis"
set "NSIS_BUNDLE_DIR_ALT=%~dp0src-tauri\target-alt\release\bundle\nsis"

if /I "%MODE%"=="--preflight" goto :run_preflight
if /I "%MODE%"=="--dev" goto :run_preflight
if /I "%MODE%"=="dev" goto :run_preflight
if /I "%MODE%"=="--release" goto :try_release

:try_release
if exist "%RELEASE_EXE%" (
  call :is_installer_running
  if not errorlevel 1 (
    echo [INFO] OpenClaw Installer is already running.
    exit /b 0
  )
  echo [INFO] Release binary found. Launching directly...
  start "" "%RELEASE_EXE%"
  exit /b 0
)

if exist "%RELEASE_EXE_ALT%" (
  call :is_installer_running
  if not errorlevel 1 (
    echo [INFO] OpenClaw Installer is already running.
    exit /b 0
  )
  echo [INFO] Release binary found. Launching directly...
  start "" "%RELEASE_EXE_ALT%"
  exit /b 0
)

if exist "%RELEASE_EXE_TARGET_ALT%" (
  call :is_installer_running
  if not errorlevel 1 (
    echo [INFO] OpenClaw Installer is already running.
    exit /b 0
  )
  echo [INFO] Release binary (target-alt) found. Launching directly...
  start "" "%RELEASE_EXE_TARGET_ALT%"
  exit /b 0
)

if exist "%RELEASE_EXE_TARGET_ALT_NAME%" (
  call :is_installer_running
  if not errorlevel 1 (
    echo [INFO] OpenClaw Installer is already running.
    exit /b 0
  )
  echo [INFO] Release binary (target-alt) found. Launching directly...
  start "" "%RELEASE_EXE_TARGET_ALT_NAME%"
  exit /b 0
)

if exist "%DEBUG_EXE%" (
  call :is_installer_running
  if not errorlevel 1 (
    echo [INFO] OpenClaw Installer is already running.
    exit /b 0
  )
  echo [INFO] Debug binary found. Launching directly...
  start "" "%DEBUG_EXE%"
  exit /b 0
)

if exist "%INSTALLED_EXE_USER%" (
  call :is_installer_running
  if not errorlevel 1 (
    echo [INFO] OpenClaw Installer is already running.
    exit /b 0
  )
  echo [INFO] Installed app found. Launching directly...
  start "" "%INSTALLED_EXE_USER%"
  exit /b 0
)

if exist "%INSTALLED_EXE_MACHINE%" (
  call :is_installer_running
  if not errorlevel 1 (
    echo [INFO] OpenClaw Installer is already running.
    exit /b 0
  )
  echo [INFO] Installed app found. Launching directly...
  start "" "%INSTALLED_EXE_MACHINE%"
  exit /b 0
)

:run_preflight
echo [INFO] Entering dev mode (this terminal must stay open). Use packaged EXE/MSI for normal use.
call :ensure_node
if errorlevel 1 goto :hard_fail

if not exist "node_modules" (
  echo [INFO] node_modules not found, running npm ci...
  call npm ci
  if errorlevel 1 (
    echo [WARN] npm ci failed, fallback to npm install...
    call npm install
  )
  if errorlevel 1 (
    echo [ERROR] npm install failed.
    goto :hard_fail
  )
)

call :ensure_cargo
if errorlevel 1 goto :hard_fail

call :ensure_msvc_linker
if errorlevel 1 goto :hard_fail

if /I "%MODE%"=="--preflight" (
  echo [OK] Preflight passed.
  exit /b 0
)

call :release_port_if_needed 1420

echo [INFO] Starting GUI...
call npm run tauri:dev

if errorlevel 1 (
  echo [ERROR] Launch failed. Check logs and terminal output.
  goto :hard_fail
)

endlocal
exit /b 0

:ensure_node
where npm >nul 2>nul
if not errorlevel 1 exit /b 0

echo [WARN] npm not found, trying auto install Node.js LTS...
where winget >nul 2>nul
if not errorlevel 1 (
  call winget install --id OpenJS.NodeJS.LTS -e --accept-source-agreements --accept-package-agreements --silent
)

where npm >nul 2>nul
if not errorlevel 1 exit /b 0

where choco >nul 2>nul
if not errorlevel 1 (
  call choco install nodejs-lts -y
)

where npm >nul 2>nul
if not errorlevel 1 exit /b 0

echo [ERROR] npm still not found.
echo         Please install Node.js 20+ manually:
echo         winget install --id OpenJS.NodeJS.LTS -e
exit /b 1

:ensure_cargo
where cargo >nul 2>nul
if not errorlevel 1 exit /b 0

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
  set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
  where cargo >nul 2>nul
  if not errorlevel 1 exit /b 0
)

echo [WARN] cargo not found, trying auto install Rust toolchain...
where winget >nul 2>nul
if not errorlevel 1 (
  call winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements --silent
)

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
  set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)
where cargo >nul 2>nul
if not errorlevel 1 exit /b 0

where choco >nul 2>nul
if not errorlevel 1 (
  call choco install rustup.install -y
)
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
  set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)
where cargo >nul 2>nul
if not errorlevel 1 exit /b 0

echo [ERROR] cargo still not found.
if exist "%NSIS_BUNDLE_DIR%" (
  for /f "delims=" %%F in ('dir /b /o-n "%NSIS_BUNDLE_DIR%\\*.exe" 2^>nul') do (
    echo [INFO] Found packaged EXE installer. Opening: %%F
    start "" "%NSIS_BUNDLE_DIR%\\%%F"
    exit /b 0
  )
)
if exist "%NSIS_BUNDLE_DIR_ALT%" (
  for /f "delims=" %%F in ('dir /b /o-n "%NSIS_BUNDLE_DIR_ALT%\\*.exe" 2^>nul') do (
    echo [INFO] Found packaged EXE installer (target-alt). Opening: %%F
    start "" "%NSIS_BUNDLE_DIR_ALT%\\%%F"
    exit /b 0
  )
)
if exist "%MSI_BUNDLE_DIR%" (
  for /f "delims=" %%F in ('dir /b /o-n "%MSI_BUNDLE_DIR%\\*.msi" 2^>nul') do (
    echo [INFO] Found packaged MSI. Opening installer: %%F
    start "" "%MSI_BUNDLE_DIR%\\%%F"
    exit /b 0
  )
)
if exist "%MSI_BUNDLE_DIR_ALT%" (
  for /f "delims=" %%F in ('dir /b /o-n "%MSI_BUNDLE_DIR_ALT%\\*.msi" 2^>nul') do (
    echo [INFO] Found packaged MSI (target-alt). Opening installer: %%F
    start "" "%MSI_BUNDLE_DIR_ALT%\\%%F"
    exit /b 0
  )
)
echo         Please install Rust manually for dev mode:
echo         winget install --id Rustlang.Rustup -e
exit /b 1

:ensure_msvc_linker
call :locate_msvc_linker
if not errorlevel 1 exit /b 0

echo [WARN] MSVC linker not found, trying auto install Build Tools...
where winget >nul 2>nul
if not errorlevel 1 (
  call winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements --accept-package-agreements --override "--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
)

call :locate_msvc_linker
if not errorlevel 1 exit /b 0

echo [ERROR] MSVC linker still not found.
echo         Please install VS Build Tools with C++ workload:
echo         winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
exit /b 1

:locate_msvc_linker
set "MSVC_BIN="

for /f "delims=" %%P in ('where link.exe 2^>nul') do (
  echo %%P | findstr /I /C:"Microsoft Visual Studio" >nul
  if not errorlevel 1 (
    set "MSVC_BIN=%%~dpP"
  )
)

if defined MSVC_BIN (
  set "PATH=%MSVC_BIN%;%PATH%"
  exit /b 0
)

if exist "%ProgramFiles%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC" (
  for /f "delims=" %%D in ('dir /b /ad "%ProgramFiles%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC" ^| sort /R') do (
    if exist "%ProgramFiles%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\%%D\bin\Hostx64\x64\link.exe" (
      set "MSVC_BIN=%ProgramFiles%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\%%D\bin\Hostx64\x64"
      goto :msvc_found
    )
  )
)

if exist "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC" (
  for /f "delims=" %%D in ('dir /b /ad "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC" ^| sort /R') do (
    if exist "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\%%D\bin\Hostx64\x64\link.exe" (
      set "MSVC_BIN=%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\%%D\bin\Hostx64\x64"
      goto :msvc_found
    )
  )
)

exit /b 1

:msvc_found
set "PATH=%MSVC_BIN%;%PATH%"
exit /b 0

:is_installer_running
tasklist /FI "IMAGENAME eq openclaw-installer.exe" /FO CSV /NH | findstr /I /C:"openclaw-installer.exe" >nul
if not errorlevel 1 exit /b 0
exit /b 1

:release_port_if_needed
set "TARGET_PORT=%~1"
for /f "delims=" %%L in ('powershell -NoProfile -Command "$port=%TARGET_PORT%; $conn=Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue | Select-Object -First 1; if(-not $conn){ return }; $proc=Get-Process -Id $conn.OwningProcess -ErrorAction SilentlyContinue; if($proc -and $proc.ProcessName -ieq 'node'){ Stop-Process -Id $proc.Id -Force; Start-Sleep -Milliseconds 400; Write-Output ('[WARN] Dev port ' + $port + ' had stale node process (PID ' + $proc.Id + '), cleaned.'); } elseif($proc){ Write-Output ('[WARN] Dev port ' + $port + ' is occupied by PID ' + $proc.Id + ' (' + $proc.ProcessName + ').'); Write-Output '       Close it manually or change build.devUrl in src-tauri\\tauri.conf.json.'; }"') do (
  echo %%L
)
exit /b 0

:hard_fail
pause
exit /b 1
