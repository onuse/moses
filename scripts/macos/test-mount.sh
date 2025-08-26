#!/bin/bash

# Test script for Moses filesystem mounting on macOS
# Prerequisites: macFUSE must be installed

set -e

# Colors for output (macOS compatible)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Default values
SOURCE_DEVICE="/dev/disk2s1"  # Source device with filesystem
MOUNT_POINT="/Volumes/moses"   # Where to mount it
BUILD_FIRST=0                  # Build before testing
FS_TYPE=""                     # Auto-detect by default

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -s|--source)
            SOURCE_DEVICE="$2"
            shift 2
            ;;
        -m|--mount)
            MOUNT_POINT="$2"
            shift 2
            ;;
        -t|--type)
            FS_TYPE="$2"
            shift 2
            ;;
        -b|--build)
            BUILD_FIRST=1
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  -s, --source DEVICE   Source device (default: /dev/disk2s1)"
            echo "  -m, --mount PATH      Mount point (default: /Volumes/moses)"
            echo "  -t, --type TYPE       Filesystem type (auto-detect if not specified)"
            echo "  -b, --build           Build Moses before testing"
            echo "  -h, --help            Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo -e "${CYAN}===============================================${NC}"
echo -e "${CYAN}     Moses Bridge - macOS Mount Test${NC}"
echo -e "${CYAN}===============================================${NC}"
echo ""

# Check if macFUSE is installed
if [[ ! -d "/Library/Filesystems/macfuse.fs" ]] && [[ ! -d "/Library/Filesystems/osxfuse.fs" ]]; then
    echo -e "${RED}❌ macFUSE not found!${NC}"
    echo -e "${YELLOW}   Please install macFUSE from:${NC}"
    echo -e "${YELLOW}   https://osxfuse.github.io/${NC}"
    echo -e "${YELLOW}   Or via Homebrew:${NC}"
    echo -e "${YELLOW}   brew install --cask macfuse${NC}"
    echo ""
    echo -e "${YELLOW}   Note: You may need to allow the kernel extension in:${NC}"
    echo -e "${YELLOW}   System Preferences > Security & Privacy${NC}"
    exit 1
fi
echo -e "${GREEN}✅ macFUSE is installed${NC}"

# Build if requested
if [[ $BUILD_FIRST -eq 1 ]]; then
    echo ""
    echo -e "${YELLOW}Building Moses with FUSE support...${NC}"
    
    # Navigate to project root
    cd "$(dirname "$0")/../.."
    
    # Build with Unix mount feature
    cargo build --package moses-cli --features mount-unix --release
    if [[ $? -ne 0 ]]; then
        echo -e "${RED}❌ Build failed!${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Build successful!${NC}"
fi

# Find Moses executable
MOSES_PATH=""
if [[ -f "../../target/release/moses" ]]; then
    MOSES_PATH="../../target/release/moses"
elif [[ -f "../../target/debug/moses" ]]; then
    MOSES_PATH="../../target/debug/moses"
elif command -v moses &> /dev/null; then
    MOSES_PATH="moses"
else
    echo -e "${RED}❌ Moses CLI not found!${NC}"
    echo -e "${YELLOW}   Run with -b flag or build manually:${NC}"
    echo -e "${YELLOW}   cargo build --package moses-cli --features mount-unix${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Moses CLI found at: $MOSES_PATH${NC}"

# List available devices
echo ""
echo -e "${CYAN}Available devices:${NC}"
echo -e "${YELLOW}Note: Use 'diskutil list' to see all disks${NC}"
sudo $MOSES_PATH list

# Check if source device exists
if [[ ! -b "$SOURCE_DEVICE" ]] && [[ ! -c "$SOURCE_DEVICE" ]]; then
    echo -e "${RED}❌ Source device $SOURCE_DEVICE not found!${NC}"
    echo -e "${YELLOW}   Please specify a valid device.${NC}"
    echo -e "${YELLOW}   Use 'diskutil list' to find available devices.${NC}"
    exit 1
fi

# Create mount point if it doesn't exist
if [[ ! -d "$MOUNT_POINT" ]]; then
    echo -e "${YELLOW}Creating mount point: $MOUNT_POINT${NC}"
    sudo mkdir -p "$MOUNT_POINT"
fi

# Test the mount command
echo ""
echo -e "${CYAN}Testing mount command...${NC}"
if [[ -n "$FS_TYPE" ]]; then
    echo -e "Command: sudo moses mount $SOURCE_DEVICE $MOUNT_POINT --fs-type $FS_TYPE --readonly"
    sudo $MOSES_PATH mount "$SOURCE_DEVICE" "$MOUNT_POINT" --fs-type "$FS_TYPE" --readonly
else
    echo -e "Command: sudo moses mount $SOURCE_DEVICE $MOUNT_POINT --readonly"
    sudo $MOSES_PATH mount "$SOURCE_DEVICE" "$MOUNT_POINT" --readonly
fi

if [[ $? -eq 0 ]]; then
    echo ""
    echo -e "${GREEN}✅ Mount command executed successfully!${NC}"
    
    # Check if mount point is accessible (macOS specific check)
    if mount | grep -q "$MOUNT_POINT"; then
        echo -e "${GREEN}✅ Filesystem is mounted at $MOUNT_POINT${NC}"
        echo ""
        
        # Show some statistics
        echo -e "${CYAN}Mount information:${NC}"
        df -h "$MOUNT_POINT"
        echo ""
        
        # List root directory
        echo -e "${CYAN}Root directory contents:${NC}"
        ls -la "$MOUNT_POINT" | head -10
        echo ""
        
        echo -e "${GREEN}You can now:${NC}"
        echo -e "  1. Open in Finder: open $MOUNT_POINT"
        echo -e "  2. Browse files: cd $MOUNT_POINT"
        echo -e "  3. Copy files: cp $MOUNT_POINT/file.txt ~/Desktop/"
        echo -e "  4. Use any macOS application to access the files"
        echo ""
        echo -e "${YELLOW}To unmount, run:${NC}"
        echo -e "  sudo moses unmount $MOUNT_POINT"
        echo -e "${YELLOW}Or:${NC}"
        echo -e "  sudo umount $MOUNT_POINT"
        echo -e "${YELLOW}Or use Finder:${NC}"
        echo -e "  Click the eject button next to the volume"
    else
        echo -e "${YELLOW}⚠️  Mount point not accessible yet.${NC}"
        echo -e "${YELLOW}   The filesystem may still be initializing.${NC}"
    fi
else
    echo ""
    echo -e "${RED}❌ Mount command failed!${NC}"
    echo ""
    echo -e "${YELLOW}Troubleshooting:${NC}"
    echo -e "  1. Make sure $SOURCE_DEVICE contains a supported filesystem"
    echo -e "  2. Run this script with sudo"
    echo -e "  3. Check system logs: log show --last 5m | grep -i fuse"
    echo -e "  4. Ensure $MOUNT_POINT is not already in use"
    echo -e "  5. Try specifying filesystem type with -t option"
    echo -e "  6. Check Security & Privacy settings for kernel extension approval"
fi

echo ""
echo -e "${CYAN}===============================================${NC}"