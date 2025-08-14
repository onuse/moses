@echo off
echo Building and Testing Moses CLI on Windows
echo ==========================================
echo.

echo Building CLI (this may take a minute)...
cargo build --package moses-cli
if %errorlevel% neq 0 (
    echo Build failed! Make sure Rust is installed.
    echo Install from: https://rustup.rs/
    pause
    exit /b %errorlevel%
)

echo.
echo Build successful! Now testing device enumeration...
echo.
echo ==========================================
echo DEVICE LIST:
echo ==========================================
echo.

target\debug\moses.exe list

echo.
echo ==========================================
echo.
echo If you see your drives listed above, the Windows device enumeration is working!
echo.
echo Note: System drives are marked with "PROTECTED" and cannot be formatted.
echo Note: You may need to run as Administrator to see all drives.
echo.
pause