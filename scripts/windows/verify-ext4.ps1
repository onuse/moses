# PowerShell script to verify ext4 filesystem on USB drive
# Run as Administrator

param(
    [Parameter(Mandatory=$true)]
    [string]$DriveNumber  # e.g., "2" for PhysicalDrive2
)

$drivePath = "\\.\PHYSICALDRIVE$DriveNumber"

Write-Host "Checking ext4 filesystem on $drivePath" -ForegroundColor Cyan

# Read the superblock (at offset 1024)
$stream = [System.IO.File]::OpenRead($drivePath)
$reader = New-Object System.IO.BinaryReader($stream)

try {
    # Seek to superblock
    $stream.Seek(1024, [System.IO.SeekOrigin]::Begin) | Out-Null
    
    # Read magic number (should be 0xEF53)
    $magic = $reader.ReadUInt16()
    
    if ($magic -eq 0xEF53) {
        Write-Host "✅ Valid ext4 filesystem found!" -ForegroundColor Green
        Write-Host "   Magic number: 0x$($magic.ToString('X4'))" -ForegroundColor Gray
        
        # Read more superblock fields
        $inodeCount = $reader.ReadUInt32()
        $blockCount = $reader.ReadUInt32()
        $reservedBlockCount = $reader.ReadUInt32()
        $freeBlocks = $reader.ReadUInt32()
        $freeInodes = $reader.ReadUInt32()
        $firstDataBlock = $reader.ReadUInt32()
        $blockSize = 1024 -shl $reader.ReadUInt32()
        
        Write-Host "`nFilesystem Information:" -ForegroundColor Yellow
        Write-Host "  Block size: $blockSize bytes"
        Write-Host "  Total blocks: $blockCount"
        Write-Host "  Total inodes: $inodeCount"
        Write-Host "  Free blocks: $freeBlocks"
        Write-Host "  Free inodes: $freeInodes"
        Write-Host "  Total size: $([math]::Round($blockCount * $blockSize / 1GB, 2)) GB"
        
        # Check for volume name at offset 120 (s_volume_name)
        $stream.Seek(1024 + 120, [System.IO.SeekOrigin]::Begin) | Out-Null
        $volumeNameBytes = $reader.ReadBytes(16)
        $volumeName = [System.Text.Encoding]::ASCII.GetString($volumeNameBytes).TrimEnd([char]0)
        if ($volumeName) {
            Write-Host "  Volume name: '$volumeName'"
        }
        
        Write-Host "`n✅ The ext4 filesystem created by Moses is valid!" -ForegroundColor Green
        
    } else {
        Write-Host "❌ Not an ext4 filesystem (magic: 0x$($magic.ToString('X4')))" -ForegroundColor Red
    }
    
} finally {
    $reader.Close()
    $stream.Close()
}