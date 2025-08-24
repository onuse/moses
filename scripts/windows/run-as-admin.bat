@echo off
:: Batch script to run Moses with administrator privileges

echo Requesting Administrator privileges for Moses Drive Formatter...
powershell -ExecutionPolicy Bypass -File "%~dp0run-as-admin.ps1"

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Failed to start Moses. Please ensure the application is built.
    echo Build commands:
    echo   cargo build --release
    echo   or
    echo   npm run tauri build
    pause
)