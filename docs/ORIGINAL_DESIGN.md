# Moses - Cross-Platform Drive Formatting Tool
## Design Document v2.0

## Project Overview
Moses is a cross-platform drive formatting tool designed to provide a zero-hassle experience for users who need to format drives with various filesystems across Windows, macOS, and Linux. The tool addresses the gap in the market for a truly cross-platform formatting solution, particularly for filesystems like EXT4 on Windows where existing solutions are either expensive or unreliable.

## Goals

### Primary Goals:
- Format drives with any filesystem on any supported platform
- Zero installation hassle (fat binaries with bundled dependencies)
- Single executable per platform with consistent behavior
- Intuitive GUI for non-technical users
- Reliable and safe disk operations
- **Minimal privilege escalation** - only elevate when absolutely necessary

### Secondary Goals:
- CLI version alongside GUI for automation
- Extensible plugin architecture for new filesystems
- Support for legacy/retro filesystems (Amiga, Atari, etc.)
- Open source with permissive licensing
- Community contributions for specialized filesystem support

## Technology Stack

**Core Language:** Rust
- Memory safety for low-level disk operations
- Excellent cross-platform support
- Superior C library interop without GC interference
- Mature systems programming ecosystem

**GUI Framework:** Tauri
- Native performance with web-based UI
- Small bundle sizes compared to Electron
- Built-in security model
- Familiar web technologies for UI development

**Target Platforms:**
- Windows 10 1903+ (for consistent UAC and storage APIs)
- macOS 10.15+ Catalina (for notarization and security model)
- Linux with Kernel 3.10+ and glibc 2.17+

## Architecture Overview

### Process Separation Architecture
The application is split into three distinct processes for security and flexibility:

```
moses/
├── core/                 # Platform-agnostic business logic
│   ├── device.rs        # Device enumeration and management
│   ├── filesystem.rs    # Filesystem trait definitions
│   ├── format.rs        # Core formatting operations
│   ├── error.rs         # Error handling and types
│   └── registry.rs      # Filesystem formatter registry
├── daemon/              # Privileged formatting service
│   ├── service.rs       # IPC service implementation
│   ├── formatter.rs     # Actual formatting execution
│   └── security.rs      # Permission validation
├── platform/            # Platform-specific implementations
│   ├── windows/
│   │   ├── device.rs    # Windows device enumeration (WinAPI)
│   │   ├── privilege.rs # UAC handling (delayed)
│   │   └── filesystem.rs # Windows-specific formatters
│   ├── macos/
│   │   ├── device.rs    # IOKit device management
│   │   ├── privilege.rs # Admin privilege escalation (delayed)
│   │   └── filesystem.rs # macOS-specific formatters
│   └── linux/
│       ├── device.rs    # Linux device enumeration
│       ├── privilege.rs # sudo/polkit handling (delayed)
│       └── filesystem.rs # Linux-specific formatters
├── formatters/          # Filesystem-specific implementations
│   ├── ext4.rs
│   ├── ntfs.rs
│   ├── fat32.rs
│   ├── exfat.rs
│   └── plugins/        # Future plugin system
├── ui/                 # Tauri frontend
│   ├── src/
│   │   ├── App.vue     # Main application UI
│   │   ├── components/ # Vue components
│   │   └── api.js      # Tauri API bindings
│   └── dist/           # Built frontend assets
├── cli/                # Command-line interface
│   ├── main.rs         # CLI entry point
│   └── commands.rs     # CLI command implementations
├── bundle/             # Fat binary resources
│   ├── windows/
│   │   ├── ext2fsd/    # EXT4 tools for Windows (with WSL2 fallback)
│   │   └── ntfs-3g/    # Alternative NTFS tools
│   ├── macos/
│   │   └── osxfuse/    # FUSE-based filesystem tools
│   └── linux/
│       └── e2fsprogs/  # EXT filesystem utilities
└── build/              # Build scripts and CI/CD
    ├── windows.yml
    ├── macos.yml
    └── linux.yml
```

## Core Components

### Device Management
```rust
pub trait DeviceManager: Send + Sync {
    fn enumerate_devices(&self) -> Result<Vec<Device>>;
    fn get_device_info(&self, device: &Device) -> Result<DeviceInfo>;
    fn is_safe_to_format(&self, device: &Device) -> Result<bool>;
    fn check_permissions(&self, device: &Device) -> Result<PermissionLevel>;
}

pub struct Device {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub device_type: DeviceType,
    pub mount_points: Vec<PathBuf>,
}

pub enum PermissionLevel {
    ReadOnly,      // Can enumerate and inspect
    Simulate,      // Can do dry-runs
    FullAccess,    // Can format (requires elevation)
}
```

### Filesystem Plugin System
```rust
pub trait FilesystemFormatter: Send + Sync {
    fn name(&self) -> &'static str;
    fn supported_platforms(&self) -> Vec<Platform>;
    fn can_format(&self, device: &Device) -> bool;
    fn requires_external_tools(&self) -> bool;
    fn bundled_tools(&self) -> Vec<&'static str>;
    fn format(&self, device: &Device, options: &FormatOptions) -> Result<()>;
    fn validate_options(&self, options: &FormatOptions) -> Result<()>;
    fn dry_run(&self, device: &Device, options: &FormatOptions) -> Result<SimulationReport>;
}

pub struct FormatterRegistry {
    formatters: HashMap<String, Box<dyn FilesystemFormatter>>,
}
```

### IPC Communication
```rust
pub enum DaemonCommand {
    CheckDevice(Device),
    SimulateFormat(Device, FormatOptions),
    ExecuteFormat(Device, FormatOptions),
    CancelOperation(OperationId),
}

pub enum DaemonResponse {
    DeviceStatus(DeviceInfo),
    SimulationComplete(SimulationReport),
    FormatProgress(f32),
    FormatComplete(Result<()>),
}
```

### Error Handling
```rust
#[derive(Debug, thiserror::Error)]
pub enum MosesError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    
    #[error("Insufficient privileges: {0}")]
    InsufficientPrivileges(String),
    
    #[error("Formatting failed: {0}")]
    FormatError(String),
    
    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),
    
    #[error("External tool missing: {0}")]
    ExternalToolMissing(String),
    
    #[error("Operation cancelled by user")]
    UserCancelled,
    
    #[error("Simulation mode: {0}")]
    SimulationOnly(String),
}
```

## Platform-Specific Considerations

### Windows
- **Device Enumeration:** Use SetupDiGetClassDevs and WMI for disk discovery
- **Privilege Escalation:** Delayed UAC - only when executing format, not for browsing
- **EXT4 Support:** Primary: Bundle portable ext4 tools; Fallback: WSL2 integration if available
- **Safety:** Prevent formatting of system drives and mounted volumes

### macOS
- **Device Enumeration:** IOKit framework for hardware discovery
- **Privilege Escalation:** Authorization Services only when needed
- **Filesystem Tools:** Bundle OSXFUSE and platform-specific formatters
- **Security:** Handle System Integrity Protection and notarization

### Linux
- **Device Enumeration:** Parse /proc/partitions and use lsblk
- **Privilege Escalation:** Prefer polkit over sudo for better UX
- **Native Support:** Most filesystems supported natively
- **Distribution Compatibility:** Static linking or dependency bundling

## Development Phases

### Phase 1: Core Infrastructure (MVP)
- Basic device enumeration on Windows (read-only mode)
- Simple GUI with device selection
- Dry-run/simulation mode (no privileges needed)
- EXT4 formatting capability on Windows
- Delayed privilege escalation
- Basic error handling and logging

### Phase 2: Cross-Platform Foundation
- Complete platform abstraction layer
- Device enumeration on all platforms
- Refined privilege escalation handling
- Core filesystem support (EXT4, NTFS, FAT32, exFAT)
- CLI version for automation

### Phase 3: Enhanced Features
- Advanced formatting options (cluster size, labels, etc.)
- Progress reporting and cancellation
- Device safety checks and warnings
- Comprehensive error reporting
- Process separation fully implemented

### Phase 4: Extensibility
- Plugin system for custom formatters
- Documentation for formatter development
- Community contribution guidelines
- Package manager integration

### Phase 5: Legacy Support
- Retro filesystem support (Amiga AFS, Atari TOS, etc.)
- Specialized tools integration
- Historical filesystem preservation features

## User Experience Design

### Core User Flow
1. Launch Moses (no admin required initially)
2. Browse and select target drive
3. Choose filesystem type from supported options
4. Configure formatting options (optional)
5. Run simulation/dry-run (still no admin required)
6. Review simulation results
7. Confirm operation with safety warnings
8. **Only now:** Request admin privileges if proceeding
9. Monitor progress with cancel option
10. Receive completion confirmation

### Safety Features
- Clear visual distinction between system and data drives
- Multiple confirmation dialogs for destructive operations
- Automatic detection of mounted filesystems
- Warning for drives with existing data
- **Default to dry-run mode** - explicit action needed for real formatting
- Simulation report before any destructive operation

## Security Considerations

### Privilege Minimization
- GUI runs entirely unprivileged
- Only formatter daemon requests elevation
- Elevation only for actual format operations
- Read-only operations never require privileges
- Clear indication when privileges will be requested

### Input Validation
- Sanitize all user inputs and device paths
- Validate IPC messages between processes
- Formatter daemon validates all requests independently

### Safe Defaults
- Conservative formatting options by default
- Dry-run mode as default action
- Explicit user action for destructive operations

### Audit Trail
- Log all operations for debugging and security
- Separate logs for privileged operations
- User-readable operation history

### Code Signing
- Proper signing for all platform binaries
- Budget consideration: ~$100-300/year per platform

## Testing Strategy

### Unit Testing
- Platform abstraction layer testing
- Filesystem formatter validation
- Error handling verification
- Mock device management for CI/CD
- IPC communication testing

### Integration Testing
- Cross-platform device enumeration
- End-to-end formatting workflows
- Privilege escalation testing
- Bundle integrity verification
- Process separation validation

### Manual Testing
- Real hardware testing on all platforms
- Various filesystem combinations
- Edge cases and error conditions
- User experience validation
- Dry-run vs actual format comparison

### Testing Matrix Focus
- Start with narrow MVP: EXT4 on Windows only
- Expand systematically to avoid combinatorial explosion
- Prioritize most common use cases

## Build and Distribution

### Build System
- Cargo for Rust compilation
- Tauri CLI for app packaging
- GitHub Actions for CI/CD
- Cross-compilation for all targets
- Separate builds for GUI and CLI

### Distribution Strategy
- Single executable per platform (fat binaries)
- Separate lightweight CLI binary
- GitHub Releases for version distribution
- Automatic update mechanism (future)
- Package manager integration (future)

## Success Metrics

- **Primary:** Successf
