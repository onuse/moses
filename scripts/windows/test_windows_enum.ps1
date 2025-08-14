# Test Windows device enumeration
# Run this from PowerShell to verify the commands work

Write-Host "Testing Windows Device Enumeration" -ForegroundColor Cyan
Write-Host "===================================" -ForegroundColor Cyan

Write-Host "`n1. Getting Disks via Get-Disk:" -ForegroundColor Yellow
Get-Disk | Format-Table Number, FriendlyName, Size, BusType, IsSystem, IsBoot -AutoSize

Write-Host "`n2. Getting Disks as JSON:" -ForegroundColor Yellow
$disks = Get-Disk | Select-Object Number, FriendlyName, Size, PartitionStyle, BusType, MediaType, IsSystem, IsBoot
$disks | ConvertTo-Json | Write-Host

Write-Host "`n3. Getting WMI Disk Drives:" -ForegroundColor Yellow
Get-WmiObject Win32_DiskDrive | Format-Table DeviceID, Model, Size, MediaType, InterfaceType -AutoSize

Write-Host "`n4. Getting Partitions for each disk:" -ForegroundColor Yellow
foreach ($disk in $disks) {
    Write-Host "  Disk $($disk.Number) ($($disk.FriendlyName)):" -ForegroundColor Green
    Get-Partition -DiskNumber $disk.Number -ErrorAction SilentlyContinue | 
        Format-Table PartitionNumber, DriveLetter, Size, Type -AutoSize
}

Write-Host "`n5. Checking Admin Status:" -ForegroundColor Yellow
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] 'Administrator')
Write-Host "  Running as Administrator: $isAdmin" -ForegroundColor $(if ($isAdmin) {'Green'} else {'Red'})