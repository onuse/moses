#!/bin/sh
# Script to check actual superblock values on disk
# Run on OpenWrt after formatting

DEVICE="${1:-/dev/sda}"

echo "=== Checking EXT4 Superblock on $DEVICE ==="
echo

# Read key superblock fields (all at offset 1024 from start)
echo "1. Total blocks count (64-bit at offset 1024+4):"
echo -n "  Low 32 bits (offset 1028): "
dd if=$DEVICE bs=1 skip=1028 count=4 2>/dev/null | od -An -tx4 -v
echo -n "  High 32 bits (offset 1360): "
dd if=$DEVICE bs=1 skip=1360 count=4 2>/dev/null | od -An -tx4 -v

echo
echo "2. Free blocks count (64-bit):"
echo -n "  Low 32 bits (offset 1036): "
dd if=$DEVICE bs=1 skip=1036 count=4 2>/dev/null | od -An -tx4 -v
echo -n "  High 32 bits (offset 1368): "
dd if=$DEVICE bs=1 skip=1368 count=4 2>/dev/null | od -An -tx4 -v

echo
echo "3. Block size (at offset 1048):"
dd if=$DEVICE bs=1 skip=1048 count=4 2>/dev/null | od -An -td4 -v

echo
echo "4. First group descriptor (at block 1 = offset 4096):"
echo "  Block bitmap low (offset 4096+0):"
dd if=$DEVICE bs=1 skip=4096 count=4 2>/dev/null | od -An -tx4 -v
echo "  Free blocks count (16+16 bit at offset 4096+12):"
echo -n "    Low 16 bits: "
dd if=$DEVICE bs=1 skip=4108 count=2 2>/dev/null | od -An -tx2 -v
echo -n "    High 16 bits: "
dd if=$DEVICE bs=1 skip=4140 count=2 2>/dev/null | od -An -tx2 -v

echo
echo "5. Calculated values:"
# Use awk for calculations since OpenWrt sh might not have advanced arithmetic
TOTAL_LO=$(dd if=$DEVICE bs=1 skip=1028 count=4 2>/dev/null | od -An -tx4 -v | tr -d ' ')
TOTAL_HI=$(dd if=$DEVICE bs=1 skip=1360 count=4 2>/dev/null | od -An -tx4 -v | tr -d ' ')
FREE_LO=$(dd if=$DEVICE bs=1 skip=1036 count=4 2>/dev/null | od -An -tx4 -v | tr -d ' ')
FREE_HI=$(dd if=$DEVICE bs=1 skip=1368 count=4 2>/dev/null | od -An -tx4 -v | tr -d ' ')

echo "  Total blocks: low=0x$TOTAL_LO high=0x$TOTAL_HI"
echo "  Free blocks:  low=0x$FREE_LO high=0x$FREE_HI"

# Check if free > total (the bug condition)
echo
echo "6. Checking for overflow condition:"
if [ "$FREE_HI" != "00000000" ]; then
    echo "  WARNING: Free blocks high bits are non-zero: 0x$FREE_HI"
    echo "  This indicates free blocks > 4GB * 4K = 16TB!"
fi

# Python one-liner to calculate actual values if python is available
if command -v python3 >/dev/null 2>&1; then
    echo
    echo "7. Python calculation:"
    python3 -c "
total_lo = 0x$TOTAL_LO if '$TOTAL_LO' else 0
total_hi = 0x$TOTAL_HI if '$TOTAL_HI' else 0
free_lo = 0x$FREE_LO if '$FREE_LO' else 0
free_hi = 0x$FREE_HI if '$FREE_HI' else 0
total = total_lo + (total_hi << 32)
free = free_lo + (free_hi << 32)
print(f'  Total blocks: {total} ({total * 4096 / 1e9:.1f} GB)')
print(f'  Free blocks: {free} ({free * 4096 / 1e9:.1f} GB)')
if free > total:
    print(f'  ERROR: Free blocks exceeds total by {free - total}!')
    print(f'  This causes df to show {(2**64 - (free - total)) * 4096 / 1e18:.1f} EB used!')
"
fi