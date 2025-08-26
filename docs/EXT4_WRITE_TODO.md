# EXT4 Write Implementation TODO

## Overview
This document outlines the complete requirements for implementing write support in the Moses EXT4 native implementation. Currently, the implementation is read-only and suitable for formatting and reading, but not for mounting as a read-write filesystem.

## Current State
- ✅ **Read Operations**: Fully implemented
- ✅ **Formatting**: Complete with modern EXT4 features
- ❌ **Write Operations**: Not implemented
- ❌ **Journal Support**: Detection only, no replay or writing

## Implementation Phases

### Phase 1: Foundation (Prerequisites)
Before implementing any write operations, we need core infrastructure:

#### 1.1 Block Allocation System
- [ ] Free block bitmap management
- [ ] Block allocator with best-fit/first-fit strategies
- [ ] Multi-block allocation for large files
- [ ] Preallocation support for performance
- [ ] Block group selection algorithm

**Implementation Notes:**
```rust
// Needed structures
struct BlockAllocator {
    bitmap_cache: HashMap<u32, Bitmap>,
    reserved_blocks: u64,
    goal_block: Option<u64>, // Allocation hint
}

impl BlockAllocator {
    fn allocate_blocks(&mut self, count: u32, goal: Option<u64>) -> Result<Vec<u64>, Error>;
    fn free_blocks(&mut self, blocks: &[u64]) -> Result<(), Error>;
    fn update_bitmap(&mut self, group: u32, block: u32, allocated: bool) -> Result<(), Error>;
}
```

#### 1.2 Inode Allocation System
- [ ] Free inode bitmap management
- [ ] Inode allocator with directory hinting
- [ ] Inode initialization with proper defaults
- [ ] Inode table updates

**Implementation Notes:**
```rust
struct InodeAllocator {
    bitmap_cache: HashMap<u32, Bitmap>,
    last_alloc_group: u32, // For directory spreading
}

impl InodeAllocator {
    fn allocate_inode(&mut self, is_directory: bool) -> Result<u32, Error>;
    fn free_inode(&mut self, inode_num: u32) -> Result<(), Error>;
    fn initialize_inode(&mut self, inode: &mut Ext4Inode, mode: u16) -> Result<(), Error>;
}
```

#### 1.3 Transaction System (Critical for Consistency)
- [ ] Transaction handle creation
- [ ] Metadata buffering
- [ ] Write ordering enforcement
- [ ] Rollback capability

**Implementation Notes:**
```rust
struct Transaction {
    id: u64,
    blocks: Vec<(u64, Vec<u8>)>, // Block number -> data
    metadata: Vec<MetadataUpdate>,
    state: TransactionState,
}

enum TransactionState {
    Active,
    Committing,
    Committed,
    Aborted,
}
```

### Phase 2: Basic Write Operations

#### 2.1 File Creation
- [ ] `create()` - Create new regular file
- [ ] Allocate inode
- [ ] Initialize inode metadata
- [ ] Add directory entry
- [ ] Update parent directory

**Critical Steps:**
1. Check parent directory permissions
2. Verify name doesn't exist
3. Allocate inode from bitmap
4. Initialize inode structure
5. Add directory entry to parent
6. Update parent mtime/ctime
7. Update free inode count

#### 2.2 File Writing
- [ ] `write()` - Write data to file
- [ ] Block allocation for new data
- [ ] Extent tree updates (EXT4)
- [ ] Indirect block updates (EXT2/3)
- [ ] Handle sparse files
- [ ] Update file size
- [ ] Update timestamps

**Critical Steps:**
1. Check file permissions
2. Determine blocks needed
3. Allocate blocks if needed
4. Update extent tree or indirect blocks
5. Write actual data
6. Update inode size and timestamps
7. Handle partial writes correctly

#### 2.3 File Deletion
- [ ] `unlink()` - Delete file
- [ ] Remove directory entry
- [ ] Decrease link count
- [ ] Free inode if link count = 0
- [ ] Free data blocks
- [ ] Update bitmaps

**Critical Steps:**
1. Find and remove directory entry
2. Decrease inode link count
3. If link count reaches 0:
   - Free all data blocks
   - Free indirect blocks
   - Clear inode
   - Update free counts

### Phase 3: Directory Operations

#### 3.1 Directory Creation
- [ ] `mkdir()` - Create directory
- [ ] Create . and .. entries
- [ ] Update parent link count
- [ ] Initialize directory blocks

**Critical Steps:**
1. Allocate inode as directory
2. Allocate first directory block
3. Create "." entry pointing to self
4. Create ".." entry pointing to parent
5. Increment parent's link count
6. Add entry in parent directory

#### 3.2 Directory Removal
- [ ] `rmdir()` - Remove directory
- [ ] Verify directory is empty
- [ ] Update parent link count
- [ ] Free directory blocks

**Critical Steps:**
1. Verify directory only has . and ..
2. Remove entry from parent
3. Decrease parent's link count
4. Free directory blocks
5. Free directory inode

#### 3.3 Directory Entry Management
- [ ] Add directory entry
- [ ] Remove directory entry
- [ ] Handle directory expansion
- [ ] Directory compaction
- [ ] Hash tree directories (large dirs)

### Phase 4: Advanced Operations

#### 4.1 File Renaming
- [ ] `rename()` - Move/rename files
- [ ] Handle same-directory rename
- [ ] Handle cross-directory move
- [ ] Atomic operation guarantee
- [ ] Handle directory renaming

**Critical Steps:**
1. Check permissions on both paths
2. Handle special cases (overwriting, loops)
3. Add new directory entry
4. Remove old directory entry
5. Update ctime on inode
6. Handle directory parent changes

#### 4.2 Truncation
- [ ] `truncate()` - Change file size
- [ ] Free blocks beyond new size
- [ ] Update extent tree
- [ ] Handle indirect blocks
- [ ] Update inode size

#### 4.3 Symbolic Links
- [ ] `symlink()` - Create symbolic link
- [ ] Store target path
- [ ] Handle inline symlinks (< 60 bytes)
- [ ] Handle external symlinks

#### 4.4 Hard Links
- [ ] `link()` - Create hard link
- [ ] Increase link count
- [ ] Add directory entry
- [ ] Verify not directory

### Phase 5: Metadata Operations

#### 5.1 Permission Changes
- [ ] `chmod()` - Change permissions
- [ ] Validate permission bits
- [ ] Update inode
- [ ] Check ownership

#### 5.2 Ownership Changes
- [ ] `chown()` - Change ownership
- [ ] Update uid/gid
- [ ] Clear setuid/setgid if needed

#### 5.3 Timestamp Updates
- [ ] `utimes()` - Update timestamps
- [ ] Handle atime updates
- [ ] Handle mtime updates
- [ ] Respect mount options (noatime, etc.)

#### 5.4 Extended Attributes
- [ ] `setxattr()` - Set extended attribute
- [ ] `getxattr()` - Get extended attribute
- [ ] `listxattr()` - List attributes
- [ ] `removexattr()` - Remove attribute
- [ ] Handle inline and external xattrs

### Phase 6: Journal Implementation

#### 6.1 Journal Structure
- [ ] Journal superblock management
- [ ] Transaction blocks
- [ ] Descriptor blocks
- [ ] Commit blocks
- [ ] Revoke blocks

#### 6.2 Journal Operations
- [ ] Start transaction
- [ ] Add metadata to transaction
- [ ] Commit transaction
- [ ] Journal replay on mount
- [ ] Checkpoint management

**Journal Write Sequence:**
1. Write descriptor block
2. Write metadata blocks
3. Write commit block
4. Update journal superblock

#### 6.3 Recovery
- [ ] Scan journal on mount
- [ ] Replay committed transactions
- [ ] Handle incomplete transactions
- [ ] Verify checksums

### Phase 7: Optimization & Safety

#### 7.1 Write Optimization
- [ ] Delayed allocation
- [ ] Multi-block allocation
- [ ] Extent preallocation
- [ ] Write clustering
- [ ] Directory indexing (htree)

#### 7.2 Consistency Guarantees
- [ ] Ordered data mode
- [ ] Write barriers
- [ ] Sync operations
- [ ] Flush caches properly

#### 7.3 Error Handling
- [ ] Disk full handling
- [ ] I/O error recovery
- [ ] Corruption detection
- [ ] Graceful degradation

### Phase 8: Testing Strategy

#### 8.1 Unit Tests
- [ ] Block allocator tests
- [ ] Inode allocator tests
- [ ] Directory operation tests
- [ ] Journal tests

#### 8.2 Integration Tests
- [ ] File operation sequences
- [ ] Power failure simulation
- [ ] Concurrent access tests
- [ ] Stress tests

#### 8.3 Compatibility Tests
- [ ] Mount on Linux and verify
- [ ] Run e2fsck after operations
- [ ] Compare with ext4 reference implementation
- [ ] Test with various block sizes

## Implementation Priority

### Minimum Viable Write Support (MVP)
1. Block allocation
2. Inode allocation  
3. File creation
4. File writing (basic)
5. File deletion
6. Directory creation

### Full Write Support
All of the above phases completed

## Complexity Estimates

| Component | Complexity | Time Estimate |
|-----------|-----------|---------------|
| Block Allocation | High | 1 week |
| Inode Management | Medium | 3 days |
| Basic File Ops | High | 1 week |
| Directory Ops | Medium | 4 days |
| Journal | Very High | 2 weeks |
| Testing | High | 1 week |
| **Total** | **Very High** | **5-6 weeks** |

## Risks & Challenges

1. **Data Corruption**: Any bug in write path can corrupt filesystem
2. **Journal Complexity**: Journal implementation is complex and critical
3. **Atomicity**: Ensuring atomic operations across crashes
4. **Performance**: Write performance optimization while maintaining safety
5. **Compatibility**: Ensuring Linux ext4 can read our writes

## Alternative Approaches

### Option 1: Use libext2fs
- Pro: Battle-tested implementation
- Con: External dependency, less control

### Option 2: Kernel Module Wrapper
- Pro: Use kernel's ext4 implementation
- Con: Platform-specific, requires privileges

### Option 3: Read-Only + Format-Only
- Pro: Simple, safe, covers main use cases
- Con: Cannot mount read-write

## Recommendation

Given the complexity and risk, consider:
1. **Keep current read-only implementation** for safe browsing
2. **Focus on excellent formatting** as the primary feature
3. **Consider libext2fs binding** if write support becomes critical
4. **Implement writes incrementally** with extensive testing between phases

## References

- [ext4 Disk Layout](https://ext4.wiki.kernel.org/index.php/Ext4_Disk_Layout)
- [ext4 Journal (jbd2)](https://www.kernel.org/doc/html/latest/filesystems/ext4/journal.html)
- [Linux kernel ext4 source](https://github.com/torvalds/linux/tree/master/fs/ext4)
- [e2fsprogs source](https://github.com/tytso/e2fsprogs)