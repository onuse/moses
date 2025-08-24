# Fix missing disk signature in MBR
param(
    [Parameter(Mandatory=$true)]
    [int]$DiskNumber
)

Write-Host "Adding disk signature to MBR on Disk $DiskNumber" -ForegroundColor Yellow

# Generate a random disk signature
$signature = Get-Random -Maximum 2147483647

# Open disk for writing
$disk = "\\.\PHYSICALDRIVE$DiskNumber"
$stream = [System.IO.File]::Open($disk, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::ReadWrite)

# Read current MBR
$mbr = New-Object byte[] 512
$stream.Read($mbr, 0, 512) | Out-Null

# Check if MBR is valid
if ($mbr[510] -eq 0x55 -and $mbr[511] -eq 0xAA) {
    Write-Host "Valid MBR found" -ForegroundColor Green
    
    # Check current disk signature
    $currentSig = [BitConverter]::ToUInt32($mbr[440..443], 0)
    if ($currentSig -eq 0) {
        Write-Host "No disk signature found, adding one..." -ForegroundColor Yellow
        
        # Add disk signature
        $sigBytes = [BitConverter]::GetBytes($signature)
        for ($i = 0; $i -lt 4; $i++) {
            $mbr[440 + $i] = $sigBytes[$i]
        }
        
        # Write back
        $stream.Seek(0, [System.IO.SeekOrigin]::Begin) | Out-Null
        $stream.Write($mbr, 0, 512)
        $stream.Flush()
        
        Write-Host "Disk signature added: 0x$($signature.ToString('X8'))" -ForegroundColor Green
        Write-Host "Windows should now recognize the partition!" -ForegroundColor Cyan
    } else {
        Write-Host "Disk signature already present: 0x$($currentSig.ToString('X8'))" -ForegroundColor Green
    }
} else {
    Write-Host "Invalid or no MBR found!" -ForegroundColor Red
}

$stream.Close()

Write-Host ""
Write-Host "Now run 'Rescan Disks' in Disk Management or reboot"