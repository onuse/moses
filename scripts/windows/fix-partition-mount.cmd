@echo off
REM Script to force Windows to mount a partition on a USB drive

echo.
echo USB Partition Mount Fix
echo =======================
echo.

set /p DISKNUM="Enter disk number (from Disk Management): "

echo.
echo Attempting to mount partition on Disk %DISKNUM%...
echo.

echo select disk %DISKNUM% > temp_diskpart.txt
echo list partition >> temp_diskpart.txt
echo select partition 1 >> temp_diskpart.txt
echo assign >> temp_diskpart.txt

diskpart /s temp_diskpart.txt

del temp_diskpart.txt

echo.
echo If successful, the partition should now have a drive letter.
echo Check File Explorer or run this command again.
echo.
echo If it didn't work, try:
echo   1. Open Disk Management (diskmgmt.msc)
echo   2. Right-click on the partition
echo   3. Select "Change Drive Letter and Paths"
echo   4. Click "Add" and assign a letter
echo.
pause