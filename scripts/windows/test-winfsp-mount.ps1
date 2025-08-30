# Moses WinFsp Mount Test Script
# Tests filesystem mounting on Windows using WinFsp

param(
    [string]$Source = "",
    [string]$MountPoint = "M:",
    [string]$FileSystem = "",
    [switch]$BuildFirst,
    [switch]$CreateTestImage,
    [switch]$Help
)

# Colors for output
$Host.UI.RawUI.ForegroundColor = "White"

function Write-Success {
    param([string]$Message)
    Write-Host "✅ $Message" -ForegroundColor Green
}

function Write-Error {
    param([string]$Message)
    Write-Host "❌ $Message" -ForegroundColor Red
}

function Write-Warning {
    param([string]$Message)
    Write-Host "⚠️  $Message" -ForegroundColor Yellow
}

function Write-Info {
    param([string]$Message)
    Write-Host "ℹ️  $Message" -ForegroundColor Cyan
}

# Show help
if ($Help) {
    Write-Host @"
Moses WinFsp Mount Test Script

Usage: .\test-winfsp-mount.ps1 [options]

Options:
    -Source <path>      Source device or image file (required unless -CreateTestImage)
    -MountPoint <letter> Mount point drive letter (default: M:)
    -FileSystem <type>   Filesystem type (auto-detect if not specified)
    -BuildFirst         Build Moses before testing
    -CreateTestImage    Create and use a test FAT32 image
    -Help              Show this help message

Examples:
    .\test-winfsp-mount.ps1 -Source E: -MountPoint M:
    .\test-winfsp-mount.ps1 -Source disk.img -FileSystem ext4
    .\test-winfsp-mount.ps1 -CreateTestImage -BuildFirst
"@
    exit 0
}

Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "     Moses Bridge - WinFsp Mount Test" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

# Check if running as Administrator
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")
if (-not $isAdmin) {
    Write-Warning "Not running as Administrator!"
    Write-Warning "Some operations may require admin privileges."
    Write-Host ""
}

# Check if WinFsp is installed
Write-Info "Checking for WinFsp installation..."
$winfspService = Get-Service -Name "WinFsp.Launcher" -ErrorAction SilentlyContinue
if ($null -eq $winfspService) {
    Write-Error "WinFsp is not installed!"
    Write-Host ""
    Write-Host "Please install WinFsp from: https://winfsp.dev/" -ForegroundColor Yellow
    Write-Host "Or using Chocolatey: choco install winfsp" -ForegroundColor Yellow
    Write-Host "Or using Scoop: scoop install winfsp" -ForegroundColor Yellow
    exit 1
}
Write-Success "WinFsp is installed (Service: $($winfspService.Status))"

# Build if requested
if ($BuildFirst) {
    Write-Host ""
    Write-Info "Building Moses with WinFsp support..."
    
    $projectRoot = (Get-Item $PSScriptRoot).Parent.Parent.FullName
    Push-Location $projectRoot
    
    $buildResult = cargo build --release --features mount-windows -p moses-cli 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Build failed!"
        Write-Host $buildResult
        Pop-Location
        exit 1
    }
    
    Pop-Location
    Write-Success "Build successful!"
}

# Find Moses executable
$mosesPath = ""
$projectRoot = (Get-Item $PSScriptRoot).Parent.Parent.FullName

if (Test-Path "$projectRoot\target\release\moses.exe") {
    $mosesPath = "$projectRoot\target\release\moses.exe"
} elseif (Test-Path "$projectRoot\target\debug\moses.exe") {
    $mosesPath = "$projectRoot\target\debug\moses.exe"
} else {
    $mosesCmd = Get-Command moses -ErrorAction SilentlyContinue
    if ($mosesCmd) {
        $mosesPath = $mosesCmd.Path
    }
}

if ($mosesPath -eq "") {
    Write-Error "Moses CLI not found!"
    Write-Host "Build with: cargo build --release --features mount-windows -p moses-cli" -ForegroundColor Yellow
    exit 1
}
Write-Success "Moses CLI found at: $mosesPath"

# Create test image if requested
if ($CreateTestImage) {
    Write-Host ""
    Write-Info "Creating test FAT32 image..."
    
    $testImage = "$env:TEMP\moses-test-$([System.Guid]::NewGuid().ToString('N').Substring(0,8)).img"
    
    # Create a 100MB image file
    $fs = [System.IO.File]::Create($testImage)
    $fs.SetLength(100MB)
    $fs.Close()
    
    # Format as FAT32 using diskpart
    $diskpartScript = @"
select vdisk file="$testImage"
attach vdisk
create partition primary
format fs=fat32 quick
assign letter=Z
"@
    
    $scriptFile = "$env:TEMP\diskpart-script.txt"
    $diskpartScript | Out-File -FilePath $scriptFile -Encoding ASCII
    
    Write-Info "Formatting image as FAT32..."
    $result = diskpart /s $scriptFile 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        # Add some test files
        if (Test-Path "Z:\") {
            "Test content" | Out-File -FilePath "Z:\test.txt"
            New-Item -ItemType Directory -Path "Z:\testdir" -Force | Out-Null
            "Nested content" | Out-File -FilePath "Z:\testdir\nested.txt"
            
            # Detach the VHD
            $detachScript = @"
select vdisk file="$testImage"
detach vdisk
"@
            $detachScript | Out-File -FilePath $scriptFile -Encoding ASCII
            diskpart /s $scriptFile | Out-Null
        }
        
        Remove-Item $scriptFile -Force
        
        $Source = $testImage
        Write-Success "Test image created: $testImage"
    } else {
        Write-Error "Failed to create test image"
        Remove-Item $scriptFile -Force -ErrorAction SilentlyContinue
        Remove-Item $testImage -Force -ErrorAction SilentlyContinue
        exit 1
    }
}

# Validate source
if ($Source -eq "") {
    Write-Error "Source not specified! Use -Source parameter or -CreateTestImage"
    exit 1
}

if (-not (Test-Path $Source)) {
    # Check if it's a device
    if (-not $Source.StartsWith("\\.\")) {
        Write-Error "Source not found: $Source"
        exit 1
    }
}

# Ensure mount point format
if (-not $MountPoint.EndsWith(":")) {
    $MountPoint = "${MountPoint}:"
}

# Check if mount point is already in use
if (Test-Path "${MountPoint}\") {
    Write-Warning "Mount point $MountPoint is already in use!"
    $response = Read-Host "Continue anyway? (y/n)"
    if ($response -ne 'y') {
        exit 0
    }
}

# List available devices
Write-Host ""
Write-Info "Available devices:"
& $mosesPath list

# Test the mount
Write-Host ""
Write-Info "Testing mount..."

$mountArgs = @("mount", $Source, $MountPoint, "--readonly")
if ($FileSystem -ne "") {
    $mountArgs += @("--fs-type", $FileSystem)
}

Write-Host "Command: moses $($mountArgs -join ' ')" -ForegroundColor Gray

$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
$result = & $mosesPath $mountArgs 2>&1
$stopwatch.Stop()

if ($LASTEXITCODE -eq 0) {
    Write-Success "Mount completed in $($stopwatch.ElapsedMilliseconds)ms"
    
    # Wait a moment for mount to stabilize
    Start-Sleep -Seconds 2
    
    # Check if mount is accessible
    if (Test-Path "${MountPoint}\") {
        Write-Success "Mount point is accessible!"
        
        Write-Host ""
        Write-Info "Mount statistics:"
        Get-PSDrive -Name $MountPoint.TrimEnd(':') -ErrorAction SilentlyContinue | Format-Table
        
        Write-Host ""
        Write-Info "Root directory contents:"
        Get-ChildItem "${MountPoint}\" | Select-Object Mode, LastWriteTime, Length, Name | Format-Table
        
        # Try to read a file if it exists
        $testFile = Get-ChildItem "${MountPoint}\" -File | Select-Object -First 1
        if ($testFile) {
            Write-Host ""
            Write-Info "Reading test file: $($testFile.Name)"
            Get-Content "$($testFile.FullName)" -TotalCount 5 | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
        }
        
        Write-Host ""
        Write-Success "Mount test successful!"
        Write-Host ""
        Write-Host "You can now:" -ForegroundColor Green
        Write-Host "  1. Browse files in Windows Explorer at $MountPoint" -ForegroundColor Green
        Write-Host "  2. Copy files: Copy-Item ${MountPoint}\file.txt C:\Temp\" -ForegroundColor Green
        Write-Host "  3. Use any Windows application with the mounted filesystem" -ForegroundColor Green
        Write-Host ""
        Write-Host "To unmount, run:" -ForegroundColor Yellow
        Write-Host "  moses unmount $MountPoint" -ForegroundColor Yellow
        
    } else {
        Write-Warning "Mount command succeeded but mount point is not accessible"
        Write-Host "This might indicate a WinFsp configuration issue" -ForegroundColor Yellow
    }
} else {
    Write-Error "Mount failed!"
    Write-Host $result
    
    Write-Host ""
    Write-Host "Troubleshooting:" -ForegroundColor Yellow
    Write-Host "  1. Ensure WinFsp is properly installed" -ForegroundColor Yellow
    Write-Host "  2. Try running as Administrator" -ForegroundColor Yellow
    Write-Host "  3. Check if $MountPoint is available" -ForegroundColor Yellow
    Write-Host "  4. Verify the source contains a supported filesystem" -ForegroundColor Yellow
    Write-Host "  5. Try specifying -FileSystem explicitly" -ForegroundColor Yellow
}

# Cleanup test image if created
if ($CreateTestImage -and $Source -ne "") {
    Write-Host ""
    $cleanup = Read-Host "Delete test image? (y/n)"
    if ($cleanup -eq 'y') {
        Remove-Item $Source -Force -ErrorAction SilentlyContinue
        Write-Success "Test image deleted"
    }
}

Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan