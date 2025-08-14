@echo off
echo Building Moses for Windows...
echo ================================

echo.
echo Step 1: Building CLI executable
cargo build --package moses-cli --release
if %errorlevel% neq 0 (
    echo Failed to build CLI
    exit /b %errorlevel%
)

echo.
echo Step 2: Building Tauri GUI app (requires Node.js)
cd ui
call npm install
if %errorlevel% neq 0 (
    echo Failed to install npm dependencies
    echo Make sure Node.js is installed: https://nodejs.org/
    exit /b %errorlevel%
)

call npm run build
if %errorlevel% neq 0 (
    echo Failed to build UI
    exit /b %errorlevel%
)
cd ..

echo.
echo Step 3: Building Tauri executable
call npm run tauri build
if %errorlevel% neq 0 (
    echo Failed to build Tauri app
    exit /b %errorlevel%
)

echo.
echo ================================
echo Build completed successfully!
echo.
echo Executables location:
echo CLI: target\release\moses.exe
echo GUI: src-tauri\target\release\moses.exe
echo.
echo To test CLI device enumeration:
echo   target\release\moses.exe list
echo.
echo To run GUI in development mode:
echo   npm run tauri dev