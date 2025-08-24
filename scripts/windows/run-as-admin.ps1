# PowerShell script to run Moses with administrator privileges
# This is useful for development and testing

$scriptPath = Split-Path -Parent $MyInvocation.MyCommand.Path
$exePath = Join-Path $scriptPath "target\release\moses.exe"
$tauriExePath = Join-Path $scriptPath "src-tauri\target\release\moses.exe"

# Check which executable exists
if (Test-Path $tauriExePath) {
    $targetExe = $tauriExePath
    Write-Host "Found Tauri build at: $tauriExePath"
} elseif (Test-Path $exePath) {
    $targetExe = $exePath
    Write-Host "Found CLI build at: $exePath"
} else {
    Write-Host "Moses executable not found. Please build the project first:" -ForegroundColor Red
    Write-Host "  cargo build --release" -ForegroundColor Yellow
    Write-Host "  or" -ForegroundColor Gray
    Write-Host "  npm run tauri build" -ForegroundColor Yellow
    exit 1
}

# Check if running as administrator
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")

if ($isAdmin) {
    Write-Host "Already running as Administrator. Starting Moses..." -ForegroundColor Green
    & $targetExe
} else {
    Write-Host "Requesting Administrator privileges..." -ForegroundColor Yellow
    Start-Process PowerShell -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -Command `"& '$targetExe'`""
}