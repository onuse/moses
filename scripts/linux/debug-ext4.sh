#!/bin/sh
# Debug script to examine ext4 filesystem structure
# Run on OpenWrt or any Linux system with the formatted USB drive

DEVICE="${1:-/dev/sda}"

echo "=== EXT4 Filesystem Debug for $DEVICE ==="
echo

echo "1. Raw Superblock (at offset 1024):"
hexdump -C $DEVICE -s 1024 -n 256 | head -20

echo
echo "2. Checking magic number:"
MAGIC=$(dd if=$DEVICE bs=1 skip=1080 count=2 2>/dev/null | hexdump -e '"%04x"')
echo "Magic: 0x$MAGIC (should be ef53)"

echo
echo "3. Superblock key fields:"
dd if=$DEVICE bs=1024 skip=1 count=1 2>/dev/null | od -x | head -20

echo
echo "4. Group Descriptor Table (block 1):"
hexdump -C $DEVICE -s 4096 -n 512 | head -20

echo
echo "5. Attempting to read with dumpe2fs (if available):"
if command -v dumpe2fs >/dev/null 2>&1; then
    dumpe2fs -h $DEVICE 2>&1 | head -40
else
    echo "dumpe2fs not available"
fi

echo
echo "6. Attempting to read with tune2fs (if available):"
if command -v tune2fs >/dev/null 2>&1; then
    tune2fs -l $DEVICE 2>&1 | head -40
else
    echo "tune2fs not available"
fi

echo
echo "7. Kernel mount attempt and dmesg:"
mkdir -p /tmp/ext4_test
mount -t ext4 $DEVICE /tmp/ext4_test 2>&1
if [ $? -eq 0 ]; then
    echo "Mount successful!"
    ls -la /tmp/ext4_test/
    umount /tmp/ext4_test
else
    echo "Mount failed. Checking dmesg:"
    dmesg | tail -10
fi

echo
echo "8. File system type detection:"
blkid $DEVICE 2>&1

echo
echo "=== End Debug ==="