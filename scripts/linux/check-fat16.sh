#!/bin/bash
# FAT16 Quick Check - Test if Windows recognizes Moses-formatted drive

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <drive_letter>"
    echo "Example: $0 E:"
    exit 1
fi

DRIVE=$1
echo "Checking FAT16 drive: $DRIVE"
echo "================================"

# 1. Check if Windows sees it
echo "Windows Volume Info:"
wmic volume where "DriveLetter='$DRIVE'" get FileSystem,Label,Capacity

# 2. Try to create a test file
echo ""
echo "Write Test:"
echo "test" > "${DRIVE}\\moses_test.txt" 2>/dev/null && echo "✅ Write successful" || echo "❌ Write failed"

# 3. List directory
echo ""
echo "Directory Listing:"
dir "${DRIVE}\\" 2>/dev/null | head -5

echo ""
echo "================================"
echo "If all checks pass, FAT16 is working!"
