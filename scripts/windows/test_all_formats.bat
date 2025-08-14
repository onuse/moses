@echo off
echo =========================================================
echo Moses Multi-Format Testing Script
echo =========================================================
echo.
echo SAFETY WARNING: This will format your USB drive!
echo Please ensure you have selected the correct drive.
echo.
echo Available formats to test:
echo   1. EXT4  (via WSL2)
echo   2. NTFS  (native Windows)
echo   3. FAT32 (native Windows)
echo   4. exFAT (native Windows)
echo.
pause

echo.
echo Building Moses CLI...
cargo build --package moses-cli --release
if %errorlevel% neq 0 (
    echo Build failed!
    pause
    exit /b %errorlevel%
)

echo.
echo =========================================================
echo STEP 1: List Available Devices
echo =========================================================
target\release\moses.exe list

echo.
echo Please note your USB drive name (e.g., "Kingston DataTraveler")
echo.
set /p DEVICE_NAME="Enter your USB drive name exactly as shown above: "

echo.
echo =========================================================
echo FORMAT TESTING - DRY RUN FIRST
echo =========================================================
echo.

echo Testing EXT4 (dry run)...
target\release\moses.exe format "%DEVICE_NAME%" ext4 --dry-run

echo.
echo Testing NTFS (dry run)...
target\release\moses.exe format "%DEVICE_NAME%" ntfs --dry-run

echo.
echo Testing FAT32 (dry run)...
target\release\moses.exe format "%DEVICE_NAME%" fat32 --dry-run

echo.
echo Testing exFAT (dry run)...
target\release\moses.exe format "%DEVICE_NAME%" exfat --dry-run

echo.
echo =========================================================
echo DRY RUNS COMPLETE - Ready for actual formatting
echo =========================================================
echo.
echo Which format would you like to test first?
echo   1. EXT4
echo   2. NTFS
echo   3. FAT32
echo   4. exFAT
echo   5. Exit
echo.
set /p FORMAT_CHOICE="Enter your choice (1-5): "

if "%FORMAT_CHOICE%"=="1" goto FORMAT_EXT4
if "%FORMAT_CHOICE%"=="2" goto FORMAT_NTFS
if "%FORMAT_CHOICE%"=="3" goto FORMAT_FAT32
if "%FORMAT_CHOICE%"=="4" goto FORMAT_EXFAT
if "%FORMAT_CHOICE%"=="5" goto END
goto END

:FORMAT_EXT4
echo.
echo Formatting as EXT4...
target\release\moses.exe format "%DEVICE_NAME%" ext4
goto VERIFY

:FORMAT_NTFS
echo.
echo Formatting as NTFS...
target\release\moses.exe format "%DEVICE_NAME%" ntfs
goto VERIFY

:FORMAT_FAT32
echo.
echo Formatting as FAT32...
target\release\moses.exe format "%DEVICE_NAME%" fat32
goto VERIFY

:FORMAT_EXFAT
echo.
echo Formatting as exFAT...
target\release\moses.exe format "%DEVICE_NAME%" exfat
goto VERIFY

:VERIFY
echo.
echo =========================================================
echo FORMAT COMPLETE - Verification
echo =========================================================
echo.
echo To verify the format worked:
echo   1. Open File Explorer
echo   2. Check if the drive appears with the new filesystem
echo   3. Try writing a test file to the drive
echo.
echo For EXT4: The drive may not be visible in Windows Explorer.
echo          To verify EXT4, run: wsl lsblk -f
echo.

:END
echo.
echo Test another format? (Y/N)
set /p CONTINUE="Enter Y to continue or N to exit: "
if /i "%CONTINUE%"=="Y" goto :FORMAT_TESTING
echo.
echo Testing complete!
pause