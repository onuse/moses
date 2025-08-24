# Fixing FAT16 Recognition Issues

## Current State
Your drive has a valid FAT16 filesystem but Windows doesn't recognize it.

## Issues Found:
1. **No partition table** - Formatted as "superfloppy"
2. **Large cluster size** - 64KB clusters (128 sectors × 512 bytes)
3. **Possible volume label issue** - "hsext" might be corrupted

## Solutions to Try:

### Option 1: Add MBR Partition Table (Recommended)
Windows 11 strongly prefers partitioned media for drives > 512MB.

```bash
# Using diskpart (Windows):
diskpart
select disk 2  # Be VERY careful - verify this is correct!
clean           # WARNING: Erases everything!
create partition primary
format fs=fat quick
```

### Option 2: Fix with Our Tools
```bash
# Run our MBR partitioner to add partition table while preserving FAT16
cargo run --bin moses-cli -- partition create-mbr \\.\PHYSICALDRIVE2 --preserve-filesystem
```

### Option 3: Manual Fix (Advanced)
Create an MBR that points to the existing FAT16:

1. **Backup first 512 bytes** (boot sector)
2. **Write MBR** with partition entry pointing to sector 0
3. **Update hidden sectors** in FAT16 BPB to match partition offset

## The Real Problem:
Windows expects:
- USB drives < 512MB: Can be superfloppy (no partition)
- USB drives > 512MB: Should have MBR partition table
- Your 3.6GB drive without partition table confuses Windows

## Quick Test:
Try these commands to see if Windows can at least read it:
```cmd
# Check if Windows sees the filesystem
fsutil fsinfo volumeinfo \\.\PHYSICALDRIVE2

# Try to assign a drive letter manually
diskpart
select disk 2
select partition 1  # Will fail if no partition
assign letter=Z
```

## Moses Formatter Fix Needed:
Our FAT16 formatter should:
1. **Always create MBR** for drives > 512MB
2. **Set hidden sectors** correctly
3. **Use smaller clusters** (max 32KB recommended)

The filesystem itself is valid - it's the lack of partition table that's the issue!