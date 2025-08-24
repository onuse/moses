# Moses Bridge Implementation Roadmap

## Vision
Transform Moses from a formatting tool into a universal filesystem translator that makes filesystem incompatibility obsolete.

## Core Principle
**"If it has a filesystem, Moses can read it"** - From modern NVMe drives to 1980s Amiga disks, Moses provides universal data access.

## The Killer Feature
Not filesystem conversion, but **universal filesystem access**. Moses lets any OS read any filesystem natively through standard OS APIs.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     User Applications                        │
├─────────────────────────────────────────────────────────────┤
│                     Moses CLI/GUI                            │
├─────────────────────────────────────────────────────────────┤
│                     Moses Bridge Layer                       │
│  ┌──────────────┬──────────────┬──────────────┐            │
│  │   Mount      │   Serve      │   Transfer   │            │
│  │  (Native)    │  (Network)   │   (Direct)   │            │
│  └──────────────┴──────────────┴──────────────┘            │
├─────────────────────────────────────────────────────────────┤
│                 Filesystem Trait Layer                       │
│  ┌──────┬──────┬──────┬──────┬──────┬──────┐              │
│  │ ext4 │ NTFS │ FAT32│ APFS │btrfs │ ZFS  │              │
│  └──────┴──────┴──────┴──────┴──────┴──────┘              │
├─────────────────────────────────────────────────────────────┤
│                   Device Access Layer                        │
└─────────────────────────────────────────────────────────────┘
```

## Phase 1: Core Filesystem Trait System
**Goal**: Establish the foundational trait architecture that all filesystems will implement.

### 1.1 Define Core Traits (SYNC - Matching Existing Code!)
Create `moses-core/src/filesystem_ops.rs`:

```rust
/// Unified filesystem operations trait - SYNC to match our existing readers
pub trait FilesystemOps: Send + Sync {
    // Detection and info
    fn detect(device: &Device) -> Result<bool, MosesError> where Self: Sized;
    fn info(&self) -> FilesystemInfo;
    
    // Core read operations (matching existing ExtReader/NtfsReader methods!)
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError>;
    fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError>;
    fn stat(&mut self, path: &str) -> Result<FileMetadata, MosesError>;
    
    // Write operations (can return NotImplemented initially)
    fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), MosesError> {
        Err(MosesError::NotImplemented("Write not supported"))
    }
    fn create_directory(&mut self, path: &str) -> Result<(), MosesError> {
        Err(MosesError::NotImplemented("Write not supported"))
    }
    fn delete(&mut self, path: &str) -> Result<(), MosesError> {
        Err(MosesError::NotImplemented("Write not supported"))
    }
    
    // Format operation (optional)
    fn format(&mut self, options: &FormatOptions) -> Result<(), MosesError> {
        Err(MosesError::NotImplemented("Format not supported"))
    }
}
```

### 1.2 Universal Filesystem Registry
Create `moses-filesystems/src/registry.rs`:

```rust
pub struct FilesystemRegistry {
    filesystems: Vec<Box<dyn FilesystemDetector>>,
}

impl FilesystemRegistry {
    pub fn new() -> Self {
        Self {
            filesystems: vec![
                // Modern filesystems
                Box::new(Ext4Detector),     // Already have ExtReader!
                Box::new(NtfsDetector),     // Already have NtfsReader!
                Box::new(Fat32Detector),    // Already have Fat32Reader!
                Box::new(ExfatDetector),    // Already have ExFatReader!
                Box::new(ApfsDetector),     // Future
                Box::new(BtrfsDetector),    // Future
                Box::new(ZfsDetector),      // Future
                
                // Legacy support (future additions)
                Box::new(AmigaFFSDetector), // Amiga Fast File System
                Box::new(BefsDetector),     // BeOS filesystem
                Box::new(Commodore1541Detector), // C64 disk images
                // ... add ANY filesystem that ever existed!
            ],
        }
    }
    
    pub fn detect(&self, device: &Device) -> Result<Box<dyn FilesystemOps>, MosesError> {
        for detector in &self.filesystems {
            if detector.detect(device)? {
                return detector.create_ops(device);
            }
        }
        Err(MosesError::UnknownFilesystem)
    }
}
```

### 1.3 Adapt Existing Readers (Minimal Work!)
Your existing readers already have the right methods:

```rust
// moses-filesystems/src/ext4/ops.rs
use crate::ext4_native::reader::ExtReader;

pub struct Ext4Ops {
    reader: ExtReader,
}

impl Ext4Ops {
    pub fn new(device: Device) -> Result<Self, MosesError> {
        Ok(Self {
            reader: ExtReader::new(device)?
        })
    }
}

impl FilesystemOps for Ext4Ops {
    fn detect(device: &Device) -> Result<bool, MosesError> {
        // ExtReader::new() already validates ext2/3/4!
        ExtReader::new(device.clone()).map(|_| true).or(Ok(false))
    }
    
    fn list_directory(&mut self, path: &str) -> Result<Vec<FileEntry>, MosesError> {
        // Your ExtReader already has read_directory()!
        let entries = self.reader.read_directory(path)?;
        // Just map the types - minimal work
        Ok(entries.into_iter().map(convert_entry).collect())
    }
    
    fn read_file(&mut self, path: &str) -> Result<Vec<u8>, MosesError> {
        self.reader.read_file(path) // Already perfect!
    }
    
    fn stat(&mut self, path: &str) -> Result<FileMetadata, MosesError> {
        let meta = self.reader.stat(path)?; // Already implemented!
        Ok(convert_metadata(meta))
    }
}
```

## Phase 2: Read-Only Implementation
**Goal**: Implement read operations for each filesystem.

### 2.1 ext4 Reader
- [ ] Read superblock and block groups
- [ ] Inode traversal
- [ ] Directory entry parsing
- [ ] Extent tree navigation
- [ ] File content reading

### 2.2 NTFS Reader
- [ ] Read MFT (Master File Table)
- [ ] Parse file records
- [ ] Handle resident and non-resident attributes
- [ ] Data run decoding
- [ ] Directory index (B-tree) traversal

### 2.3 FAT32 Reader
- [ ] FAT table parsing
- [ ] Cluster chain following
- [ ] Directory entry parsing
- [ ] Long filename support

### 2.4 exFAT Reader
- [ ] Allocation bitmap reading
- [ ] File entry parsing
- [ ] Stream extension handling

## Phase 3: Platform Mount Implementation
**Goal**: Native mounting on each platform.

### 3.1 Windows (WinFsp) - PRIORITY IMPLEMENTATION

#### What WinFsp Actually Does
WinFsp is a **Windows kernel filesystem driver** that lets userspace programs implement filesystems. It's the Windows equivalent of FUSE for Linux. It translates between Windows filesystem APIs and your code:

```
Windows App tries to read M:\file.txt
    ↓
Windows Kernel routes to WinFsp driver
    ↓
WinFsp calls YOUR callbacks (read, readdir, etc.)
    ↓
Your ExtReader reads actual ext4 structures from E:
    ↓
Data flows back: ExtReader → WinFsp → Windows → App
```

#### Implementation
Create `moses-mount/src/windows.rs`:

```rust
use winfsp::*;

pub struct MosesWinFspAdapter {
    ops: Box<dyn FilesystemOps>,  // Your filesystem reader
}

// WinFsp callbacks are SYNC - perfect match for our sync readers!
impl WinFspFileSystem for MosesWinFspAdapter {
    fn read(&mut self, path: &str, buffer: &mut [u8], offset: u64) -> NTSTATUS {
        match self.ops.read_file(path) {
            Ok(data) => {
                let end = (offset as usize + buffer.len()).min(data.len());
                buffer.copy_from_slice(&data[offset as usize..end]);
                STATUS_SUCCESS
            }
            Err(_) => STATUS_FILE_NOT_FOUND
        }
    }
    
    fn readdir(&mut self, path: &str, add_entry: impl FnMut(&FileInfo)) -> NTSTATUS {
        match self.ops.list_directory(path) {
            Ok(entries) => {
                for entry in entries {
                    add_entry(&FileInfo::from(entry));
                }
                STATUS_SUCCESS
            }
            Err(_) => STATUS_FILE_NOT_FOUND
        }
    }
}

pub fn mount(source: &str, target: &str) -> Result<MountHandle, MosesError> {
    // 1. Detect what filesystem is on E:
    let device = Device::from_path(source)?;
    let registry = FilesystemRegistry::new();
    let ops = registry.detect(&device)?;
    println!("🔍 Detected: {}", ops.info().name);
    
    // 2. Wrap in WinFsp adapter
    let adapter = MosesWinFspAdapter { ops };
    
    // 3. Mount as Windows drive letter
    let fs = FileSystem::create(adapter, target, Default::default())?;
    fs.start()?;
    
    println!("✅ {} mounted as {} drive", source, target);
    Ok(MountHandle(fs))
}
```

**Usage**: `moses mount E: M:` - Your ext4/NTFS/Amiga/whatever drive is now M: in Windows!

### 3.2 Linux (FUSE)
Create `moses-mount/src/linux.rs`:

```rust
use fuser::{Filesystem as FuseFS, Request, ReplyDirectory};

pub struct FuseMount {
    filesystem: Box<dyn Filesystem>,
}

impl FuseFS for FuseMount {
    fn readdir(&mut self, _req: &Request, ino: u64, reply: ReplyDirectory) {
        let entries = self.filesystem.read_dir(ino).await.unwrap();
        for entry in entries {
            reply.add(entry.inode, entry.offset, entry.kind, &entry.name);
        }
        reply.ok();
    }
    
    // ... other FUSE operations
}
```

### 3.3 macOS (macFUSE)
Similar to Linux but using macFUSE specifics.

## Phase 4: Network Protocol Servers
**Goal**: Serve filesystems over standard network protocols.

### 4.1 SMB/CIFS Server
Create `moses-bridge/src/servers/smb.rs`:
- Implement SMB2/3 protocol
- Map filesystem operations to SMB commands
- Handle Windows authentication

### 4.2 NFS Server
Create `moses-bridge/src/servers/nfs.rs`:
- Implement NFSv3 (simpler) initially
- Map filesystem operations to NFS procedures
- Handle file handles and exports

### 4.3 WebDAV Server
Create `moses-bridge/src/servers/webdav.rs`:
- HTTP-based file access
- Works through firewalls
- Browser accessible

### 4.4 REST API
Create `moses-bridge/src/servers/rest.rs`:
- Simple JSON API
- For web UI and mobile apps

## Phase 5: Write Support
**Goal**: Add write operations (carefully!).

### 5.1 Safety First
- [ ] Implement transaction logging
- [ ] Add rollback capability
- [ ] Extensive testing framework
- [ ] Dry-run mode for all operations

### 5.2 Incremental Implementation
1. Start with creating new files
2. Add file content modification
3. Add directory operations
4. Add metadata changes
5. Finally, structural changes

## Phase 6: Advanced Features
**Goal**: Features that make Moses indispensable.

### 6.1 Caching Layer
- Read cache for performance
- Write buffering
- Metadata caching

### 6.2 Forensic Mode
- Deleted file recovery
- Timeline reconstruction
- Hidden data detection

### 6.3 Transfer Optimization
- Deduplication
- Compression
- Incremental sync

## Implementation Priority Order

### Milestone 1: "The Killer Demo" (MVP) 🎯
1. ✅ Core trait definition
2. ⬜ ext4 read-only implementation (minimal)
3. ⬜ **WinFsp mount implementation** (THE KILLER FEATURE)
4. ⬜ Simple CLI (`moses mount E: M:`)

**Demo**: "Watch this - I'll mount this Linux drive as M: on Windows!"
- Plug in ext4 drive
- `moses mount E: M:`
- Open M: in Windows Explorer
- 🤯 Mind blown

### Milestone 2: "The Product"
1. ⬜ Complete ext4 read implementation
2. ⬜ NTFS read-only
3. ⬜ FAT32/exFAT read-only  
4. ⬜ FUSE mount (Linux/macOS)
5. ⬜ Basic SMB server (fallback option)
6. ⬜ Polished CLI with proper error handling

**Release**: First public version that "just works"

### Milestone 3: "The Platform"
1. ⬜ Write support (ext4 first)
2. ⬜ All protocol servers
3. ⬜ GUI application
4. ⬜ System tray integration

**Vision**: Complete filesystem translation platform

## File Structure Evolution

```
moses/
├── moses-core/                 # Core traits and types
│   └── src/
│       ├── filesystem.rs       # Filesystem trait
│       ├── device.rs          # Device abstraction
│       └── error.rs           # Error types
│
├── moses-filesystems/          # Filesystem implementations
│   └── src/
│       ├── registry.rs        # Filesystem registry
│       ├── ext4/
│       │   ├── mod.rs         # Trait implementation
│       │   ├── reader.rs      # Read operations
│       │   ├── writer.rs      # Write operations
│       │   ├── structures.rs  # ext4 structures
│       │   └── formatter.rs   # Format operations
│       ├── ntfs/
│       ├── fat32/
│       └── exfat/
│
├── moses-mount/                # Platform-specific mounting
│   └── src/
│       ├── lib.rs             # Platform abstraction
│       ├── windows.rs         # WinFsp implementation
│       ├── linux.rs           # FUSE implementation
│       └── macos.rs           # macFUSE implementation
│
├── moses-bridge/               # Protocol servers
│   └── src/
│       ├── lib.rs             # Bridge orchestration
│       ├── servers/
│       │   ├── smb.rs         # SMB/CIFS server
│       │   ├── nfs.rs         # NFS server
│       │   ├── webdav.rs      # WebDAV server
│       │   └── rest.rs        # REST API
│       └── cache.rs           # Caching layer
│
├── moses-cli/                  # Command-line interface
│   └── src/
│       ├── main.rs
│       └── commands/
│           ├── serve.rs       # Bridge commands
│           ├── mount.rs       # Mount commands
│           ├── format.rs      # Format commands
│           └── transfer.rs    # Copy/sync commands
│
└── moses-gui/                  # GUI application
    └── src/
        └── main.rs

```

## Testing Strategy

### Unit Tests
- Each filesystem operation
- Edge cases and error conditions
- Cross-platform compatibility

### Integration Tests
- Mount and access files
- Transfer between filesystems
- Network protocol compliance

### Stress Tests
- Large files (>4GB)
- Many files (>100k)
- Concurrent access
- Network interruption

### Compatibility Tests
- Windows 7/8/10/11
- Ubuntu/Debian/Fedora/Arch
- macOS 10.15+
- Various filesystem versions

## Success Metrics

1. **Performance**: Read speed within 10% of native
2. **Compatibility**: 100% of normal files accessible
3. **Reliability**: Zero data corruption
4. **Usability**: One command to bridge any filesystem

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Data corruption | Read-only first, extensive testing |
| Performance issues | Caching, async I/O, profiling |
| Platform differences | Abstraction layer, CI/CD per platform |
| Filesystem complexity | Start with common features, expand gradually |

## FAQ

**Q: Why not just use existing tools?**
A: No single tool handles all filesystems on all platforms. Moses provides a unified solution.

**Q: Is this safe for production data?**
A: Read-only operations are safe. Write support will be extensively tested and optional.

**Q: What about encrypted filesystems?**
A: Future feature - would require key management infrastructure.

**Q: Can this replace traditional drivers?**
A: For many use cases, yes. Native drivers still better for OS boot drives.

## Next Steps - Ready to Implement!

### Week 1: Core Infrastructure
1. [ ] Create `moses-core/src/filesystem_ops.rs` with sync trait
2. [ ] Create `moses-filesystems` crate structure
3. [ ] Wrap ExtReader in FilesystemOps trait (1-2 hours)
   - Your ExtReader already has everything needed!
   - Just needs thin adapter layer

### Week 2: WinFsp Integration (The Real Work)
4. [ ] Research WinFsp Rust bindings options:
   - Option A: Use `winfsp-rs` if it exists
   - Option B: Create minimal FFI bindings
   - Option C: Use `windows-rs` for COM interop
5. [ ] Implement MosesWinFspAdapter
   - Map FilesystemOps to WinFsp callbacks
   - Handle path translation (Windows → Unix style)
6. [ ] Create mount/unmount commands

### Week 3: MVP Demo
7. [ ] Build `moses mount E: M:` command
8. [ ] Test with real ext4 drive
9. [ ] **DEMO**: Mount Linux drive on Windows!
10. [ ] Share video: "Look ma, no drivers!"

### Week 4: Polish & Expand
11. [ ] Add NTFS, FAT32, exFAT (you already have readers!)
12. [ ] Better error handling
13. [ ] Progress indicators
14. [ ] System tray app for Windows

### The Beautiful Part
- **ExtReader**: ✅ Already done (90% of the work!)
- **NtfsReader**: ✅ Already done
- **Fat32Reader**: ✅ Already done
- **ExFatReader**: ✅ Already done
- Just need: **Trait wrapper + WinFsp adapter**

## Contributing

This is an ambitious project that needs contributors! Key areas:
- Filesystem implementations (especially APFS, btrfs, ZFS)
- Network protocol servers
- Platform-specific mounting
- Testing and documentation

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Resources

### Filesystem Documentation
- [ext4 Documentation](https://ext4.wiki.kernel.org/)
- [NTFS Documentation](https://docs.microsoft.com/en-us/windows/win32/fileio/ntfs-technical-reference)
- [FAT32 Specification](https://www.win.tue.nl/~aeb/linux/fs/fat/fat-1.html)

### Platform APIs
- [WinFsp Documentation](https://winfsp.dev/doc/)
- [FUSE Documentation](https://libfuse.github.io/doxygen/)
- [macFUSE Documentation](https://osxfuse.github.io/)

### Network Protocols
- [SMB/CIFS Protocol](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-smb2/)
- [NFS Protocol](https://tools.ietf.org/html/rfc1813)
- [WebDAV Protocol](https://tools.ietf.org/html/rfc4918)

---

*"Making filesystems invisible since 2025"*