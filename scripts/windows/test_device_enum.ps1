# Test Windows device enumeration
# Run this script in PowerShell on Windows to test the device enumeration

Write-Host "Testing Windows Device Enumeration" -ForegroundColor Cyan
Write-Host "==================================`n" -ForegroundColor Cyan

# Test 1: Get-Disk command
Write-Host "Test 1: PowerShell Get-Disk" -ForegroundColor Yellow
Get-Disk | Select-Object Number, FriendlyName, Size, PartitionStyle, BusType, MediaType, IsSystem, IsBoot | Format-Table

# Test 2: WMI Win32_DiskDrive
Write-Host "`nTest 2: WMI Win32_DiskDrive" -ForegroundColor Yellow
Get-WmiObject Win32_DiskDrive | Select-Object DeviceID, Model, Size, MediaType, InterfaceType | Format-Table

# Test 3: Get Partitions for each disk
Write-Host "`nTest 3: Partitions by Disk" -ForegroundColor Yellow
$disks = Get-Disk
foreach ($disk in $disks) {
    Write-Host "`nDisk $($disk.Number): $($disk.FriendlyName)" -ForegroundColor Green
    Get-Partition -DiskNumber $disk.Number | Select-Object PartitionNumber, DriveLetter, Size, Type | Format-Table
}

# Test 4: Check if running as Administrator
Write-Host "`nTest 4: Admin Status" -ForegroundColor Yellow
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] 'Administrator')
Write-Host "Running as Administrator: $isAdmin"

Write-Host "`n==================================`n" -ForegroundColor Cyan
Write-Host "If the above commands work, the Moses device enumeration should work too!" -ForegroundColor Green
Write-Host "To test Moses CLI on Windows:" -ForegroundColor White
Write-Host "1. Build on Windows: cargo build --package moses-cli" -ForegroundColor Gray
Write-Host "2. Run: .\target\debug\moses.exe list" -ForegroundColor Gray