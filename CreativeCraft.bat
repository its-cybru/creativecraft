@echo off
rem CreativeCraft launcher: runs the app that lives in creativecraft\studio-shell.
rem Double-clicking this file is enough - no terminal commands to remember.
cd /d "%~dp0creativecraft\studio-shell"
if exist "target\debug\studio-shell.exe" (
    start "" "target\debug\studio-shell.exe"
    exit /b 0
)
echo First run: building CreativeCraft. This takes a while, once.
cargo run
pause
