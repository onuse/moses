# Moses Mount Alternatives to WinFsp

## Option 1: Network Drive Approach (No Kernel Driver!)

Instead of mounting as a local drive, Moses could expose filesystems as network shares:

```
Moses → Built-in SMB/WebDAV Server → Windows Maps as Network Drive
```

### Implementation Sketch

```rust
// Moses runs a local SMB server
moses serve /dev/sdb1 --port 44445

// Windows maps it
net use Z: \\localhost:44445\ext4
```

### Pros
- ✅ No kernel driver needed
- ✅ No admin rights required
- ✅ Works on all Windows versions
- ✅ No signing certificates
- ✅ We control everything

### Cons
- ❌ Shows as network drive, not local
- ❌ Some applications don't like network paths
- ❌ Slightly higher latency

## Option 2: Shell Namespace Extension

Create a Windows Explorer extension that makes Moses filesystems appear in "This PC":

```
This PC
├── C: (Windows)
├── D: (Data)
└── 📁 Moses Filesystems
    ├── ext4 on /dev/sdb1
    └── btrfs on /dev/sdc1
```

### Pros
- ✅ Native Explorer integration
- ✅ No kernel driver
- ✅ Can provide custom UI

### Cons
- ❌ Not a real drive letter
- ❌ Complex COM programming
- ❌ Only works in Explorer

## Option 3: Projected File System (ProjFS)

Windows 10 1809+ has ProjFS API (what OneDrive uses):

```rust
use windows::Win32::Storage::ProjectedFileSystem::*;

// Create virtual filesystem backed by Moses
moses mount-projfs /dev/sdb1 C:\MountedExt4
```

### Pros
- ✅ Official Microsoft API
- ✅ No kernel driver needed
- ✅ Used by Git Virtual File System

### Cons
- ❌ Not a drive letter (folder only)
- ❌ Windows 10 1809+ only
- ❌ Designed for different use case

## Option 4: SubstituteDriver + Junction Points

Clever workaround using Windows built-in features:

```rust
// 1. Moses serves files via local API
let server = MosesFileServer::new(device);

// 2. Create junction point
mklink /J C:\Temp\MosesMount \\?\pipe\moses\ext4

// 3. Create virtual drive
subst M: C:\Temp\MosesMount
```

### Pros
- ✅ Real drive letter
- ✅ No special drivers
- ✅ Uses Windows built-ins

### Cons
- ❌ Hacky approach
- ❌ May confuse some programs
- ❌ Performance limitations

## Option 5: Pure User-Mode Mini-Filter (Theoretical)

Build a user-mode filesystem using undocumented Windows APIs:

```rust
// Hook into Windows file APIs at user level
// Intercept file operations for our drive letters
// Redirect to Moses engine
```

### Pros
- ✅ Would work like WinFsp
- ✅ No kernel component

### Cons
- ❌ Requires undocumented APIs
- ❌ Could break with Windows updates
- ❌ Essentially recreating WinFsp
- ❌ Years of development

## Recommendation

**For production: Use WinFsp**
- It's free, tested, and signed
- Bundle the installer or auto-download
- Saves years of development

**For experimenting: Network Drive Approach**
- Can implement today
- Good fallback when WinFsp unavailable
- "Moses serve" could be useful anyway

**For future: ProjFS API**
- Microsoft's direction for virtual filesystems
- Could complement WinFsp option

## Implementation Priority

1. **Keep WinFsp** as primary (it's free!)
2. **Add SMB/WebDAV server** as fallback
3. **Consider ProjFS** for Windows 10+ enhancement
4. **Skip kernel driver** development (not worth it)

## Quick SMB Server Implementation

```rust
// Using existing crates
use smb3_rust::SmbServer;

impl MosesNetworkMount {
    pub fn serve(device: Device, port: u16) -> Result<(), Error> {
        let ops = create_ops(device)?;
        let server = SmbServer::new(port);
        
        server.on_read(|path, offset, size| {
            ops.read(path, offset, size)
        });
        
        server.on_list(|path| {
            ops.readdir(path)
        });
        
        server.start()?;
        println!("Mount with: net use Z: \\\\localhost:{}", port);
        Ok(())
    }
}
```

This gives users options without requiring WinFsp!