# Moses USB Drive Diagnostic Script
# Run as Administrator for best results

Write-Host "======================================" -ForegroundColor Cyan
Write-Host "Moses USB Drive Diagnostic" -ForegroundColor Cyan
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""

# 1. Check all disks
Write-Host "1. All Disks in System:" -ForegroundColor Yellow
Get-Disk | Format-Table Number, FriendlyName, Size, PartitionStyle, OperationalStatus, HealthStatus -AutoSize

# 2. Check Disk 2 specifically
Write-Host "`n2. Disk 2 Details:" -ForegroundColor Yellow
try {
    $disk = Get-Disk -Number 2 -ErrorAction Stop
    $disk | Format-List *
    
    # Check if offline
    if ($disk.OperationalStatus -eq "Offline") {
        Write-Host "ISSUE: Disk is OFFLINE!" -ForegroundColor Red
        Write-Host "Try: Set-Disk -Number 2 -IsOffline `$false" -ForegroundColor Green
    }
    
    # Check partitions
    Write-Host "`n3. Partitions on Disk 2:" -ForegroundColor Yellow
    Get-Partition -DiskNumber 2 -ErrorAction SilentlyContinue | Format-Table -AutoSize
    
    # Check volumes
    Write-Host "`n4. Volumes on Disk 2:" -ForegroundColor Yellow
    Get-Partition -DiskNumber 2 -ErrorAction SilentlyContinue | Get-Volume -ErrorAction SilentlyContinue | Format-Table -AutoSize
    
} catch {
    Write-Host "ERROR: Cannot access Disk 2" -ForegroundColor Red
    Write-Host $_.Exception.Message
}

# 3. Check USB devices
Write-Host "`n5. USB Storage Devices:" -ForegroundColor Yellow
Get-PnpDevice -Class DiskDrive | Where-Object { $_.InstanceId -like "*USB*" } | Format-Table Status, FriendlyName -AutoSize

# 4. Check WMI for disk info
Write-Host "`n6. WMI Disk Information:" -ForegroundColor Yellow
Get-WmiObject Win32_DiskDrive | Where-Object { $_.Index -eq 2 } | Format-List Model, Status, StatusInfo, LastErrorCode, InterfaceType, MediaType

# 5. Check for MBR signature
Write-Host "`n7. MBR Analysis:" -ForegroundColor Yellow
try {
    $bytes = New-Object byte[] 512
    $stream = [System.IO.File]::OpenRead("\\.\PHYSICALDRIVE2")
    $null = $stream.Read($bytes, 0, 512)
    $stream.Close()
    
    # Check boot signature
    $bootSig = "{0:X2}{1:X2}" -f $bytes[0x1FE], $bytes[0x1FF]
    Write-Host "Boot Signature: $bootSig $(if($bootSig -eq '55AA'){'✓ Valid'}else{'✗ Invalid!'})" -ForegroundColor $(if($bootSig -eq '55AA'){'Green'}else{'Red'})
    
    # Check disk signature
    $diskSig = "{0:X2}{1:X2}{2:X2}{3:X2}" -f $bytes[0x1BB], $bytes[0x1BA], $bytes[0x1B9], $bytes[0x1B8]
    Write-Host "Disk Signature: $diskSig $(if($diskSig -eq '00000000'){'✗ Missing!'}else{'✓ Present'})" -ForegroundColor $(if($diskSig -eq '00000000'){'Red'}else{'Green'})
    
    # Check partition type
    $partType = "{0:X2}" -f $bytes[0x1C2]
    Write-Host "Partition 1 Type: 0x$partType $(switch($partType){'06'{'(FAT16)'}'0B'{'(FAT32)'}'07'{'(NTFS)'}'00'{'(Empty)'}default{''}})"
    
    # Show first 16 bytes
    Write-Host "`nFirst 16 bytes of MBR:"
    $hex = ($bytes[0..15] | ForEach-Object { "{0:X2}" -f $_ }) -join " "
    Write-Host $hex
    
} catch {
    Write-Host "ERROR: Cannot read MBR" -ForegroundColor Red
    Write-Host $_.Exception.Message
}

# 6. Recommendations
Write-Host "`n8. Diagnosis:" -ForegroundColor Yellow

# Check if disk signature is missing
if ($diskSig -eq '00000000') {
    Write-Host "⚠ PROBLEM: Missing MBR disk signature!" -ForegroundColor Red
    Write-Host "  FIX: Run in diskpart:" -ForegroundColor Yellow
    Write-Host "    select disk 2" -ForegroundColor Gray
    Write-Host "    uniqueid disk id=12345678" -ForegroundColor Gray
}

# Check if disk is offline
if ($disk.OperationalStatus -eq "Offline") {
    Write-Host "⚠ PROBLEM: Disk is offline!" -ForegroundColor Red
    Write-Host "  FIX: Run in PowerShell (Admin):" -ForegroundColor Yellow
    Write-Host "    Set-Disk -Number 2 -IsOffline `$false" -ForegroundColor Gray
}

# Check if no drive letter
$volumes = Get-Partition -DiskNumber 2 -ErrorAction SilentlyContinue | Get-Volume -ErrorAction SilentlyContinue
if ($volumes -and -not $volumes.DriveLetter) {
    Write-Host "⚠ PROBLEM: No drive letter assigned!" -ForegroundColor Red
    Write-Host "  FIX: Run in diskpart:" -ForegroundColor Yellow
    Write-Host "    select disk 2" -ForegroundColor Gray
    Write-Host "    select partition 1" -ForegroundColor Gray
    Write-Host "    assign letter=Z" -ForegroundColor Gray
}

Write-Host "`nDone!" -ForegroundColor Green