# PowerShell script to demonstrate how Windows treats drives differently

Write-Host "Windows Drive Type Analysis" -ForegroundColor Cyan
Write-Host "===========================" 
Write-Host ""

# Get all disks
$disks = Get-Disk

foreach ($disk in $disks) {
    Write-Host "Disk $($disk.Number): $($disk.FriendlyName)" -ForegroundColor Yellow
    Write-Host "  Size: $([math]::Round($disk.Size / 1GB, 2)) GB"
    Write-Host "  Bus Type: $($disk.BusType)"
    Write-Host "  Media Type: $($disk.MediaType)"
    Write-Host "  Partition Style: $($disk.PartitionStyle)"
    
    # Check if removable
    $isRemovable = $disk.BusType -eq "USB" -or $disk.MediaType -eq "Removable Media"
    Write-Host "  Removable: $isRemovable" -ForegroundColor $(if ($isRemovable) { "Green" } else { "Gray" })
    
    # Get partitions
    $partitions = Get-Partition -DiskNumber $disk.Number -ErrorAction SilentlyContinue
    
    if ($partitions) {
        Write-Host "  Partitions: $($partitions.Count)"
        foreach ($part in $partitions) {
            if ($part.DriveLetter) {
                $vol = Get-Volume -DriveLetter $part.DriveLetter -ErrorAction SilentlyContinue
                if ($vol) {
                    Write-Host "    [$($part.DriveLetter):] $($vol.FileSystem) - $($vol.FileSystemLabel)"
                }
            }
        }
    } else {
        Write-Host "  Partitions: None or raw filesystem" -ForegroundColor Magenta
    }
    
    # Check removal policy
    $diskDrive = Get-WmiObject Win32_DiskDrive | Where-Object { $_.Index -eq $disk.Number }
    if ($diskDrive) {
        $pnpDevice = Get-PnpDeviceProperty -InstanceId $diskDrive.PNPDeviceID -KeyName "DEVPKEY_Device_RemovalPolicy" -ErrorAction SilentlyContinue
        if ($pnpDevice) {
            $removalPolicy = switch ($pnpDevice.Data) {
                2 { "Quick Removal (ExpectedRemoval)" }
                3 { "Better Performance (SurpriseRemoval)" }
                default { "Unknown ($($pnpDevice.Data))" }
            }
            Write-Host "  Removal Policy: $removalPolicy" -ForegroundColor Cyan
        }
    }
    
    Write-Host ""
}

Write-Host "Key Differences:" -ForegroundColor Yellow
Write-Host "================"
Write-Host ""
Write-Host "REMOVABLE DRIVES (USB/SD):" -ForegroundColor Green
Write-Host "  • Often formatted without partition table (superfloppy)"
Write-Host "  • Windows mounts the whole device if no partition table"
Write-Host "  • Even with MBR, usually only first partition is accessible"
Write-Host "  • Quick removal policy (no write caching)"
Write-Host "  • Examples: USB flash drives, SD cards"
Write-Host ""
Write-Host "FIXED DRIVES:" -ForegroundColor Blue
Write-Host "  • Always use partition table (MBR or GPT)"
Write-Host "  • Each partition gets a separate drive letter"
Write-Host "  • Multiple partitions fully supported"
Write-Host "  • Better performance policy (write caching enabled)"
Write-Host "  • Examples: Internal HDDs, SSDs, external HDDs"
Write-Host ""
Write-Host "SPECIAL CASE - External HDDs:" -ForegroundColor Magenta
Write-Host "  • Connected via USB but treated more like fixed drives"
Write-Host "  • Usually have partition tables"
Write-Host "  • Can have multiple accessible partitions"
Write-Host "  • Size often determines behavior (large = fixed-like)"