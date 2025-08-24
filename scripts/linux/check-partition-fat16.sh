#!/bin/bash
# Check FAT16 filesystem inside a partition

if [ $# -ne 1 ]; then
    echo "Usage: $0 <device>"
    echo "Example: $0 /dev/sdb"
    exit 1
fi

DEVICE=$1

echo "Checking device: $DEVICE"
echo "================================"
echo ""

# Check MBR
echo "MBR Analysis:"
dd if="$DEVICE" bs=512 count=1 2>/dev/null | od -A x -t x1 | grep "1b0\|1c0\|1d0\|1e0"
echo ""

# Check partition entry at offset 446
echo "Partition Table Entry 1:"
PARTITION_TYPE=$(dd if="$DEVICE" bs=1 skip=450 count=1 2>/dev/null | od -A n -t x1)
echo "  Type: $PARTITION_TYPE (06 = FAT16, 0b/0c = FAT32, 07 = NTFS)"

START_LBA=$(dd if="$DEVICE" bs=1 skip=454 count=4 2>/dev/null | od -A n -t u4 -j 0 --endian=little | tr -d ' ')
echo "  Start LBA: $START_LBA"
echo "  Start Offset: $((START_LBA * 512)) bytes"
echo ""

# Check FAT16 at partition offset (usually sector 2048 = 1MB)
if [ "$START_LBA" -gt 0 ]; then
    OFFSET=$((START_LBA * 512))
    echo "Checking FAT16 at partition offset $OFFSET:"
    echo ""
    
    # Read boot sector from partition
    echo "FAT16 Boot Sector at partition:"
    dd if="$DEVICE" bs=512 skip=$START_LBA count=1 2>/dev/null | od -A x -t x1 -N 64
    echo ""
    
    # Check signature
    SIGNATURE=$(dd if="$DEVICE" bs=1 skip=$((OFFSET + 510)) count=2 2>/dev/null | od -A n -t x1)
    echo "  Boot Signature: $SIGNATURE (should be 55 aa)"
    
    # Check OEM
    OEM=$(dd if="$DEVICE" bs=1 skip=$((OFFSET + 3)) count=8 2>/dev/null)
    echo "  OEM Name: '$OEM'"
    
    # Check FS Type
    FS_TYPE=$(dd if="$DEVICE" bs=1 skip=$((OFFSET + 54)) count=8 2>/dev/null)
    echo "  FS Type: '$FS_TYPE'"
    
    # Check label
    LABEL=$(dd if="$DEVICE" bs=1 skip=$((OFFSET + 43)) count=11 2>/dev/null)
    echo "  Volume Label: '$LABEL'"
    
    echo ""
    echo "FAT16 Parameters at partition offset:"
    BYTES_PER_SECTOR=$(dd if="$DEVICE" bs=1 skip=$((OFFSET + 11)) count=2 2>/dev/null | od -A n -t u2 --endian=little | tr -d ' ')
    SECTORS_PER_CLUSTER=$(dd if="$DEVICE" bs=1 skip=$((OFFSET + 13)) count=1 2>/dev/null | od -A n -t u1 | tr -d ' ')
    ROOT_ENTRIES=$(dd if="$DEVICE" bs=1 skip=$((OFFSET + 17)) count=2 2>/dev/null | od -A n -t u2 --endian=little | tr -d ' ')
    
    echo "  Bytes per Sector: $BYTES_PER_SECTOR"
    echo "  Sectors per Cluster: $SECTORS_PER_CLUSTER"
    echo "  Root Entries: $ROOT_ENTRIES"
    
    # Check FAT entries
    RESERVED_SECTORS=$(dd if="$DEVICE" bs=1 skip=$((OFFSET + 14)) count=2 2>/dev/null | od -A n -t u2 --endian=little | tr -d ' ')
    FAT_OFFSET=$((OFFSET + RESERVED_SECTORS * BYTES_PER_SECTOR))
    
    echo ""
    echo "FAT Table at offset $FAT_OFFSET:"
    FAT0=$(dd if="$DEVICE" bs=1 skip=$FAT_OFFSET count=2 2>/dev/null | od -A n -t x1)
    FAT1=$(dd if="$DEVICE" bs=1 skip=$((FAT_OFFSET + 2)) count=2 2>/dev/null | od -A n -t x1)
    echo "  FAT[0]: $FAT0 (should be f8 ff)"
    echo "  FAT[1]: $FAT1 (should be ff ff)"
else
    echo "No partition found or invalid partition table"
fi

echo ""
echo "Summary:"
echo "--------"
if [ "$PARTITION_TYPE" == " 06" ]; then
    echo "✓ Partition type is FAT16 (0x06)"
else
    echo "✗ Partition type is not FAT16"
fi

if [ "$SIGNATURE" == " 55 aa" ]; then
    echo "✓ Valid boot signature found at partition"
else
    echo "✗ Invalid boot signature at partition"
fi

if [[ "$FS_TYPE" == *"FAT16"* ]]; then
    echo "✓ FAT16 filesystem type string found"
else
    echo "✗ FAT16 filesystem type string not found"
fi

echo ""
echo "To access this FAT16 partition:"
echo "  - On Linux: mount ${DEVICE}1 /mnt/usb"
echo "  - On Windows: Use Disk Management to assign a drive letter"
echo "  - The partition starts at sector $START_LBA (offset $((START_LBA * 512)) bytes)"