# Moses Plugin System Design Document
Version 1.0.0 - DRAFT

## Executive Summary

Moses aims to be a universal filesystem manipulation platform through a robust plugin architecture. This document defines how filesystem plugins integrate with Moses core, enabling third-party developers to add support for any filesystem while maintaining consistency, security, and reliability.

## Core Philosophy

1. **Separation of Concerns**: Moses core handles device management, UI, and orchestration. Plugins handle filesystem-specific operations.
2. **Progressive Enhancement**: Plugins can start simple (format-only) and add capabilities over time.
3. **Safety First**: All plugins run with capability declarations and sandboxing where possible.
4. **Cross-Platform**: Plugin API must work on Windows, Linux, and macOS.

## Architecture Overview

```
┌─────────────────────────────────────────────┐
│              Moses Application              │
├─────────────────────────────────────────────┤
│                  Moses Core                 │
│  ┌─────────────────────────────────────┐   │
│  │        Plugin Manager               │   │
│  ├─────────────────────────────────────┤   │
│  │   Device Manager | UI | Scheduler   │   │
│  └─────────────────────────────────────┘   │
├─────────────────────────────────────────────┤
│              Plugin Interface               │
├────────┬────────┬────────┬────────┬────────┤
│  ext4  │  NTFS  │ FAT32  │ Btrfs │  ...   │
│ Plugin │ Plugin │ Plugin │ Plugin │        │
└────────┴────────┴────────┴────────┴────────┘
```

## Plugin Trait Definition

### Core Trait
```rust
/// Every filesystem plugin must implement this trait
pub trait FilesystemPlugin: Send + Sync {
    /// Unique identifier for this plugin (e.g., "com.moses.ext4")
    fn id(&self) -> &str;
    
    /// Human-readable name (e.g., "EXT4 Filesystem")
    fn name(&self) -> &str;
    
    /// Version of the plugin
    fn version(&self) -> Version;
    
    /// Capabilities this plugin provides
    fn capabilities(&self) -> PluginCapabilities;
    
    /// Filesystem types this plugin handles (e.g., ["ext2", "ext3", "ext4"])
    fn supported_filesystems(&self) -> Vec<FilesystemType>;
    
    /// Check if this plugin can handle a device
    fn probe(&self, device: &Device) -> Result<ProbeResult, PluginError>;
    
    /// Get a formatter implementation (if capable)
    fn get_formatter(&self) -> Option<Box<dyn FilesystemFormatter>>;
    
    /// Get a filesystem reader implementation (if capable)
    fn get_reader(&self) -> Option<Box<dyn FilesystemReader>>;
    
    /// Get a filesystem writer implementation (if capable)
    fn get_writer(&self) -> Option<Box<dyn FilesystemWriter>>;
    
    /// Get a partition manager implementation (if capable)
    fn get_partition_manager(&self) -> Option<Box<dyn PartitionManager>>;
}
```

### Capability Declaration
```rust
pub struct PluginCapabilities {
    /// Can format devices with this filesystem
    pub can_format: bool,
    
    /// Can read files from this filesystem
    pub can_read: bool,
    
    /// Can write files to this filesystem
    pub can_write: bool,
    
    /// Can verify filesystem integrity
    pub can_verify: bool,
    
    /// Can manage partitions (MBR/GPT)
    pub can_partition: bool,
    
    /// Can convert from other filesystems
    pub can_convert_from: Vec<FilesystemType>,
    
    /// Required privileges
    pub required_privileges: PrivilegeLevel,
    
    /// Platform restrictions
    pub supported_platforms: Vec<Platform>,
}

pub enum PrivilegeLevel {
    /// No special privileges needed
    User,
    /// Needs admin/root for device access
    Administrator,
    /// Needs kernel module or driver
    Kernel,
}
```

## Filesystem Operations

### Formatter Trait
```rust
pub trait FilesystemFormatter: Send + Sync {
    /// Validate format options
    async fn validate_options(&self, options: &FormatOptions) -> Result<(), ValidationError>;
    
    /// Perform dry run / simulation
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport>;
    
    /// Execute format operation
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), FormatError>;
    
    /// Verify formatted filesystem
    async fn verify(&self, device: &Device) -> Result<VerificationReport>;
}
```

### Reader Trait
```rust
pub trait FilesystemReader: Send + Sync {
    /// Mount/open filesystem for reading
    async fn mount(&self, device: &Device, options: &MountOptions) -> Result<Box<dyn Filesystem>>;
}

pub trait Filesystem: Send + Sync {
    /// Read directory contents
    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
    
    /// Read file contents
    async fn read_file(&self, path: &Path) -> Result<Box<dyn AsyncRead>>;
    
    /// Get file/directory metadata
    async fn stat(&self, path: &Path) -> Result<FileMetadata>;
    
    /// Walk filesystem tree
    fn walk(&self, path: &Path) -> Box<dyn Stream<Item = Result<DirEntry>>>;
}
```

### Writer Trait
```rust
pub trait FilesystemWriter: FilesystemReader {
    /// Mount filesystem for writing
    async fn mount_write(&self, device: &Device, options: &MountOptions) -> Result<Box<dyn WritableFilesystem>>;
}

pub trait WritableFilesystem: Filesystem {
    /// Write file
    async fn write_file(&self, path: &Path, content: Box<dyn AsyncRead>) -> Result<()>;
    
    /// Create directory
    async fn create_dir(&self, path: &Path) -> Result<()>;
    
    /// Delete file or directory
    async fn remove(&self, path: &Path) -> Result<()>;
    
    /// Rename/move file or directory
    async fn rename(&self, from: &Path, to: &Path) -> Result<()>;
    
    /// Set metadata
    async fn set_metadata(&self, path: &Path, metadata: &FileMetadata) -> Result<()>;
    
    /// Flush changes
    async fn sync(&self) -> Result<()>;
}
```

## Plugin Manifest

Each plugin must include a `moses-plugin.toml` manifest:

```toml
[plugin]
# Unique identifier
id = "com.moses.ext4"
# Display name
name = "EXT4 Filesystem Plugin"
# Semantic version
version = "1.0.0"
# Authors
authors = ["Moses Team <team@moses.app>"]
# License
license = "MIT"
# Minimum Moses version required
moses_version = ">=0.5.0"

[capabilities]
format = true
read = true
write = false
verify = true
partition = false
convert_from = []

[requirements]
# Required system privileges
privileges = "administrator"
# Supported platforms
platforms = ["windows", "linux", "macos"]
# Required system libraries
system_libs = []

[filesystems]
# Filesystems this plugin handles
[[filesystems.supported]]
type = "ext4"
aliases = ["ext4", "fourth-extended"]
max_volume_size = "1EiB"
max_file_size = "16TiB"

[[filesystems.supported]]
type = "ext3"
aliases = ["ext3", "third-extended"]
max_volume_size = "32TiB"
max_file_size = "2TiB"

[[filesystems.supported]]
type = "ext2"
aliases = ["ext2", "second-extended"]
max_volume_size = "32TiB"
max_file_size = "2TiB"

[build]
# Build configuration for different platforms
[build.windows]
features = ["windows-native"]

[build.linux]
features = ["linux-native"]

[build.macos]
features = ["macos-native"]
```

## Plugin Discovery and Loading

### Discovery Locations
1. **Built-in plugins**: Compiled into Moses binary
2. **System plugins**: `/usr/lib/moses/plugins` (Linux), `C:\Program Files\Moses\plugins` (Windows)
3. **User plugins**: `~/.moses/plugins`
4. **Development**: Path specified via `MOSES_PLUGIN_PATH` environment variable

### Loading Process
```rust
pub struct PluginManager {
    plugins: HashMap<String, Box<dyn FilesystemPlugin>>,
}

impl PluginManager {
    /// Discover and load all plugins
    pub async fn load_plugins(&mut self) -> Result<Vec<PluginInfo>> {
        let mut loaded = Vec::new();
        
        // 1. Load built-in plugins
        self.load_builtin_plugins(&mut loaded)?;
        
        // 2. Scan plugin directories
        for dir in Self::plugin_directories() {
            self.scan_directory(&dir, &mut loaded).await?;
        }
        
        // 3. Validate plugin compatibility
        self.validate_plugins(&loaded)?;
        
        // 4. Initialize plugins
        for plugin in &loaded {
            plugin.initialize().await?;
        }
        
        Ok(loaded)
    }
    
    /// Find plugin for a specific filesystem
    pub fn find_plugin(&self, fs_type: &FilesystemType) -> Option<&dyn FilesystemPlugin> {
        self.plugins.values()
            .find(|p| p.supported_filesystems().contains(fs_type))
            .map(|p| p.as_ref())
    }
}
```

## Security Considerations

### Sandboxing
- Plugins should run with minimal privileges
- Use OS-specific sandboxing where available (AppArmor, Windows AppContainer)
- Limit filesystem access to device being operated on

### Validation
- Verify plugin signatures (future enhancement)
- Validate manifest before loading
- Check capability declarations match actual implementation

### Resource Limits
- Memory usage caps
- Timeout for long-running operations
- Rate limiting for device I/O

## Error Handling

### Plugin Errors
```rust
#[derive(Debug, Error)]
pub enum PluginError {
    /// Plugin initialization failed
    #[error("Failed to initialize plugin: {0}")]
    InitializationFailed(String),
    
    /// Operation not supported by this plugin
    #[error("Operation not supported: {0}")]
    NotSupported(String),
    
    /// Device access error
    #[error("Device access error: {0}")]
    DeviceError(String),
    
    /// Filesystem corruption detected
    #[error("Filesystem corruption: {0}")]
    CorruptionDetected(String),
    
    /// Invalid options provided
    #[error("Invalid options: {0}")]
    InvalidOptions(String),
    
    /// Platform-specific error
    #[error("Platform error: {0}")]
    PlatformError(String),
}
```

## Plugin Development Guide

### Minimal Plugin Example
```rust
use moses_plugin_api::*;

pub struct MyFilesystemPlugin;

impl FilesystemPlugin for MyFilesystemPlugin {
    fn id(&self) -> &str {
        "com.example.myfs"
    }
    
    fn name(&self) -> &str {
        "My Filesystem"
    }
    
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            can_format: true,
            can_read: false,
            can_write: false,
            can_verify: false,
            can_partition: false,
            can_convert_from: vec![],
            required_privileges: PrivilegeLevel::Administrator,
            supported_platforms: vec![Platform::Linux],
        }
    }
    
    fn supported_filesystems(&self) -> Vec<FilesystemType> {
        vec![FilesystemType::Custom("myfs".to_string())]
    }
    
    fn probe(&self, device: &Device) -> Result<ProbeResult, PluginError> {
        // Check if device has MyFS
        Ok(ProbeResult::NotRecognized)
    }
    
    fn get_formatter(&self) -> Option<Box<dyn FilesystemFormatter>> {
        Some(Box::new(MyFilesystemFormatter))
    }
    
    // Other methods return None since we only support formatting
    fn get_reader(&self) -> Option<Box<dyn FilesystemReader>> { None }
    fn get_writer(&self) -> Option<Box<dyn FilesystemWriter>> { None }
    fn get_partition_manager(&self) -> Option<Box<dyn PartitionManager>> { None }
}

// Export plugin - Moses will look for this symbol
#[no_mangle]
pub extern "C" fn moses_plugin_create() -> Box<dyn FilesystemPlugin> {
    Box::new(MyFilesystemPlugin)
}
```

### Testing Plugins
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use moses_plugin_test::*;
    
    #[test]
    fn test_plugin_metadata() {
        let plugin = MyFilesystemPlugin;
        assert_eq!(plugin.id(), "com.example.myfs");
        assert!(plugin.capabilities().can_format);
    }
    
    #[tokio::test]
    async fn test_format() {
        let plugin = MyFilesystemPlugin;
        let device = create_test_device(1024 * 1024 * 100); // 100MB
        let formatter = plugin.get_formatter().unwrap();
        
        let result = formatter.format(&device, &Default::default()).await;
        assert!(result.is_ok());
    }
}
```

## Versioning and Compatibility

### API Stability
- Plugin API follows semantic versioning
- Major version changes may break compatibility
- Moses maintains backward compatibility for at least 2 major versions

### Migration Path
```rust
/// Plugins can support multiple API versions
impl FilesystemPlugin for MyPlugin {
    fn api_version(&self) -> ApiVersion {
        ApiVersion::V1  // Current version
    }
    
    fn migrate_from_v0(&self, old_config: &v0::Config) -> Result<Config> {
        // Migration logic for old plugins
    }
}
```

## Performance Considerations

### Async Operations
- All I/O operations must be async
- Use tokio for async runtime consistency
- Support cancellation tokens for long operations

### Progress Reporting
```rust
pub trait ProgressReporter: Send + Sync {
    /// Report progress (0.0 to 1.0)
    fn report_progress(&self, progress: f32);
    
    /// Report status message
    fn report_status(&self, message: &str);
    
    /// Check if operation should be cancelled
    fn is_cancelled(&self) -> bool;
}
```

### Caching
- Plugins should cache filesystem metadata where appropriate
- Moses core provides shared cache infrastructure
- Cache invalidation on device changes

## Future Enhancements

### Phase 1 (Current)
- Basic plugin loading
- Format operation support
- Built-in plugins for major filesystems

### Phase 2
- Read-only filesystem access
- Plugin marketplace/registry
- Plugin signing and verification

### Phase 3
- Read-write filesystem access
- Cross-filesystem transfer
- Live filesystem conversion

### Phase 4
- Network filesystem support
- Cloud storage plugins
- Filesystem-in-file support (disk images)

## Reference Implementation

The EXT4 plugin serves as the reference implementation demonstrating all features and best practices. See `/plugins/ext4/` for the complete implementation.

## FAQ

**Q: Can plugins be written in languages other than Rust?**
A: Currently, plugins must expose a C ABI. This allows plugins in C, C++, and other languages that can export C functions. Future versions may support WASM plugins.

**Q: How are plugin conflicts resolved?**
A: If multiple plugins support the same filesystem, Moses uses a priority system: built-in > system > user. Users can override this in settings.

**Q: Can plugins depend on other plugins?**
A: Not currently. Each plugin must be self-contained. This may be added in future versions.

**Q: How are plugins distributed?**
A: Initially, plugins are distributed as dynamic libraries (.dll, .so, .dylib). Future versions will support a plugin registry.

## Appendix A: Type Definitions

```rust
// Complete type definitions for plugin API
pub struct Device {
    pub id: String,
    pub path: PathBuf,
    pub size: u64,
    pub sector_size: u32,
    pub is_removable: bool,
    pub is_system: bool,
}

pub struct FormatOptions {
    pub filesystem_type: FilesystemType,
    pub label: Option<String>,
    pub cluster_size: Option<u32>,
    pub quick_format: bool,
    pub enable_compression: bool,
    pub enable_encryption: bool,
}

pub struct MountOptions {
    pub read_only: bool,
    pub no_exec: bool,
    pub no_atime: bool,
}

pub struct FileMetadata {
    pub size: u64,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub accessed: SystemTime,
    pub permissions: Permissions,
    pub attributes: Attributes,
}

pub struct DirEntry {
    pub name: OsString,
    pub path: PathBuf,
    pub metadata: FileMetadata,
    pub entry_type: EntryType,
}

pub enum EntryType {
    File,
    Directory,
    Symlink(PathBuf),
    Device,
    Other,
}
```

## Appendix B: Plugin Checklist

Before submitting a plugin:

- [ ] Implements all required trait methods
- [ ] Includes complete manifest file
- [ ] Handles all error cases gracefully
- [ ] Includes comprehensive tests
- [ ] Documents all public APIs
- [ ] Follows Moses coding standards
- [ ] Tested on all declared platforms
- [ ] Includes example usage
- [ ] Performance benchmarks included
- [ ] Security review completed

---

*This document is a living specification and will be updated as the plugin system evolves.*

*Last Updated: 2024*
*Version: 1.0.0-DRAFT*