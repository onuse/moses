# Critical FAT16 Writing Bugs Found

## BUG #1: Wrong Device Path for Windows
```rust
format!("\\\\.\\{}", mount_str.trim_end_matches('\\'))
```
**This is WRONG!** For drive letters it should be `\\.\X:` but we're stripping the backslash!
- We get: `\\.\E`
- Should be: `\\.\E:`

## BUG #2: FAT[0] Entry is WRONG
```rust
fat[0] = media_descriptor;  // WRONG!
fat[1] = 0xFF;
```
**FAT16 entries are 16-bit!** We're writing bytes when we should write words:
```rust
// CORRECT for FAT16:
let fat0_value = 0xFF00 | media_descriptor as u16;
fat[0..2].copy_from_slice(&fat0_value.to_le_bytes());
```

## BUG #3: Wrong Total Sectors Calculation
When creating partition table, we calculate:
```rust
let size = device.size - offset;  // This is partition size
let total_sectors = partition_size / 512;
```
But the partition table itself might use different size! We should read back the actual partition size from MBR.

## BUG #4: Media Descriptor Mismatch
Boot sector media descriptor MUST match FAT[0] low byte:
- Boot sector: `0xF0` for removable
- FAT[0]: Should be `0xF0FF` for removable (we write `0xF0` `0xFF` as separate bytes)

## BUG #5: CHS Geometry Not From Device
We hardcode:
```rust
boot_sector[0x18..0x1A].copy_from_slice(&63u16.to_le_bytes());   // sectors per track
boot_sector[0x1A..0x1C].copy_from_slice(&255u16.to_le_bytes());  // heads
```
But Windows expects the ACTUAL device geometry! For USB drives this is often different.

## BUG #6: No Volume Label in Root Directory
We write empty root directory:
```rust
let root_dir = vec![0u8; root_dir_sectors as usize * 512];
```
But Windows expects at least a volume label entry if boot sector has one!

## BUG #7: Incorrect Cluster Count Validation
Our calculation in `calculate_fat16_params`:
```rust
let data_start_estimate = reserved_sectors + root_dir_sectors;
let usable_sectors = total_sectors.saturating_sub(data_start_estimate as u64);
```
**This is WRONG!** We forgot to subtract FAT sectors:
```rust
// CORRECT:
let fat_sectors = num_fats * sectors_per_fat;
let data_start = reserved_sectors + fat_sectors + root_dir_sectors;
```

## BUG #8: Windows Device Path Opening
For Windows raw device access:
- Physical drives: `\\.\PhysicalDriveN`
- Logical drives: `\\.\X:` (with colon!)
- Volumes: `\\.\Volume{GUID}`

We're mixing these up!

## The Smoking Gun

The most critical bug is **BUG #2** - we're writing FAT entries as 8-bit values when FAT16 uses 16-bit entries! This completely corrupts the FAT table and makes it unreadable.

## Quick Fix Priority:
1. **Fix FAT[0] and FAT[1] to be 16-bit values**
2. **Fix device path to include colon for drive letters**
3. **Fix cluster count calculation**
4. **Add volume label entry to root directory**

## Test to Confirm:
```powershell
# After our format:
# Dump FAT table
fsutil file createnew test.img 10485760
# Format with our tool
moses format test.img fat16
# Check bytes at offset 512 (first FAT)
# Should see: F0 FF FF FF (for removable)
# We probably see: F0 FF FF FF (looks same but wrong reason!)
```

The FAT entry bug alone would make Windows unable to read the filesystem!