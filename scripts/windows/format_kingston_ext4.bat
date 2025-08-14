@echo off
echo =========================================================
echo Moses EXT4 Formatter - Kingston DataTraveler Test
echo =========================================================
echo.
echo SAFETY CHECK: This script will format your Kingston DataTraveler 3.0
echo              as EXT4 filesystem using WSL2.
echo.
echo Prerequisites:
echo   1. WSL2 must be installed (run: wsl --install)
echo   2. A Linux distribution must be installed in WSL
echo   3. Your Kingston DataTraveler must be connected
echo.
pause
echo.

echo Step 1: Building Moses CLI...
cargo build --package moses-cli
if %errorlevel% neq 0 (
    echo Build failed!
    pause
    exit /b %errorlevel%
)

echo.
echo Step 2: Listing available devices...
echo =========================================================
target\debug\moses.exe list
echo =========================================================
echo.
echo Please verify your Kingston DataTraveler 3.0 is listed above.
echo It should show as:
echo   - Size: ~57.66 GB
echo   - Type: USB
echo   - Removable: Yes
echo.
pause

echo.
echo Step 3: Running format simulation (dry run)...
echo This will check if formatting is possible without making changes.
echo.
target\debug\moses.exe format "Kingston DataTraveler" ext4

echo.
echo =========================================================
echo If the simulation was successful and you typed 'yes',
echo your Kingston DataTraveler should now be formatted as EXT4!
echo.
echo To verify in WSL, run:
echo   wsl lsblk -f
echo.
echo To mount and use in WSL:
echo   wsl sudo mkdir -p /mnt/kingston
echo   wsl sudo mount /dev/sdX /mnt/kingston
echo   (replace X with the correct letter)
echo.
pause