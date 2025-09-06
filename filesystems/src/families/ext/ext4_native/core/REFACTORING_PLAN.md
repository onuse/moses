# Safe Refactoring Plan for ext4 Modularization

## Goal
Refactor ext4 implementation to be modular and reusable for ext2/ext3, WITHOUT breaking existing functionality.

## Current Structure
```
ext4_native/core/
├── formatter_impl.rs   # Main formatting logic (600+ lines)
├── structures.rs       # Superblock, Inode, GroupDesc, etc.
├── types.rs           # FilesystemParams, FilesystemLayout
├── bitmap.rs          # Bitmap operations
├── checksum.rs        # CRC32 calculations
└── constants.rs       # Feature flags and constants
```

## Proposed Modular Structure
```
ext4_native/core/
├── base/              # Shared ext family components
│   ├── superblock.rs  # Trait + base implementation
│   ├── inode.rs       # Trait + base implementation  
│   ├── group_desc.rs  # Trait + base implementation
│   ├── directory.rs   # Directory entry handling
│   └── bitmap.rs      # Bitmap operations (unchanged)
├── ext2/              # ext2-specific implementations
│   ├── mod.rs         # Ext2 formatter
│   └── params.rs      # Ext2 parameters
├── ext3/              # ext3-specific implementations
│   ├── mod.rs         # Ext3 formatter
│   ├── params.rs      # Ext3 parameters
│   └── journal.rs     # Journal support
├── ext4/              # ext4-specific implementations
│   ├── mod.rs         # Ext4 formatter (current)
│   ├── params.rs      # Ext4 parameters
│   └── extents.rs     # Extent tree support
└── formatter_impl.rs  # Generic formatter using traits
```

## Refactoring Steps

### Phase 1: Create Traits (No Breaking Changes)
1. Create trait definitions for Superblock, Inode, GroupDesc
2. Implement traits for existing structures
3. Run golden tests - must pass

### Phase 2: Extract Base Components
1. Move shared logic to base/ module
2. Keep existing structures working via traits
3. Run golden tests - must pass

### Phase 3: Create Version-Specific Implementations
1. Create ext4/ module with current implementation
2. Verify identical behavior
3. Run golden tests - must pass

### Phase 4: Add ext2/ext3
1. Implement ext2 using base components
2. Implement ext3 (ext2 + journal)
3. Test with Linux mounting

## Key Traits

```rust
trait ExtSuperblock {
    fn init(&mut self, params: &ExtParams, layout: &ExtLayout);
    fn set_feature_flags(&mut self, compat: u32, incompat: u32, ro_compat: u32);
    fn update_free_counts(&mut self, free_blocks: u64, free_inodes: u32);
    fn write_to_buffer(&self, buffer: &mut [u8]) -> Result<(), Error>;
}

trait ExtInode {
    fn init_directory(&mut self, params: &ExtParams);
    fn init_file(&mut self, params: &ExtParams);
    fn set_extent_tree(&mut self, extents: &[Extent]);
    fn set_block_pointers(&mut self, blocks: &[u32]);
}

trait ExtGroupDesc {
    fn init(&mut self, group: u32, layout: &ExtLayout);
    fn set_free_counts(&mut self, blocks: u32, inodes: u32);
    fn update_checksum(&mut self, group: u32, sb: &dyn ExtSuperblock);
}
```

## Safety Guarantees

1. **Golden Tests**: Comprehensive tests that verify exact byte patterns
2. **Incremental Changes**: Each phase must pass all tests
3. **Parallel Implementation**: New modular code runs alongside old code
4. **Feature Flags**: Can toggle between old and new implementation
5. **Binary Compatibility**: Produced filesystems must be identical

## Success Criteria

- [ ] All existing tests pass
- [ ] Golden byte patterns unchanged  
- [ ] Linux can mount filesystems created by refactored code
- [ ] Performance unchanged or improved
- [ ] Code is cleaner and more maintainable
- [ ] ext2/ext3 can reuse 80%+ of code