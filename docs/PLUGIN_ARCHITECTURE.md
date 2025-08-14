# Moses Plugin Architecture

## Overview

Moses uses a unified plugin architecture where all filesystem formatters are treated equally. Whether it's a modern filesystem like BTRFS or a historical one like Commodore 1541, they all implement the same `FilesystemFormatter` trait.

## Core Architecture

### 1. Formatter Trait (Already Exists)

```rust
// In moses_core/src/formatter.rs
#[async_trait]
pub trait FilesystemFormatter: Send + Sync {
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError>;
    async fn verify(&self, device: &Device) -> Result<bool, MosesError>;
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError>;
    fn supported_platforms(&self) -> Vec<Platform>;
}
```

### 2. Formatter Registry (New)

```rust
// In moses_core/src/registry.rs
pub struct FormatterRegistry {
    formatters: HashMap<String, Arc<dyn FilesystemFormatter>>,
    metadata: HashMap<String, FormatterMetadata>,
}

pub struct FormatterMetadata {
    pub name: String,
    pub aliases: Vec<String>,           // e.g., ["msdos", "fat"] for FAT32
    pub category: FormatterCategory,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub platform_support: Vec<Platform>,
    pub required_tools: Vec<ExternalTool>,
    pub documentation_url: Option<String>,
}

pub enum FormatterCategory {
    Modern,           // ext4, btrfs, zfs
    Legacy,           // fat32, ntfs
    Historical,       // Commodore, Amiga
    Console,          // PlayStation, Xbox
    Embedded,         // YAFFS, UBIFS
    Experimental,     // Research filesystems
}
```

### 3. Plugin Loading System

```rust
// In moses_core/src/plugin.rs
pub trait FormatterPlugin {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn create_formatter(&self) -> Box<dyn FilesystemFormatter>;
    fn metadata(&self) -> FormatterMetadata;
}

// Dynamic loading for external plugins
pub struct PluginLoader {
    plugin_dir: PathBuf,
    loaded_plugins: Vec<Box<dyn FormatterPlugin>>,
}

impl PluginLoader {
    pub fn load_plugin(&mut self, path: &Path) -> Result<(), MosesError> {
        // Load .dll/.so/.dylib files
        // Or load WASM modules for sandboxed plugins
    }
}
```

## Plugin Types

### 1. Built-in Plugins
Compiled directly into Moses binary. These are the core, well-tested formatters.

```rust
// In formatters/src/lib.rs
pub fn register_builtin_formatters(registry: &mut FormatterRegistry) {
    // Modern
    registry.register("ext4", Box::new(Ext4Formatter), metadata_ext4());
    registry.register("btrfs", Box::new(BtrfsFormatter), metadata_btrfs());
    
    // Legacy
    registry.register("ntfs", Box::new(NtfsFormatter), metadata_ntfs());
    registry.register("fat32", Box::new(Fat32Formatter), metadata_fat32());
    
    // Historical (when implemented)
    registry.register("amiga-ofs", Box::new(AmigaOfsFormatter), metadata_amiga_ofs());
}
```

### 2. Dynamic Plugins
Loaded at runtime from external files.

```rust
// Example plugin structure
moses-plugins/
├── official/           # Vetted by Moses team
│   ├── commodore.dll
│   ├── amiga.dll
│   └── atari.dll
├── community/          # Community contributed
│   ├── experimental-fs.dll
│   └── custom-format.dll
└── metadata/          # Plugin descriptions
    ├── commodore.toml
    └── amiga.toml
```

### 3. Script-based Plugins
For simple formatters that just wrap command-line tools.

```toml
# plugins/zfs.toml
[plugin]
name = "zfs"
type = "script"
platforms = ["linux", "freebsd"]

[formatter]
create_command = "zpool create {pool_name} {device}"
format_command = "zfs create {pool_name}/{filesystem}"
verify_command = "zpool status {pool_name}"

[requirements]
tools = ["zpool", "zfs"]
min_version = "2.0.0"
```

## Plugin Development

### Simple Plugin Example

```rust
// my_formatter_plugin/src/lib.rs
use moses_core::{FilesystemFormatter, FormatterPlugin, Device, FormatOptions, MosesError};

pub struct MyCustomFormatter;

#[async_trait]
impl FilesystemFormatter for MyCustomFormatter {
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Implementation
        Ok(())
    }
    
    async fn verify(&self, device: &Device) -> Result<bool, MosesError> {
        // Verify the format succeeded
        Ok(true)
    }
    
    async fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport, MosesError> {
        // Simulate the format
        Ok(SimulationReport::default())
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Windows, Platform::Linux]
    }
}

// Export the plugin
#[no_mangle]
pub extern "C" fn moses_plugin_init() -> Box<dyn FormatterPlugin> {
    Box::new(MyCustomFormatterPlugin)
}
```

### Complex Plugin with External Tools

```rust
// Example: Commodore 1541 formatter
pub struct Commodore1541Formatter {
    tool_path: Option<PathBuf>,
}

impl Commodore1541Formatter {
    async fn find_or_download_tools(&self) -> Result<PathBuf, MosesError> {
        // Check for cc1541 tool
        if let Ok(path) = which::which("cc1541") {
            return Ok(path);
        }
        
        // Download from official source
        self.download_tool("https://github.com/cc1541/cc1541/releases").await
    }
}
```

## Plugin Discovery & Management

### CLI Commands

```bash
# List all available formatters
moses list-formats

# Get detailed info about a formatter
moses format-info ext4

# Install a plugin
moses plugin install amiga-formats

# List installed plugins
moses plugin list

# Update plugins
moses plugin update
```

### Output Example

```
$ moses list-formats

BUILT-IN FORMATTERS:
  Modern:
    ext4     - Fourth Extended Filesystem (Linux)
    btrfs    - B-tree Filesystem (Linux)
    xfs      - XFS Filesystem (Linux)
    
  Legacy:
    ntfs     - NT File System (Windows)
    fat32    - File Allocation Table 32
    exfat    - Extended FAT (cross-platform)
    
  Platform: Windows
    refs     - Resilient File System

INSTALLED PLUGINS:
  Historical:
    amiga-ofs    - Amiga Old File System [Plugin v1.0.0]
    c64-1541     - Commodore 1541 Disk Format [Plugin v1.0.0]
    apple-dos33  - Apple DOS 3.3 [Plugin v1.0.0]
    
  Console:
    ps2-mcfs     - PlayStation 2 Memory Card [Plugin v1.0.0]
    xbox-fatx    - Xbox FATX [Plugin v1.0.0]

AVAILABLE TO INSTALL:
    zfs          - ZFS (requires additional setup)
    hammerfs     - HAMMER Filesystem (DragonFly BSD)
```

## Plugin Capabilities

### 1. Metadata & Discovery

```rust
pub trait FormatterCapabilities {
    // Basic info
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn category(&self) -> FormatterCategory;
    
    // Constraints
    fn min_device_size(&self) -> Option<u64>;
    fn max_device_size(&self) -> Option<u64>;
    fn supported_device_types(&self) -> Vec<DeviceType>;
    
    // Features
    fn supports_labels(&self) -> bool;
    fn max_label_length(&self) -> Option<usize>;
    fn supports_uuid(&self) -> bool;
    fn supports_encryption(&self) -> bool;
    fn supports_compression(&self) -> bool;
    
    // Platform-specific
    fn required_kernel_modules(&self) -> Vec<String>;
    fn required_privileges(&self) -> PrivilegeLevel;
}
```

### 2. Tool Management

```rust
pub trait ToolManager {
    async fn check_tools(&self) -> Result<ToolStatus, MosesError>;
    async fn install_tools(&self) -> Result<(), MosesError>;
    async fn download_tool(&self, url: &str) -> Result<PathBuf, MosesError>;
}

pub struct ToolStatus {
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<PathBuf>,
    pub install_instructions: Option<String>,
}
```

### 3. Format Conversion

```rust
pub trait FormatConverter {
    // Can this formatter convert from another format?
    fn can_convert_from(&self, source_fs: &str) -> bool;
    
    // In-place conversion (rare, but some support it)
    async fn convert_inplace(&self, device: &Device, from: &str) -> Result<(), MosesError>;
    
    // Migration with data preservation
    async fn migrate_with_backup(&self, source: &Device, target: &Device) -> Result<(), MosesError>;
}
```

## Plugin Configuration

### Global Config

```toml
# ~/.moses/config.toml
[plugins]
enabled = true
plugin_dir = "~/.moses/plugins"
auto_update = false
allow_unsigned = false  # Require signed plugins

[plugin_sources]
official = "https://plugins.moses.dev/official"
community = "https://plugins.moses.dev/community"

[plugin_sandbox]
enabled = true           # Run plugins in sandbox
memory_limit = "512MB"
timeout = 300           # seconds
```

### Per-Plugin Config

```toml
# ~/.moses/plugins/amiga.toml
[amiga-ofs]
enabled = true
tool_path = "/usr/local/bin/adf-tools"

[amiga-ofs.defaults]
bootblock = "standard"
international_mode = true
directory_cache = true
```

## Safety & Sandboxing

### 1. Plugin Verification

```rust
pub struct PluginVerifier {
    trusted_keys: Vec<PublicKey>,
}

impl PluginVerifier {
    pub fn verify_signature(&self, plugin: &[u8], signature: &[u8]) -> bool {
        // Verify plugin is signed by trusted key
    }
    
    pub fn check_permissions(&self, manifest: &PluginManifest) -> Result<(), SecurityError> {
        // Ensure plugin only requests necessary permissions
    }
}
```

### 2. Sandboxing Options

- **WASM Plugins**: Run in WASM sandbox with limited syscalls
- **Process Isolation**: Run plugins in separate process with IPC
- **Container**: Run in lightweight container (Linux)
- **Capability Restrictions**: Limit filesystem/network access

### 3. Permission Model

```rust
pub enum PluginPermission {
    ReadDevice,          // Can read device info
    WriteDevice,         // Can format devices
    ExecuteTools,        // Can run external tools
    NetworkAccess,       // Can download tools
    SystemConfig,        // Can modify system config
}
```

## Testing Framework for Plugins

```rust
// Plugin test harness
pub trait PluginTest {
    async fn test_format(&self) -> Result<(), TestError>;
    async fn test_verify(&self) -> Result<(), TestError>;
    async fn test_edge_cases(&self) -> Result<(), TestError>;
}

// Automated testing
#[test]
async fn test_custom_formatter() {
    let formatter = MyCustomFormatter::new();
    let mock_device = MockDevice::new(1024 * 1024 * 100); // 100MB
    
    let options = FormatOptions {
        filesystem_type: "mycustom".to_string(),
        label: Some("TEST".to_string()),
        ..Default::default()
    };
    
    // Test format
    formatter.format(&mock_device, &options).await.unwrap();
    
    // Test verify
    assert!(formatter.verify(&mock_device).await.unwrap());
}
```

## Plugin Distribution

### Official Plugin Repository

```
https://plugins.moses.dev/
├── /api/v1/
│   ├── /search?q=amiga
│   ├── /download/amiga-ofs/1.0.0
│   └── /metadata/amiga-ofs
├── /plugins/
│   ├── amiga-ofs.dll
│   ├── amiga-ofs.dll.sig
│   └── amiga-ofs.toml
└── /docs/
    └── creating-plugins.html
```

### Plugin Package Format

```
my-formatter-1.0.0.moses
├── manifest.toml
├── plugin.dll / plugin.so / plugin.wasm
├── plugin.sig (signature)
├── tools/
│   └── required-tool.exe
├── docs/
│   └── README.md
└── tests/
    └── test_cases.json
```

## Benefits of This Architecture

1. **Simplicity**: One trait to implement for any formatter
2. **Flexibility**: Supports compiled, dynamic, and script-based plugins
3. **Safety**: Sandboxing and verification for untrusted plugins
4. **Discoverability**: Easy to find and install new formatters
5. **Community**: Enables community contributions
6. **Maintenance**: Core team focuses on framework, community handles exotic formats
7. **Testing**: Unified testing framework for all formatters
8. **Documentation**: Consistent docs for all formatters

## Implementation Phases

### Phase 1: Core Registry (Current)
- Implement FormatterRegistry
- Move existing formatters to registry
- Add metadata system

### Phase 2: Script Plugins
- Add TOML-based plugin support
- Implement tool wrapper

### Phase 3: Dynamic Loading
- Add DLL/SO loading support
- Implement plugin verification

### Phase 4: WASM Sandbox
- Add WASM runtime
- Port example plugin to WASM

### Phase 5: Distribution
- Create plugin repository
- Add plugin management commands
- Setup CI/CD for official plugins

## Example: Adding Commodore 1541 Support

1. **Create Plugin**
```rust
// plugins/commodore/src/lib.rs
pub struct C1541Formatter;

impl FilesystemFormatter for C1541Formatter {
    async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
        // Use cc1541 tool or implement natively
    }
}
```

2. **Register Metadata**
```toml
# plugins/commodore/metadata.toml
[formatter]
name = "c64-1541"
aliases = ["commodore", "1541", "d64"]
category = "Historical"
min_size = 174848  # 171KB
max_size = 174848

[requirements]
tools = ["cc1541"]
platforms = ["windows", "linux", "macos"]
```

3. **Test**
```bash
moses plugin test c64-1541
moses format /dev/sdx c64-1541
```

4. **Distribute**
```bash
moses plugin publish c64-1541
```

The plugin is now available to all Moses users!