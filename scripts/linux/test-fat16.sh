#!/bin/bash
# Test FAT16 formatting and verification

echo "FAT16 Format Test"
echo "================="
echo ""

# Create a test image
TEST_IMAGE="/tmp/fat16_test.img"
SIZE_MB=64

echo "Creating ${SIZE_MB}MB test image..."
dd if=/dev/zero of="$TEST_IMAGE" bs=1M count=$SIZE_MB 2>/dev/null

echo "Testing FAT16 format..."
echo ""

# Show before state
echo "Before formatting:"
file "$TEST_IMAGE"
echo ""

# Run the check script
echo "Checking initial state:"
./check-fat16.sh "$TEST_IMAGE" | grep -E "(Boot Signature|FS Type|FAT16)"
echo ""

# Try to format with mkfs.fat for comparison
echo "Formatting with mkfs.fat (for comparison)..."
mkfs.fat -F 16 -n "REFERENCE" "$TEST_IMAGE" 2>&1 | head -5

echo ""
echo "After mkfs.fat formatting:"
./check-fat16.sh "$TEST_IMAGE" | grep -E "(Boot Signature|OEM|FS Type|Volume Label|FAT16 Parameters|FAT\[)"
echo ""

# Show what Windows/Linux sees
echo "System identification:"
file -s "$TEST_IMAGE"

# Check with fsck
if command -v fsck.fat &> /dev/null; then
    echo ""
    echo "fsck.fat check:"
    fsck.fat -n "$TEST_IMAGE" 2>&1 | head -10
fi

# Clean up
rm -f "$TEST_IMAGE"

echo ""
echo "Test complete. Key points to verify in Moses FAT16 implementation:"
echo "1. Boot signature must be 55 AA at offset 510-511"
echo "2. OEM name should be 8 bytes (e.g., 'MSWIN4.1' or 'MSDOS5.0')"
echo "3. FS Type at offset 54 should be 'FAT16   ' (8 bytes, padded)"
echo "4. FAT[0] should be F8 FF (for hard disk)"
echo "5. FAT[1] should be FF FF (end of chain marker)"
echo "6. Bytes per sector: 512"
echo "7. Reserved sectors: 1"
echo "8. Number of FATs: 2"
echo "9. Root entries: 512 (standard)"
echo ""
echo "Common issues that prevent recognition:"
echo "- Wrong endianness (FAT16 uses little-endian)"
echo "- Missing or incorrect boot signature"
echo "- Invalid FAT[0] and FAT[1] values"
echo "- Incorrect filesystem type string"
echo "- Misaligned structures"