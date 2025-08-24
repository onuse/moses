# Diagnose why Windows isn't showing the FAT16 partition

param(
    [Parameter(Mandatory=$true)]
    [int]$DiskNumber
)

Write-Host "`nDiagnosing Partition Visibility Issue" -ForegroundColor Cyan
Write-Host "=====================================" 
Write-Host ""

# Get disk info
$disk = Get-Disk -Number $DiskNumber
Write-Host "Disk $DiskNumber: $($disk.FriendlyName)" -ForegroundColor Yellow
Write-Host "  Partition Style: $($disk.PartitionStyle)"

# Check what Windows sees
$partitions = Get-Partition -DiskNumber $DiskNumber -ErrorAction SilentlyContinue
if ($partitions) {
    Write-Host "  Windows sees $($partitions.Count) partition(s)" -ForegroundColor Green
} else {
    Write-Host "  Windows sees NO partitions!" -ForegroundColor Red
}

# Read MBR directly to see what's really there
Write-Host "`nReading MBR directly..." -ForegroundColor Yellow
$bytes = New-Object byte[] 512
$stream = [System.IO.File]::OpenRead("\\.\PHYSICALDRIVE$DiskNumber")
$stream.Read($bytes, 0, 512) | Out-Null
$stream.Close()

# Check partition entries
Write-Host "Partition Table Entries in MBR:"
for ($i = 0; $i -lt 4; $i++) {
    $offset = 446 + ($i * 16)
    $entry = $bytes[$offset..($offset + 15)]
    
    if ($entry[4] -ne 0) {  # Type != 0 means partition exists
        $type = $entry[4]
        $bootable = $entry[0]
        $lbaStart = [BitConverter]::ToUInt32($entry[8..11], 0)
        $lbaSize = [BitConverter]::ToUInt32($entry[12..15], 0)
        
        Write-Host "  Partition $($i+1): Type=0x$("{0:X2}" -f $type), Start=$lbaStart, Size=$lbaSize sectors" -ForegroundColor Green
        
        # Check partition type
        $typeName = switch ($type) {
            0x06 { "FAT16" }
            0x0E { "FAT16 LBA" }
            0x0C { "FAT32 LBA" }
            0x07 { "NTFS" }
            default { "Unknown" }
        }
        Write-Host "    Type Name: $typeName"
        Write-Host "    Bootable: $(if ($bootable -eq 0x80) { 'Yes' } else { 'No' })"
    }
}

Write-Host "`nPossible Issues:" -ForegroundColor Red
Write-Host "=================="

# Issue 1: Partition not recognized
if ($disk.PartitionStyle -eq "RAW") {
    Write-Host "❌ Windows thinks this disk is RAW (uninitialized)"
    Write-Host "   Even though there's an MBR, Windows isn't recognizing it"
}

# Issue 2: USB drives with partitions
if ($disk.BusType -eq "USB") {
    Write-Host "⚠️  This is a USB device with partition table"
    Write-Host "   Windows may not auto-mount partitions on removable media"
    
    # Check if Windows Disk Management service is running
    $service = Get-Service -Name "ShellHWDetection" -ErrorAction SilentlyContinue
    if ($service -and $service.Status -ne "Running") {
        Write-Host "❌ Shell Hardware Detection service is not running"
    }
}

# Issue 3: Check if partition needs initialization
Write-Host "`nTrying to force Windows to recognize the partition..." -ForegroundColor Yellow

try {
    # Try to rescan
    Update-Disk -Number $DiskNumber -ErrorAction SilentlyContinue
    
    # Try diskpart commands
    $diskpartScript = @"
select disk $DiskNumber
rescan
list partition
"@
    $diskpartScript | diskpart | Out-String | Write-Host
    
} catch {
    Write-Host "Could not refresh partition information"
}

Write-Host "`nSolutions to Try:" -ForegroundColor Green
Write-Host "=================="
Write-Host "1. Initialize in Disk Management:"
Write-Host "   - Open diskmgmt.msc"
Write-Host "   - Right-click the disk"
Write-Host "   - If it shows 'Initialize Disk', Windows isn't seeing the MBR"
Write-Host ""
Write-Host "2. Manual partition mount:"
Write-Host "   diskpart"
Write-Host "   select disk $DiskNumber"
Write-Host "   select partition 1"
Write-Host "   assign letter=Z"
Write-Host ""
Write-Host "3. Use partition tools:"
Write-Host "   - MiniTool Partition Wizard"
Write-Host "   - EaseUS Partition Master"
Write-Host "   These often see partitions Windows misses"
Write-Host ""
Write-Host "4. Format without partition table:"
Write-Host "   This is the most reliable for USB drives"