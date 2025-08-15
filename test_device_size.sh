#!/bin/bash
# Test script to verify device size detection on Windows

echo "Testing device size detection..."
echo ""
echo "This test will attempt to format a test device with ext4."
echo "The debug output should show the detected device size."
echo ""
echo "Look for lines like:"
echo "  DEBUG: Device size detected: XXXXXX bytes (XX.XX GB)"
echo ""
echo "If you see a non-zero device size, the fix is working!"
echo ""
echo "Note: This test requires running as Administrator on Windows"