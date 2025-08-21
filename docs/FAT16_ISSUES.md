# FAT16 Implementation Issues Analysis

## Current Problems
1. Windows cannot recognize our FAT16 formatted drives
2. Moses itself sometimes fails to detect FAT16

## Potential Issues in Our Implementation

### 1. **Packed Struct Alignment**
```rust
#[repr(C, packed(1))]
struct Fat16BootSector { ... }
```
- Using `packed(1)` might cause issues with byte alignment
- When we write this struct directly to disk, padding bytes might be incorrect

### 2. **Boot Sector Issues**

#### Jump Instruction
- Current: `[0xEB, 0x3C, 0x90]`
- This is correct for FAT16

#### OEM Name
- Current: `"MSWIN4.1"`
- This should be fine (Windows 95 compatible)

#### Critical Fields That Might Be Wrong:
1. **Hidden Sectors**: We use `2048` when partition table exists, `0` otherwise
   - This MUST match the actual partition offset
   - If MBR says partition starts at sector 2048, hidden_sectors MUST be 2048

2. **Drive Number**: We use `0x80` (hard disk)
   - For removable media (USB), this should be `0x00`
   - Wrong drive number can cause Windows to reject the filesystem

3. **Media Descriptor**: We use `0xF8` (fixed disk)
   - For removable media, should be `0xF0`
   - Must match the first byte of FAT[0]

### 3. **FAT Table Initialization**
```rust
fat[0] = 0xF8; // Media descriptor
fat[1] = 0xFF;
fat[2] = 0xFF; // End of chain
fat[3] = 0xFF;
```
This looks correct, but we should verify the media descriptor matches the boot sector.

### 4. **Windows-Specific Requirements**

Windows has additional requirements beyond the FAT specification:

1. **Cluster Size**: Must follow Microsoft's recommendations
   - We seem to follow this correctly

2. **Volume Serial Number**: Should be unique (based on current time)
   - We use fixed `0x12345678` - this should be randomized

3. **Filesystem Type String**: Must be exactly "FAT16   " (with spaces)
   - We have this correct

### 5. **Partition Table Issues**

When we create an MBR:
- We correctly add disk signature (fixed in previous session)
- Partition type is `0x06` for FAT16 (correct)
- BUT: The hidden_sectors field in the boot sector MUST match the partition offset

## Recommended Fixes

### Fix 1: Correct Drive Number for Removable Media
```rust
drive_number: if device.is_removable { 0x00 } else { 0x80 },
```

### Fix 2: Correct Media Descriptor
```rust
media_descriptor: if device.is_removable { 0xF0 } else { 0xF8 },
```

### Fix 3: Generate Unique Volume ID
```rust
use std::time::SystemTime;
let volume_id = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs() as u32;
```

### Fix 4: Verify Hidden Sectors Match Partition Offset
When using partition table, ensure hidden_sectors in boot sector matches the actual partition start.

### Fix 5: Write Boot Sector Correctly
Instead of using unsafe pointer cast, properly serialize the struct:
```rust
let mut boot_sector_bytes = vec![0u8; 512];
// Manually copy each field at the correct offset
// This ensures no alignment/padding issues
```

## Testing Strategy

1. Format a USB drive with our FAT16 formatter
2. Run the spec_compliance_test to check all fields
3. Compare with a Windows-formatted FAT16 drive
4. Use a hex editor to inspect the boot sector directly

## References
- Microsoft FAT Specification: https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/fatgen103.doc
- FAT16 Boot Sector: https://wiki.osdev.org/FAT#BPB_.28BIOS_Parameter_Block.29