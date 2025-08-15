# EXT4 Native Windows Implementation Plan

## Executive Summary

This document outlines a comprehensive plan to implement a complete, native ext4 filesystem formatter for Windows in pure Rust. The implementation will be built incrementally in phases, with validation at each step to ensure correctness.

## Goals

### Primary Goals
1. **Create valid ext4 filesystems** that pass `e2fsck -fn` validation
2. **Native Windows implementation** with no WSL or external dependencies  
3. **Full compatibility** with Linux kernel 4.x+ for mounting and usage
4. **Performance** - Target <5 seconds for typical USB drive formatting
5. **Safety** - Comprehensive validation and error handling

### Non-Goals (Initial Release)
- Full ext4 feature parity (encryption, compression, etc.)
- Filesystem repair capabilities
- Resize operations
- Mount/read capabilities (format only)

## Architecture Overview

```
┌─────────────────────────────────────────┐
│           User Interface (Tauri)         │
├─────────────────────────────────────────┤
│         Format Manager (Rust)            │
├─────────────────────────────────────────┤
│     EXT4 Native Formatter Module         │
├──────────────┬────────────┬─────────────┤
│  Metadata    │  Allocator │  Validator  │
│  Writer      │            │             │
├──────────────┴────────────┴─────────────┤
│      Windows Raw Device I/O (WinAPI)     │
└─────────────────────────────────────────┘
```

## Implementation Phases

### Phase 0: Planning & Architecture (CURRENT)
**Status:** In Progress  
**Goal:** Complete implementation plan with validation strategy

- [x] Research ext4 specification
- [x] Analyze mkfs.ext4 source code
- [x] Define data structures
- [ ] Complete implementation plan
- [ ] Define validation criteria
- [ ] Set up test infrastructure

### Phase 1: Minimal Valid Filesystem
**Goal:** Create the simplest possible ext4 that passes e2fsck

#### 1.1 Core Structures
- [ ] Superblock with all required fields
- [ ] Single block group (no multi-group support yet)
- [ ] Block group descriptor table
- [ ] Minimal feature flags (no optional features)

#### 1.2 Metadata
- [ ] Block bitmap (mark system blocks as used)
- [ ] Inode bitmap (mark system inodes as used)
- [ ] Inode table with reserved inodes (1-10)
- [ ] Root inode (#2) with proper mode/permissions

#### 1.3 Root Directory
- [ ] Directory data block with "." and ".." entries
- [ ] Proper extent tree in root inode pointing to directory block
- [ ] Correct record lengths and padding

#### 1.4 Checksums
- [ ] CRC32c implementation
- [ ] Superblock checksum
- [ ] Group descriptor checksums
- [ ] Inode checksums (if using metadata_csum)

#### Validation Checkpoint
```bash
# Must pass:
e2fsck -fn test.img
mount -o loop test.img /mnt
ls -la /mnt  # Should show . and ..
umount /mnt
```

### Phase 2: Multi-Group Support
**Goal:** Support larger filesystems with multiple block groups

#### 2.1 Block Group Management
- [ ] Calculate optimal groups for device size
- [ ] Sparse superblock backups (groups 0, 1, powers of 3/5/7)
- [ ] Flex block groups
- [ ] Group descriptor table growth blocks

#### 2.2 Space Allocation
- [ ] Free blocks/inodes calculation per group
- [ ] Proper bitmap initialization for all groups
- [ ] Reserved blocks calculation (5% default)

#### 2.3 Special Inodes
- [ ] Bad blocks inode (#1)
- [ ] Resize inode (#7) 
- [ ] Journal inode (#8) - placeholder
- [ ] First non-reserved inode (#11)

#### Validation Checkpoint
```bash
# Test with various sizes:
for size in 100M 1G 10G; do
    ./moses-native-ext4 test-$size.img --size $size
    e2fsck -fn test-$size.img
done
```

### Phase 3: Complete Directory Structure
**Goal:** Support standard ext4 directory hierarchy

#### 3.1 lost+found Directory
- [ ] Create lost+found inode (#11)
- [ ] Allocate 4 blocks for lost+found (16KB)
- [ ] Add directory entry in root
- [ ] Proper permissions (0700)

#### 3.2 Directory Entry Management
- [ ] Variable-length directory entries
- [ ] Proper alignment and padding
- [ ] Directory entry checksums
- [ ] Hash tree directories (future)

#### 3.3 Extended Attributes
- [ ] EA block allocation
- [ ] System namespace EAs
- [ ] Security labels support

### Phase 4: Journal Implementation
**Goal:** Add journaling support for crash recovery

#### 4.1 Journal Structure
- [ ] JBD2 superblock
- [ ] Journal inode (#8) with extent tree
- [ ] Default journal size calculation (32MB typical)
- [ ] Journal UUID generation

#### 4.2 Journal Initialization
- [ ] Descriptor blocks
- [ ] Commit blocks
- [ ] Revoke tables
- [ ] Initial sequence number

#### 4.3 Feature Flags
- [ ] Enable HAS_JOURNAL feature
- [ ] Enable RECOVER feature initially
- [ ] Journal checksums (JBD2_FEATURE_COMPAT_CHECKSUM)

#### Validation Checkpoint
```bash
# Must show journal:
dumpe2fs test.img | grep -i journal
# Must pass with journal:
e2fsck -fn test.img
```

### Phase 5: Advanced Features
**Goal:** Add commonly-used ext4 features

#### 5.1 Extended Features
- [ ] 64-bit support for >16TB filesystems
- [ ] Huge file support (>2TB files)
- [ ] Metadata checksums
- [ ] Inline data for small files

#### 5.2 Performance Features
- [ ] Uninit block groups
- [ ] Lazy inode table initialization
- [ ] Multiple mount protection (MMP)
- [ ] Extent tree optimization

#### 5.3 Modern Features
- [ ] Project quotas
- [ ] Bigalloc support
- [ ] Inline encryption support (structure only)

## Validation Strategy

### Level 1: Structural Validation
```rust
// Every structure must be validated after creation
fn validate_superblock(sb: &Superblock) -> Result<()> {
    assert_eq!(sb.magic, 0xEF53);
    assert!(sb.block_size >= 1024);
    assert!(sb.blocks_count > 0);
    // ... comprehensive checks
}
```

### Level 2: Binary Comparison
```rust
// Compare with mkfs.ext4 reference output
fn compare_with_reference() {
    let our_fs = create_ext4_native();
    let ref_fs = create_ext4_mkfs();
    
    // Compare critical structures
    assert_eq!(our_fs.superblock, ref_fs.superblock);
    assert_eq!(our_fs.gdt, ref_fs.gdt);
}
```

### Level 3: Linux Validation
```bash
#!/bin/bash
# Automated test script
run_validation_suite() {
    # e2fsck must pass
    e2fsck -fn $1 || exit 1
    
    # Must be mountable
    mount -o loop $1 /mnt || exit 1
    
    # Must support basic operations
    touch /mnt/test || exit 1
    mkdir /mnt/testdir || exit 1
    echo "data" > /mnt/file || exit 1
    
    # Must unmount cleanly
    umount /mnt || exit 1
    
    # Must still be valid
    e2fsck -fn $1 || exit 1
}
```

### Level 4: Compatibility Testing
- Test on Linux kernel 4.19 (Debian 10)
- Test on Linux kernel 5.10 (Debian 11)
- Test on Linux kernel 5.15 (Ubuntu 22.04)
- Test on Linux kernel 6.1 (Debian 12)
- Test on Linux kernel 6.5 (Ubuntu 23.10)

## Data Structure Specifications

### Critical Structures (Must Be Exact)

#### Superblock (Offset 1024, Size 1024)
```rust
struct Ext4Superblock {
    // Offsets must match exactly
    /* 0x00 */ s_inodes_count: u32,      // Total inodes
    /* 0x04 */ s_blocks_count_lo: u32,   // Total blocks (low)
    /* 0x08 */ s_r_blocks_count_lo: u32, // Reserved blocks (low)
    /* 0x0C */ s_free_blocks_lo: u32,    // Free blocks (low)
    /* 0x10 */ s_free_inodes: u32,       // Free inodes
    /* 0x14 */ s_first_data_block: u32,  // First data block
    /* 0x18 */ s_log_block_size: u32,    // Block size = 1024 << this
    /* 0x1C */ s_log_cluster_size: u32,  // Cluster size
    /* 0x20 */ s_blocks_per_group: u32,  // Blocks per group
    /* 0x24 */ s_clusters_per_group: u32,// Clusters per group
    /* 0x28 */ s_inodes_per_group: u32,  // Inodes per group
    /* 0x2C */ s_mtime: u32,              // Mount time
    /* 0x30 */ s_wtime: u32,              // Write time
    /* 0x34 */ s_mnt_count: u16,         // Mount count
    /* 0x36 */ s_max_mnt_count: u16,     // Max mount count
    /* 0x38 */ s_magic: u16,             // 0xEF53
    /* 0x3A */ s_state: u16,             // Clean/errors
    /* 0x3C */ s_errors: u16,            // Error behavior
    /* 0x3E */ s_minor_rev: u16,         // Minor revision
    /* 0x40 */ s_lastcheck: u32,         // Last check time
    /* 0x44 */ s_checkinterval: u32,     // Check interval
    /* 0x48 */ s_creator_os: u32,        // Creator OS
    /* 0x4C */ s_rev_level: u32,         // Revision level
    /* 0x50 */ s_def_resuid: u16,        // Default UID
    /* 0x52 */ s_def_resgid: u16,        // Default GID
    // ... continues to offset 0x400 (1024)
}
```

#### Group Descriptor (Size 32 or 64 bytes)
```rust
struct Ext4GroupDesc {
    /* 0x00 */ bg_block_bitmap_lo: u32,     // Block bitmap block
    /* 0x04 */ bg_inode_bitmap_lo: u32,     // Inode bitmap block  
    /* 0x08 */ bg_inode_table_lo: u32,      // Inode table block
    /* 0x0C */ bg_free_blocks_count_lo: u16,// Free blocks
    /* 0x0E */ bg_free_inodes_count_lo: u16,// Free inodes
    /* 0x10 */ bg_used_dirs_count_lo: u16,  // Used directories
    /* 0x12 */ bg_flags: u16,               // Flags
    /* 0x14 */ bg_exclude_bitmap_lo: u32,   // Exclude bitmap
    /* 0x18 */ bg_block_bitmap_csum_lo: u16,// Block bitmap checksum
    /* 0x1A */ bg_inode_bitmap_csum_lo: u16,// Inode bitmap checksum
    /* 0x1C */ bg_itable_unused_lo: u16,    // Unused inodes
    /* 0x1E */ bg_checksum: u16,            // Group checksum
    // 64-bit fields follow if INCOMPAT_64BIT
}
```

#### Inode (Size 128, 256, or larger)
```rust
struct Ext4Inode {
    /* 0x00 */ i_mode: u16,         // File mode
    /* 0x02 */ i_uid: u16,          // User ID (low)
    /* 0x04 */ i_size_lo: u32,      // Size (low)
    /* 0x08 */ i_atime: u32,        // Access time
    /* 0x0C */ i_ctime: u32,        // Change time
    /* 0x10 */ i_mtime: u32,        // Modify time
    /* 0x14 */ i_dtime: u32,        // Delete time
    /* 0x18 */ i_gid: u16,          // Group ID (low)
    /* 0x1A */ i_links_count: u16,  // Link count
    /* 0x1C */ i_blocks_lo: u32,    // Block count (low)
    /* 0x20 */ i_flags: u32,        // Flags
    /* 0x24 */ i_osd1: u32,         // OS dependent
    /* 0x28 */ i_block: [u32; 15],  // Block pointers/extents
    /* 0x64 */ i_generation: u32,   // Generation
    /* 0x68 */ i_file_acl_lo: u32,  // File ACL (low)
    /* 0x6C */ i_size_high: u32,    // Size (high)
    // ... continues to 128/256 bytes
}
```

## Error Handling Strategy

### Fatal Errors (Abort Immediately)
- Cannot open device for writing
- Device size too small (<1MB)
- Out of memory
- I/O errors during write

### Recoverable Errors (Warning + Continue)
- Non-optimal block group count
- Missing optional features
- Checksum calculation issues (retry)

### Validation Errors (Detailed Report)
- Structure field violations
- Checksum mismatches
- Alignment issues
- Invalid references

## Testing Infrastructure

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    // Test every structure creation
    #[test]
    fn test_superblock_creation() { }
    
    // Test every calculation
    #[test]
    fn test_block_group_calculation() { }
    
    // Test every checksum
    #[test]
    fn test_crc32c_checksum() { }
}
```

### Integration Tests
```rust
#[test]
fn test_minimal_filesystem() {
    let fs = Ext4Filesystem::new(100 * 1024 * 1024); // 100MB
    let image = fs.create();
    
    // Validate internally
    assert!(fs.validate().is_ok());
    
    // Write to file
    fs.write_to_file("test.img")?;
    
    // Validate with e2fsck (if available)
    if let Ok(output) = Command::new("e2fsck").arg("-fn").arg("test.img").output() {
        assert!(output.status.success());
    }
}
```

### Benchmark Tests
```rust
#[bench]
fn bench_format_1gb(b: &mut Bencher) {
    b.iter(|| {
        let fs = Ext4Filesystem::new(1024 * 1024 * 1024);
        fs.create();
    });
}
```

## Performance Targets

| Operation | Target Time | Notes |
|-----------|------------|-------|
| 100MB USB | < 1 second | Minimal metadata |
| 1GB USB | < 2 seconds | Single group |
| 32GB USB | < 5 seconds | Multiple groups |
| 256GB SSD | < 10 seconds | Many groups |
| 1TB HDD | < 30 seconds | Maximum groups |

## Success Criteria

### Minimum Viable Product (MVP)
- [x] Creates structurally valid ext4 filesystem
- [ ] Passes `e2fsck -fn` without errors
- [ ] Mountable on Linux 5.x kernels
- [ ] Supports basic file operations after mount
- [ ] Faster than WSL-based approach

### Production Ready
- [ ] Passes all e2fsck validation levels
- [ ] Supports all common mkfs.ext4 options
- [ ] Comprehensive error messages
- [ ] Full test coverage (>90%)
- [ ] Performance meets targets
- [ ] Documentation complete

## Development Timeline

### Week 1-2: Phase 1 Implementation
- Core structures
- Basic metadata
- Root directory
- CRC32c checksums

### Week 3-4: Phase 2 Implementation  
- Multi-group support
- Space allocation
- Special inodes

### Week 5-6: Validation & Testing
- e2fsck validation
- Linux compatibility testing
- Bug fixes

### Week 7-8: Phase 3 Implementation
- lost+found directory
- Complete directory structure

### Week 9-10: Phase 4 Implementation (Optional)
- Journal support
- Advanced features

### Week 11-12: Polish & Release
- Performance optimization
- Documentation
- Final testing

## Risk Mitigation

### Technical Risks
1. **Checksum Algorithm Complexity**
   - Mitigation: Use well-tested crc32fast crate
   - Fallback: Implement without checksums initially

2. **Windows Raw Device Access**
   - Mitigation: Already proven in PoC
   - Fallback: Require admin privileges

3. **Compatibility Issues**
   - Mitigation: Test against multiple kernel versions
   - Fallback: Target specific kernel version initially

### Schedule Risks
1. **Underestimated Complexity**
   - Mitigation: Phase approach allows partial delivery
   - Fallback: Release without journal support

2. **Validation Issues**
   - Mitigation: Continuous testing during development
   - Fallback: Partner with Linux filesystem expert

## Appendix A: Key Resources

### Specifications
- [Linux Kernel ext4 Documentation](https://www.kernel.org/doc/html/latest/filesystems/ext4/)
- [ext4 On-Disk Layout](https://ext4.wiki.kernel.org/index.php/Ext4_Disk_Layout)
- [e2fsprogs Source Code](https://github.com/tytso/e2fsprogs)

### Reference Implementations
- mkfs.ext4 (e2fsprogs/misc/mke2fs.c)
- Linux kernel ext4 driver (fs/ext4/)
- lwext4 (embedded ext4 implementation)

### Testing Tools
- e2fsck - Filesystem checker
- dumpe2fs - Filesystem dumper
- debugfs - Filesystem debugger
- hexdump - Binary inspection

## Appendix B: Common Pitfalls

1. **Byte Order**: ext4 uses little-endian throughout
2. **Alignment**: Many structures require 4-byte alignment
3. **Checksums**: Different algorithms for different metadata
4. **Feature Flags**: Some features require multiple flags
5. **Reserved Space**: Don't forget root-reserved blocks
6. **Sparse Files**: Extent tree complexity
7. **Time Stamps**: Unix time, but with nanosecond extra fields

## Next Steps

1. Review and approve this plan
2. Set up development branch
3. Create test infrastructure
4. Begin Phase 1 implementation
5. Weekly progress reviews

---

*This is a living document and will be updated as implementation progresses.*