@echo off
echo Stopping all Moses processes...
taskkill /F /IM moses.exe 2>nul
taskkill /F /IM moses-daemon.exe 2>nul
echo Done. You can now rebuild the application.
pause