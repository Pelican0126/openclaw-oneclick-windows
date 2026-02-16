@echo off
setlocal
cd /d "%~dp0"
call "%~dp0Launch-OpenClawInstaller.cmd" --preflight
endlocal
