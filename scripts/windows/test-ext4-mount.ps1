# Test script for Moses ext4 mounting on Windows
# Prerequisites: WinFsp must be installed (http://www.secfs.net/winfsp/)

param(
    [string]$SourceDrive = "E:",  # Source drive with ext4 filesystem
    [string]$MountPoint = "M:",    # Where to mount it
    [switch]$BuildFirst = $false   # Build before testing
)

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "     Moses Bridge - Ext4 Mount Test" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

# Check if running as administrator
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")
if (-not $isAdmin) {
    Write-Host "⚠️  Not running as administrator!" -ForegroundColor Yellow
    Write-Host "   Moses may need admin rights to mount filesystems." -ForegroundColor Yellow
    Write-Host ""
}

# Check if WinFsp is installed
$winFspPath = "${env:ProgramFiles(x86)}\WinFsp"
if (-not (Test-Path $winFspPath)) {
    Write-Host "❌ WinFsp not found!" -ForegroundColor Red
    Write-Host "   Please install WinFsp from: http://www.secfs.net/winfsp/" -ForegroundColor Yellow
    Write-Host "   After installation, run this script again." -ForegroundColor Yellow
    exit 1
}
Write-Host "✅ WinFsp found at: $winFspPath" -ForegroundColor Green

# Build if requested
if ($BuildFirst) {
    Write-Host ""
    Write-Host "Building Moses with mount support..." -ForegroundColor Yellow
    
    # Build with Windows mount feature
    cargo build --package moses-cli --features mount-windows --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Build failed!" -ForegroundColor Red
        exit 1
    }
    Write-Host "✅ Build successful!" -ForegroundColor Green
}

# Check if Moses CLI exists
$mosesPath = "..\..\target\release\moses.exe"
if (-not (Test-Path $mosesPath)) {
    $mosesPath = "..\..\target\debug\moses.exe"
    if (-not (Test-Path $mosesPath)) {
        Write-Host "❌ Moses CLI not found!" -ForegroundColor Red
        Write-Host "   Run with -BuildFirst flag or build manually:" -ForegroundColor Yellow
        Write-Host "   cargo build --package moses-cli --features mount-windows" -ForegroundColor Yellow
        exit 1
    }
}
Write-Host "✅ Moses CLI found at: $mosesPath" -ForegroundColor Green

# List available drives
Write-Host ""
Write-Host "Available drives:" -ForegroundColor Cyan
& $mosesPath list

# Check if mount point is available
if (Test-Path "${MountPoint}\") {
    Write-Host "⚠️  Mount point $MountPoint already exists!" -ForegroundColor Yellow
    Write-Host "   Please choose a different drive letter." -ForegroundColor Yellow
    exit 1
}

# Test the mount command
Write-Host ""
Write-Host "Testing mount command..." -ForegroundColor Cyan
Write-Host "Command: moses mount $SourceDrive $MountPoint" -ForegroundColor Gray
Write-Host ""

& $mosesPath mount $SourceDrive $MountPoint --readonly

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "✅ Mount command executed successfully!" -ForegroundColor Green
    
    # Check if mount point exists
    if (Test-Path "${MountPoint}\") {
        Write-Host "✅ Mount point $MountPoint is accessible!" -ForegroundColor Green
        Write-Host ""
        Write-Host "You can now:" -ForegroundColor Cyan
        Write-Host "  1. Open $MountPoint in Windows Explorer" -ForegroundColor White
        Write-Host "  2. Browse ext4 files natively" -ForegroundColor White
        Write-Host "  3. Copy files from ext4 to Windows" -ForegroundColor White
        Write-Host ""
        Write-Host "To unmount, run:" -ForegroundColor Yellow
        Write-Host "  moses unmount $MountPoint" -ForegroundColor White
    } else {
        Write-Host "⚠️  Mount point not accessible yet." -ForegroundColor Yellow
        Write-Host "   The filesystem may still be initializing." -ForegroundColor Yellow
    }
} else {
    Write-Host ""
    Write-Host "❌ Mount command failed!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Troubleshooting:" -ForegroundColor Yellow
    Write-Host "  1. Make sure $SourceDrive contains an ext4 filesystem" -ForegroundColor White
    Write-Host "  2. Run this script as administrator" -ForegroundColor White
    Write-Host "  3. Ensure WinFsp service is running" -ForegroundColor White
    Write-Host "  4. Check if $MountPoint is available" -ForegroundColor White
}

Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan