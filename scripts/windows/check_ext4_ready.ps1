# Moses EXT4 Formatter - Readiness Check
Write-Host "Moses EXT4 Formatter - System Readiness Check" -ForegroundColor Cyan
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""

$ready = $true

# Check 1: WSL2 Installation
Write-Host "Checking WSL2 installation..." -ForegroundColor Yellow
try {
    $wslStatus = wsl --status 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✓ WSL2 is installed" -ForegroundColor Green
    } else {
        Write-Host "  ✗ WSL2 is not installed" -ForegroundColor Red
        Write-Host "    To install: wsl --install" -ForegroundColor Gray
        $ready = $false
    }
} catch {
    Write-Host "  ✗ WSL2 is not installed" -ForegroundColor Red
    Write-Host "    To install: wsl --install" -ForegroundColor Gray
    $ready = $false
}

# Check 2: WSL Distribution
Write-Host "Checking WSL distributions..." -ForegroundColor Yellow
$distros = wsl -l -q 2>&1
if ($LASTEXITCODE -eq 0 -and $distros) {
    Write-Host "  ✓ WSL distributions found:" -ForegroundColor Green
    $distros | ForEach-Object { if ($_) { Write-Host "    - $_" -ForegroundColor Gray } }
} else {
    Write-Host "  ✗ No WSL distribution installed" -ForegroundColor Red
    Write-Host "    To install Ubuntu: wsl --install -d Ubuntu" -ForegroundColor Gray
    $ready = $false
}

# Check 3: Check mkfs.ext4 in WSL
if ($ready) {
    Write-Host "Checking for mkfs.ext4 in WSL..." -ForegroundColor Yellow
    $mkfsCheck = wsl which mkfs.ext4 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✓ mkfs.ext4 is available in WSL" -ForegroundColor Green
    } else {
        Write-Host "  ⚠ mkfs.ext4 not found (will be installed automatically)" -ForegroundColor Yellow
    }
}

# Check 4: Find Kingston DataTraveler
Write-Host "Checking for Kingston DataTraveler..." -ForegroundColor Yellow
$kingston = Get-WmiObject Win32_DiskDrive | Where-Object { $_.Model -like "*Kingston DataTraveler*" }
if ($kingston) {
    Write-Host "  ✓ Kingston DataTraveler found:" -ForegroundColor Green
    Write-Host "    Device: $($kingston.DeviceID)" -ForegroundColor Gray
    Write-Host "    Model: $($kingston.Model)" -ForegroundColor Gray
    Write-Host "    Size: $([math]::Round($kingston.Size / 1GB, 2)) GB" -ForegroundColor Gray
    
    # Check if it's PHYSICALDRIVE2
    if ($kingston.DeviceID -eq "\\.\PHYSICALDRIVE2") {
        Write-Host "    Expected WSL path: /dev/sdc" -ForegroundColor Gray
    }
} else {
    Write-Host "  ✗ Kingston DataTraveler not found" -ForegroundColor Red
    Write-Host "    Please connect your USB drive" -ForegroundColor Gray
    $ready = $false
}

# Check 5: Rust/Cargo
Write-Host "Checking Rust installation..." -ForegroundColor Yellow
try {
    $cargoVersion = cargo --version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✓ Cargo is installed: $cargoVersion" -ForegroundColor Green
    } else {
        Write-Host "  ✗ Cargo not found" -ForegroundColor Red
        Write-Host "    Install from: https://rustup.rs/" -ForegroundColor Gray
        $ready = $false
    }
} catch {
    Write-Host "  ✗ Cargo not found" -ForegroundColor Red
    Write-Host "    Install from: https://rustup.rs/" -ForegroundColor Gray
    $ready = $false
}

Write-Host ""
Write-Host "=============================================" -ForegroundColor Cyan

if ($ready) {
    Write-Host "✓ System is ready for EXT4 formatting!" -ForegroundColor Green
    Write-Host ""
    Write-Host "To format your Kingston DataTraveler:" -ForegroundColor White
    Write-Host "  1. Run: format_kingston_ext4.bat" -ForegroundColor Gray
    Write-Host "  2. Or manually: .\target\debug\moses.exe format `"Kingston DataTraveler`" ext4" -ForegroundColor Gray
} else {
    Write-Host "✗ System is not ready. Please fix the issues above." -ForegroundColor Red
}

Write-Host ""