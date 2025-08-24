# Ext Family Implementation Strategy

## Current ext4 Implementation Assets

### Reusable Components (90%+)
- ✅ Superblock structure (Ext4Superblock)
- ✅ Group descriptors (Ext4GroupDesc)  
- ✅ Inode structure (Ext4Inode)
- ✅ Directory entries (Ext4DirEntry)
- ✅ Bitmap management
- ✅ Device I/O (Windows/Linux)
- ✅ Alignment and buffer management
- ✅ CRC32 checksums

## ext2 Implementation (Simplest)

### What to Remove:
```rust
// In superblock init:
- Remove COMPAT_HAS_JOURNAL
- Remove INCOMPAT_EXTENTS  
- Remove INCOMPAT_64BIT (if >2GB)
- Remove RO_COMPAT_METADATA_CSUM
- Set s_feature_compat = EXT2_FEATURE_COMPAT_DIR_INDEX only
```

### What to Change:
```rust
// In inode creation:
- Use indirect blocks (i_block[0-14]) instead of extent header
- No checksums needed
- Simpler timestamps (no nanosecond precision)
```

### Estimated Work: 2-3 hours
Just need a new `Ext2Formatter` that calls existing code with different flags.

## ext3 Implementation (Easy)

### What to Add:
```rust
// Journal support:
- Set COMPAT_HAS_JOURNAL flag
- Create journal inode (inode 8)
- Reserve blocks for journal (typically 32MB)
- Simple ordered mode (metadata journaling only)
```

### What to Keep from ext2:
- Indirect blocks (no extents)
- Simpler features
- No checksums

### Estimated Work: 4-6 hours
Main work is creating the journal structure.

## Code Structure Proposal

```rust
// formatters/src/ext_family/
mod core;           // Shared structures (move current ext4_native/core here)
mod ext2;          // Ext2Formatter
mod ext3;          // Ext3Formatter  
mod ext4;          // Ext4Formatter (current implementation)

// Each formatter just sets different parameters:
impl Ext2Formatter {
    fn get_params() -> FsParams {
        FsParams {
            has_journal: false,
            use_extents: false,
            use_64bit: false,
            use_checksums: false,
            ..Default::default()
        }
    }
}
```

## Quick Win Implementation Order

1. **ext2 first** - Simplest, proves the architecture
2. **ext3 next** - Adds only journal to ext2
3. **Optimize shared code** - Extract common components

## Bonus: Other Filesystems Made Easier

### ReiserFS v3
- Similar metadata concepts
- B+ tree experience from ext4 helps
- Device I/O layer ready

### JFS
- Similar journal concepts to ext3
- Allocation groups like ext4
- Can reuse bitmap management

### Minix FS
- Even simpler than ext2
- Good for embedded/educational use
- ~1 day of work

## Testing Strategy

Since ext2/ext3 are simpler than ext4, our existing ext4 tests actually over-test them:
- If ext4 structures work, ext2/ext3 definitely work
- Linux kernel can mount and verify all three
- Same verification tools work