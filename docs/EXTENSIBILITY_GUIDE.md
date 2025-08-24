# Moses Extensibility Guide

## Adding Support for New Filesystems

Moses is designed to be easily extended with new filesystem support. This guide shows how to contribute a new filesystem to Moses.

## Architecture Overview

```
moses/
├── moses-core/           # Core types and traits
│   └── traits.rs         # FilesystemFormatter trait
├── moses-formatters/     # All filesystem implementations
│   ├── ext4/            
│   ├── ntfs/            
│   ├── fat32/           
│   └── your_fs/         # Your filesystem here!
└── moses-platform/       # Platform-specific device I/O (handled for you)
```

## Quick Start: Adding a New Filesystem

### Step 1: Create Your Module

Create a new module in `moses-formatters/src/`:

```rust
// moses-formatters/src/newfs/mod.rs
use moses_core::{Device, FormatOptions, FilesystemFormatter, MosesError};

pub struct NewFsFormatter;

#[async_trait]
impl FilesystemFormatter for NewFsFormatter {
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), MosesError> {
        // Validate cluster size, label, etc.
        Ok(())
    }
    
    async fn can_format(&self, device: &Device) -> bool {
        // Check if device is suitable
        !device.is_system && device.size >= MIN_SIZE
    }
    
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Your formatting logic here
        // Moses handles device I/O for you!
        Ok(())
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        // Simulate format for preview
        Ok(SimulationReport::default())
    }
}
```

### Step 2: Register Your Filesystem

Add to `moses-formatters/src/lib.rs`:

```rust
#[cfg(feature = "newfs")]
pub mod newfs;
#[cfg(feature = "newfs")]
pub use newfs::NewFsFormatter;
```

Add to `Cargo.toml`:

```toml
[features]
newfs = []
default = ["ext4", "ntfs", "fat32", "newfs"]
```

### Step 3: Wire into UI

Add to the format executor in `src-tauri/src/lib.rs`:

```rust
"newfs" => {
    let formatter = NewFsFormatter;
    formatter.format(&device, &options).await?;
    Ok(format!("Successfully formatted {} as NewFS", device.name))
}
```

## Filesystem Implementation Guide

### Required Components

1. **Superblock/Boot Sector**: Primary filesystem metadata
2. **Allocation Structures**: Bitmaps, tables, or trees
3. **Directory Structure**: Root directory at minimum
4. **Metadata**: Timestamps, permissions, attributes

### Using Moses Helpers

Moses provides utilities to make implementation easier:

```rust
use moses_filesystems::common::{
    AlignedBuffer,      // For sector-aligned I/O
    DeviceWriter,       // Cross-platform device writing
    Checksum,          // Various checksum algorithms
    Endian,            // Endianness helpers
};

// Write to device (platform-agnostic)
let writer = DeviceWriter::new(device)?;
writer.write_at(0, &superblock_bytes)?;
```

### Testing Your Filesystem

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use moses_test_utils::{create_test_device, verify_filesystem};
    
    #[tokio::test]
    async fn test_format_small_device() {
        let device = create_test_device(100 * 1024 * 1024); // 100MB
        let formatter = NewFsFormatter;
        
        let result = formatter.format(&device, &Default::default()).await;
        assert!(result.is_ok());
        
        // Verify the filesystem is valid
        assert!(verify_filesystem(&device, "newfs").is_ok());
    }
}
```

## Platform Considerations

Moses handles platform differences for you:

- **Windows**: UAC elevation, physical drive access
- **Linux**: Root privileges, device unmounting
- **macOS**: Disk arbitration, BSD device names

Your formatter just needs to generate the correct filesystem structures!

## Best Practices

### 1. Start Simple
- Begin with format-only support
- Add verification next
- Reading/writing can come later

### 2. Follow Existing Patterns
Look at existing formatters for guidance:
- `ext4/` - Complex filesystem with multiple versions
- `fat32/` - Simple filesystem with wide compatibility
- `ntfs/` - Windows-specific considerations

### 3. Document Limitations
Be clear about what your formatter supports:
```rust
/// NewFS Formatter
/// 
/// Limitations:
/// - Maximum volume size: 1TB
/// - Maximum file size: 4GB
/// - No encryption support yet
```

### 4. Incremental Development
Use feature flags for experimental features:
```rust
#[cfg(feature = "newfs-experimental")]
fn enable_advanced_features() { ... }
```

## Contributing Checklist

Before submitting a PR:

- [ ] Implements `FilesystemFormatter` trait
- [ ] Includes unit tests
- [ ] Tested on 100MB, 1GB, and 100GB devices
- [ ] Works on Windows and Linux (minimum)
- [ ] Handles edge cases (full disk, tiny disk)
- [ ] Documentation in code
- [ ] Added to UI filesystem list
- [ ] Feature flag added to Cargo.toml

## Future Extensibility

Once your formatter is working, consider adding:

### Verification Support
```rust
impl FilesystemVerifier for NewFsFormatter {
    async fn verify(&self, device: &Device) -> Result<VerificationReport, MosesError> {
        // Check filesystem integrity
    }
}
```

### Read Support
```rust
impl FilesystemReader for NewFsFormatter {
    async fn list_files(&self, device: &Device) -> Result<Vec<FileEntry>, MosesError> {
        // Read directory structure
    }
}
```

### Partition Table Support
```rust
impl PartitionManager for NewFsFormatter {
    async fn read_partition_table(&self, device: &Device) -> Result<PartitionTable, MosesError> {
        // Read MBR/GPT
    }
}
```

## Getting Help

- Check existing implementations in `moses-formatters/src/`
- Open an issue for design discussions
- Join our Discord for real-time help
- Tag your PR with `new-filesystem`

## Examples

### Minimal Formatter
See `moses-formatters/src/examples/minimal.rs` for the simplest possible formatter.

### Complex Formatter
See `moses-formatters/src/ext4/` for a full-featured implementation with multiple filesystem versions.

---

Remember: Moses handles the hard platform-specific stuff. You just focus on your filesystem's format!