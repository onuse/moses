#!/bin/bash

# Test script for EXT4 formatting with a loop device
# This creates a virtual disk file for safe testing

set -e

echo "Creating test disk image..."
TEST_IMG="/tmp/moses_test.img"
TEST_SIZE="100M"

# Create a 100MB test image
dd if=/dev/zero of=$TEST_IMG bs=1M count=100 2>/dev/null

echo "Setting up loop device..."
# Create a loop device (requires sudo)
LOOP_DEVICE=$(sudo losetup -f --show $TEST_IMG)
echo "Loop device created: $LOOP_DEVICE"

echo "Testing Moses list command..."
./target/debug/moses list | grep -A5 "$LOOP_DEVICE" || echo "Device not found in list"

echo ""
echo "To test formatting, you can run:"
echo "  sudo ./target/debug/moses format $LOOP_DEVICE -f ext4"
echo ""
echo "To clean up after testing, run:"
echo "  sudo losetup -d $LOOP_DEVICE"
echo "  rm $TEST_IMG"