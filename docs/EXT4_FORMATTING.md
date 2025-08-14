# EXT4 Formatting Implementation Plan

## Overview
Implementing EXT4 formatting across platforms with different approaches for Linux and Windows.

## Linux Implementation (Native)

### Tools Required
- `mkfs.ext4` (part of e2fsprogs, usually pre-installed)
- `sudo` or `pkexec` for privilege escalation

### Implementation Steps
1. **Check tool availability**
   ```rust
   Command::new("which").arg("mkfs.ext4")
   ```

2. **Unmount device if mounted**
   ```rust
   Command::new("umount").arg(device_path)
   ```

3. **Execute format with privilege escalation**
   ```rust
   // Option A: pkexec (GUI-friendly)
   Command::new("pkexec")
       .arg("mkfs.ext4")
       .arg("-F")  // Force (skip confirmation)
       .arg("-L").arg(label)  // Volume label
       .arg(device_path)
   
   // Option B: sudo (if configured)
   Command::new("sudo")
       .arg("-n")  // Non-interactive
       .arg("mkfs.ext4")
       .args(...)
   ```

4. **Progress monitoring**
   - Parse mkfs.ext4 output
   - Use `-p` flag for progress bars
   - Report via IPC to GUI

## Windows Implementation (Via WSL2 or Bundled Tools)

### Strategy 1: WSL2 Integration (Preferred)

#### Prerequisites Check
```rust
// Check if WSL2 is available
Command::new("wsl").arg("--status")

// Check if a distro is installed
Command::new("wsl").arg("-l")
```

#### Format via WSL
```rust
Command::new("wsl")
    .arg("-e")
    .arg("bash")
    .arg("-c")
    .arg(format!("sudo mkfs.ext4 -F -L {} {}", label, wsl_device_path))
```

#### Device Path Translation
- Windows: `\\.\PhysicalDrive1`
- WSL: `/dev/sdb`
- Need mapping between Windows and WSL device names

### Strategy 2: Bundled mke2fs.exe

#### Tools to Bundle
- **Option A**: ext2fsd project tools
  - mke2fs.exe from ext2fsd
  - ~2MB binary size
  - License: GPL

- **Option B**: Custom build
  - Cross-compile e2fsprogs for Windows
  - Using MinGW or MSYS2
  - More control but more maintenance

#### Implementation
```rust
// Use bundled tool
let mke2fs_path = bundle_path.join("mke2fs.exe");
Command::new(mke2fs_path)
    .arg("-t").arg("ext4")
    .arg("-L").arg(label)
    .arg(windows_device_path)
```

### Strategy 3: Hybrid Approach (Recommended)

```rust
async fn format_ext4_windows(device: &str, label: &str) -> Result<()> {
    // Try WSL2 first
    if wsl2_available().await? {
        return format_via_wsl2(device, label).await;
    }
    
    // Fall back to bundled tools
    if bundled_tools_available() {
        return format_via_bundled_tools(device, label).await;
    }
    
    Err("No EXT4 formatting method available")
}
```

## Safety Measures

### Pre-format Checks
1. **Device is unmounted**
2. **Not a system device**
3. **User confirmation received**
4. **Device exists and is writable**
5. **Sufficient permissions**

### Rollback Capability
- Save partition table backup before format
- Provide recovery instructions
- Log all operations

## Progress Reporting

### Linux
```rust
// Parse mkfs.ext4 output
let output = Command::new("mkfs.ext4")
    .arg("-p")  // Progress
    .stdout(Stdio::piped())
    .spawn()?;

// Parse progress from stdout
// "Writing inode tables: 142/256"
```

### Windows
- WSL2: Similar to Linux
- Bundled: May need custom progress calculation

## Error Handling

### Common Errors
- Device busy/mounted
- Insufficient privileges  
- Bad blocks on device
- Device write-protected
- Tool not found

### Recovery
- Provide clear error messages
- Suggest fixes (unmount, run as admin, etc.)
- Log detailed errors for debugging

## Testing Strategy

### Linux
- Test with loop devices: `dd if=/dev/zero of=test.img bs=1M count=100`
- Mount and verify filesystem
- Test permission denial scenarios

### Windows  
- Test with VHD files
- Test WSL2 availability detection
- Test fallback to bundled tools

## Next Steps

1. Implement Linux native formatting ✅
2. Test WSL2 detection on Windows
3. Bundle mke2fs.exe for Windows
4. Implement device path translation
5. Add progress reporting
6. Comprehensive error handling