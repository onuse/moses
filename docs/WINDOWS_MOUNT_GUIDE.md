# Moses Windows Mount Guide - WinFsp Integration

## Overview
Moses Bridge can mount any supported filesystem on Windows using WinFsp (Windows File System Proxy). This allows you to:
- Mount ext4/ext3/ext2 drives as Windows drive letters
- Mount FAT16/FAT32/exFAT/NTFS with custom implementations
- Access Linux filesystems natively in Windows Explorer
- Use any Windows application with mounted filesystems

## Prerequisites

### 1. Install WinFsp
WinFsp is required for filesystem mounting on Windows.

**Download and install from:** https://winfsp.dev/

Or using package managers:
```powershell
# Using Chocolatey
choco install winfsp

# Using Scoop
scoop bucket add extras
scoop install winfsp
```

### 2. Verify Installation
After installation, verify WinFsp is available:
```powershell
# Check if WinFsp service is installed
Get-Service WinFsp
```

## Building Moses with WinFsp Support

Build Moses with Windows mount support enabled:

```powershell
# From project root
cargo build --release --features mount-windows -p moses-cli
```

## Using Moses Mount

### Basic Usage

```powershell
# Mount an ext4 USB drive
.\target\release\moses.exe mount E: M:

# Mount with specific filesystem type
.\target\release\moses.exe mount E: M: --fs-type ext4

# Mount read-only (recommended for data safety)
.\target\release\moses.exe mount E: M: --readonly

# Mount a disk image file
.\target\release\moses.exe mount C:\images\linux.img L: --fs-type ext4
```

### Advanced Examples

```powershell
# Mount a specific partition from a disk
.\target\release\moses.exe mount \\.\PhysicalDrive1 M: --fs-type ext4

# Mount with custom options
.\target\release\moses.exe mount E: M: `
    --fs-type fat32 `
    --readonly `
    --allow-other

# Mount subfolder from a filesystem
.\target\release\moses.exe mount "E:\home\user" H: --fs-type ext4
```

## Supported Filesystems

| Filesystem | Read Support | Write Support | Notes |
|-----------|--------------|---------------|-------|
| ext4      | ✅ Full      | ❌ Not yet    | Most common Linux filesystem |
| ext3      | ✅ Full      | ❌ Not yet    | Legacy Linux filesystem |
| ext2      | ✅ Full      | ❌ Not yet    | Old Linux filesystem |
| NTFS      | ✅ Full      | ⚠️ Limited    | Native Windows filesystem |
| FAT32     | ✅ Full      | ❌ Not yet    | Universal USB filesystem |
| FAT16     | ✅ Full      | ❌ Not yet    | Legacy DOS filesystem |
| exFAT     | ✅ Full      | ❌ Not yet    | Modern USB filesystem |

## Unmounting

### Using Moses
```powershell
.\target\release\moses.exe unmount M:
```

### Using Windows Explorer
- Right-click on the drive in Explorer
- Select "Eject" or "Safely Remove"

### Force unmount (if needed)
```powershell
# List all WinFsp mounts
winfsp-tests-x64.exe list

# Force unmount
net use M: /delete /force
```

## Troubleshooting

### "WinFsp not found" Error
- Ensure WinFsp is installed from https://winfsp.dev/
- Restart your computer after installation
- Check if WinFsp service is running: `Get-Service WinFsp`

### "Access Denied" Error
- Run PowerShell/Command Prompt as Administrator
- Check if the drive letter is already in use
- Ensure the source device is not locked by another process

### "Filesystem not recognized" Error
- Specify filesystem type explicitly with `--fs-type`
- Ensure the device actually contains a filesystem
- Check if the filesystem is corrupted: `chkdsk E: /f`

### Mount appears but shows no files
- The filesystem may be corrupted
- Try mounting read-only: `--readonly`
- Check Moses logs for detailed errors

### Performance Issues
- WinFsp caches filesystem metadata by default
- For better performance with large directories, increase cache timeout
- Consider using SSDs for disk images

## Security Considerations

1. **Always mount untrusted filesystems as read-only**
   ```powershell
   moses mount E: M: --readonly
   ```

2. **Run with minimum required privileges**
   - Admin rights needed for physical disk access
   - Regular user sufficient for image files

3. **Scan mounted drives for malware**
   - Windows Defender automatically scans mounted drives
   - Consider manual scan for sensitive data

## Integration with Windows

### Windows Explorer
- Mounted drives appear as regular drives
- Full Explorer integration (copy, paste, drag-drop)
- Thumbnail generation works normally

### Command Line Tools
```powershell
# PowerShell
Get-ChildItem M:\
Copy-Item M:\file.txt C:\Users\

# Command Prompt
dir M:\
xcopy M:\folder C:\backup\ /E
```

### WSL Integration
Access Moses-mounted drives from WSL:
```bash
# In WSL
ls /mnt/m/
cp /mnt/m/file.txt ~/
```

### Applications
- Any Windows application can access mounted drives
- IDEs can open projects from mounted filesystems
- Media players can play files directly

## Performance Tips

1. **Use image files on fast storage**
   - Place disk images on SSDs for best performance
   - Avoid network drives for image files

2. **Mount with appropriate block size**
   - Larger blocks for sequential access
   - Smaller blocks for random access

3. **Enable caching for read-heavy workloads**
   - WinFsp caches metadata by default
   - Consider increasing cache size for large filesystems

## Examples by Use Case

### Recovering Linux Data
```powershell
# Mount Linux drive from dead dual-boot system
moses mount \\.\PhysicalDrive1 L: --fs-type ext4 --readonly
```

### USB Drive Access
```powershell
# Mount ext4-formatted USB drive
moses mount E: U: --fs-type ext4
```

### Virtual Machine Disks
```powershell
# Mount VMDK/VHD after converting to raw
qemu-img convert linux.vmdk linux.img
moses mount linux.img V: --fs-type ext4
```

### Forensic Analysis
```powershell
# Mount evidence image read-only
moses mount evidence.dd E: --readonly --fs-type ext4
```

## Limitations

1. **Write support is limited**
   - Most filesystems are read-only for data safety
   - NTFS has experimental write support

2. **Some filesystem features not supported**
   - Linux permissions shown but not enforced
   - Symbolic links may not work correctly
   - Extended attributes partially supported

3. **Performance overhead**
   - WinFsp adds some overhead vs native access
   - Large directories may be slower to browse

## Getting Help

1. **Check Moses logs**
   ```powershell
   moses mount E: M: --log-level debug
   ```

2. **WinFsp documentation**
   - https://winfsp.dev/doc/

3. **Report issues**
   - https://github.com/your-org/moses/issues

## Quick Reference Card

```powershell
# List available devices
moses list

# Mount device
moses mount SOURCE TARGET [OPTIONS]

# Common options
--fs-type TYPE     # Specify filesystem (ext4, ntfs, fat32, etc.)
--readonly         # Mount read-only (recommended)
--allow-other      # Allow other users to access

# Unmount
moses unmount TARGET

# Examples
moses mount E: M:                          # Auto-detect filesystem
moses mount E: M: --fs-type ext4           # Specify ext4
moses mount disk.img D: --readonly         # Mount image file
moses unmount M:                           # Unmount drive
```