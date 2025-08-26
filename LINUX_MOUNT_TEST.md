# Testing NTFS Mount on Linux Mint

## Quick Start

### 1. Find your NTFS device or create a test image

**Option A: Use an existing NTFS device**
```bash
# List available devices
lsblk -f | grep ntfs

# Or check with Moses
./target/release/moses list
```

**Option B: Create a test NTFS image**
```bash
# Create a 100MB test image
dd if=/dev/zero of=test_ntfs.img bs=1M count=100

# Format it as NTFS (requires ntfs-3g)
mkfs.ntfs -F -L "TestNTFS" test_ntfs.img
```

### 2. Create a mount point
```bash
mkdir -p ~/moses_mount
```

### 3. Mount the NTFS filesystem

**For read-only mount (safer for testing):**
```bash
./target/release/moses mount /dev/sdX ~/moses_mount --readonly
# Or for an image file:
./target/release/moses mount test_ntfs.img ~/moses_mount --readonly
```

**For read-write mount (experimental):**
```bash
./target/release/moses mount /dev/sdX ~/moses_mount
# Or for an image file:
./target/release/moses mount test_ntfs.img ~/moses_mount
```

### 4. Test the mount
```bash
# Check if mounted
mountpoint ~/moses_mount

# List files
ls -la ~/moses_mount

# For read-write mount, try creating a file
echo "Hello from Moses!" > ~/moses_mount/test.txt
cat ~/moses_mount/test.txt
```

### 5. Unmount when done
```bash
fusermount -u ~/moses_mount
```

## Expected Behavior

### What Works:
- ✅ **Read-only mounting** - Should work reliably
- ✅ **Listing files and directories**
- ✅ **Reading file contents**
- ✅ **Navigating directory structure**

### What's Experimental:
- ⚠️ **Writing to existing files** - Non-resident data only
- ⚠️ **Creating new empty files**
- ⚠️ **Deleting files**

### Known Limitations:
- ❌ Directory index updates not implemented (new files won't appear in listings)
- ❌ Resident data modification not implemented
- ❌ Converting resident to non-resident data not implemented

## Troubleshooting

### Permission Denied
```bash
# Run with sudo if needed for device access
sudo ./target/release/moses mount /dev/sdX ~/moses_mount
```

### Mount point busy
```bash
# Force unmount
fusermount -uz ~/moses_mount
# Or
sudo umount ~/moses_mount
```

### Debug mode
```bash
# Run with debug logging
RUST_LOG=debug ./target/release/moses mount test_ntfs.img ~/moses_mount
```

## Safety Notes

1. **Always test on non-critical data first**
2. **Use read-only mode for important drives**
3. **Keep backups before testing write operations**
4. **The write support is experimental and may have bugs**

## Using the Test Script

We also have an automated test script:
```bash
cd test_scripts
./test_ntfs_mount.sh

# Or with your own NTFS image:
./test_ntfs_mount.sh /path/to/your/ntfs.img
```

This script will:
1. Create a test NTFS image (if needed)
2. Test read-only mount
3. Test read-write mount
4. Verify file operations
5. Clean up automatically