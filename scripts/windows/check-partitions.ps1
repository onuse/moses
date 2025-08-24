# PowerShell script to check partitions on a disk
param(
    [Parameter(Mandatory=$true)]
    [int]$DiskNumber
)

Write-Host "Checking Disk $DiskNumber" -ForegroundColor Cyan
Write-Host "========================" 

# Get disk info
$disk = Get-Disk -Number $DiskNumber
Write-Host "Disk: $($disk.FriendlyName)"
Write-Host "Size: $([math]::Round($disk.Size / 1GB, 2)) GB"
Write-Host "Partition Style: $($disk.PartitionStyle)"
Write-Host ""

# Get partitions
Write-Host "Partitions:" -ForegroundColor Yellow
$partitions = Get-Partition -DiskNumber $DiskNumber -ErrorAction SilentlyContinue

if ($partitions) {
    foreach ($part in $partitions) {
        Write-Host "  Partition $($part.PartitionNumber):"
        Write-Host "    Type: $($part.Type)"
        Write-Host "    Size: $([math]::Round($part.Size / 1MB, 2)) MB"
        Write-Host "    Offset: $($part.Offset) bytes"
        Write-Host "    Drive Letter: $($part.DriveLetter)"
        
        # Try to get volume info
        if ($part.DriveLetter) {
            $volume = Get-Volume -DriveLetter $part.DriveLetter -ErrorAction SilentlyContinue
            if ($volume) {
                Write-Host "    FileSystem: $($volume.FileSystem)"
                Write-Host "    Label: $($volume.FileSystemLabel)"
            }
        }
        Write-Host ""
    }
} else {
    Write-Host "  No partitions found or unable to read partition table" -ForegroundColor Red
    Write-Host ""
    Write-Host "This might mean:" -ForegroundColor Yellow
    Write-Host "  1. The disk has no partition table"
    Write-Host "  2. The partition table is corrupted"
    Write-Host "  3. Windows doesn't recognize the partition type"
}

Write-Host ""
Write-Host "Raw partition table entries (if MBR):" -ForegroundColor Yellow

# Read MBR directly
$bytes = New-Object byte[] 512
$stream = [System.IO.File]::OpenRead("\\.\PHYSICALDRIVE$DiskNumber")
$stream.Read($bytes, 0, 512) | Out-Null
$stream.Close()

# Check for MBR signature
if ($bytes[510] -eq 0x55 -and $bytes[511] -eq 0xAA) {
    Write-Host "  MBR signature found (0x55AA)"
    
    # Parse partition entries (starting at offset 446)
    for ($i = 0; $i -lt 4; $i++) {
        $offset = 446 + ($i * 16)
        $entry = $bytes[$offset..($offset + 15)]
        
        # Check if partition exists (type != 0)
        if ($entry[4] -ne 0) {
            Write-Host ""
            Write-Host "  Partition Entry $($i + 1):" -ForegroundColor Green
            Write-Host "    Boot Flag: 0x$("{0:X2}" -f $entry[0])"
            Write-Host "    Type: 0x$("{0:X2}" -f $entry[4]) $(switch ($entry[4]) {
                0x06 { "(FAT16)" }
                0x07 { "(NTFS)" }
                0x0B { "(FAT32)" }
                0x0C { "(FAT32 LBA)" }
                0x83 { "(Linux)" }
                default { "(Unknown)" }
            })"
            
            # Calculate LBA start (little-endian)
            $lbaStart = [BitConverter]::ToUInt32($entry[8..11], 0)
            $lbaSize = [BitConverter]::ToUInt32($entry[12..15], 0)
            
            Write-Host "    Start LBA: $lbaStart (offset: $($lbaStart * 512) bytes)"
            Write-Host "    Size: $lbaSize sectors ($([math]::Round($lbaSize * 512 / 1MB, 2)) MB)"
        }
    }
} else {
    Write-Host "  No valid MBR signature found"
}

Write-Host ""
Write-Host "To mount the FAT16 partition manually:" -ForegroundColor Cyan
Write-Host "  1. Open Disk Management (diskmgmt.msc)"
Write-Host "  2. Look for the disk and its partition"
Write-Host "  3. Right-click the partition and assign a drive letter"
Write-Host ""
Write-Host "Or use diskpart:"
Write-Host "  diskpart"
Write-Host "  select disk $DiskNumber"
Write-Host "  list partition"
Write-Host "  select partition 1"
Write-Host "  assign letter=X"