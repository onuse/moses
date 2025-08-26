# Moses Mount - Universal Filesystem Access

## Overview

Moses Mount allows you to mount **any** filesystem on **any** platform. Read ext4 on Windows, NTFS on Linux, or even obscure filesystems from the 1980s - if Moses can read it, Moses can mount it!

## Current Status

### ✅ Implemented
- **Ext4/Ext3/Ext2 Reader** - Fully functional, ready for mounting
- **WinFsp Integration** - Windows mounting support via WinFsp
- **FUSE Integration** - Linux/macOS mounting support via fuser
- **FilesystemOps Trait** - Universal interface for all filesystems
- **CLI Mount Command** - Cross-platform `moses mount` syntax
- **Test Scripts** - Ready-to-use scripts for all platforms

### 🚧 In Progress
- NTFS reader completion
- FAT32/exFAT reader completion
- Write support for ext4

## Quick Start

### Windows (with ext4 drive)

1. **Install WinFsp** (Windows FUSE equivalent)
   ```
   Download from: http://www.secfs.net/winfsp/
   ```

2. **Build Moses with mount support**
   ```powershell
   cargo build --package moses-cli --features mount-windows --release
   ```

3. **Mount an ext4 drive**
   ```powershell
   # Run as administrator
   moses mount E: M:
   ```

4. **Access your Linux files!**
   - Open `M:` in Windows Explorer
   - Browse, copy, and read ext4 files natively
   - All Windows applications can access the files

5. **Unmount when done**
   ```powershell
   moses unmount M:
   ```

### Linux

1. **Install FUSE**
   ```bash
   # Ubuntu/Debian
   sudo apt-get install libfuse-dev fuse
   
   # Fedora/RHEL
   sudo dnf install fuse fuse-devel
   
   # Arch
   sudo pacman -S fuse2
   ```

2. **Build Moses with FUSE support**
   ```bash
   cargo build --package moses-cli --features mount-unix --release
   ```

3. **Mount a filesystem**
   ```bash
   # Run the test script
   sudo ./scripts/linux/test-mount.sh -s /dev/sdb1 -m /mnt/ext4 -b
   
   # Or manually
   sudo moses mount /dev/sdb1 /mnt/ext4 --readonly
   ```

4. **Access your files!**
   ```bash
   ls /mnt/ext4
   cp /mnt/ext4/important.txt ~/
   ```

5. **Unmount when done**
   ```bash
   sudo moses unmount /mnt/ext4
   # Or
   sudo fusermount -u /mnt/ext4
   ```

### macOS

1. **Install macFUSE**
   ```bash
   # Download from https://osxfuse.github.io/
   # Or via Homebrew
   brew install --cask macfuse
   ```

2. **Build Moses with FUSE support**
   ```bash
   cargo build --package moses-cli --features mount-unix --release
   ```

3. **Mount a filesystem**
   ```bash
   # Run the test script
   sudo ./scripts/macos/test-mount.sh -s /dev/disk2s1 -m /Volumes/ext4 -b
   
   # Or manually
   sudo moses mount /dev/disk2s1 /Volumes/ext4 --readonly
   ```

4. **Access in Finder or Terminal**
   ```bash
   open /Volumes/ext4
   ls /Volumes/ext4
   ```

5. **Unmount**
   ```bash
   sudo moses unmount /Volumes/ext4
   # Or use Finder's eject button
   ```

## Architecture

```
Your Application (Explorer, VS Code, etc.)
           ↓
    Windows File API
           ↓
    WinFsp (or FUSE)
           ↓
    Moses Bridge Layer
           ↓
    FilesystemOps Trait
           ↓
    Filesystem Reader (ext4, ntfs, etc.)
           ↓
    Raw Device Access
```

## How It Works

1. **Device Detection**: Moses identifies the filesystem type on the device
2. **Reader Selection**: The appropriate reader (ExtReader, NtfsReader, etc.) is instantiated
3. **Bridge Creation**: FilesystemOps wrapper provides a uniform interface
4. **OS Integration**: WinFsp (Windows) or FUSE (Linux/macOS) makes it appear as a native filesystem
5. **Transparent Access**: Applications see it as a regular mounted drive

## Supported Filesystems

### Ready Now
- ✅ **ext4** - Modern Linux filesystem
- ✅ **ext3** - Journaled Linux filesystem  
- ✅ **ext2** - Classic Linux filesystem

### Coming Soon
- 🚧 **NTFS** - Windows filesystem (reader in progress)
- 🚧 **FAT32** - Universal filesystem (reader in progress)
- 🚧 **exFAT** - Modern universal filesystem (reader in progress)
- 📋 **Btrfs** - Advanced Linux filesystem
- 📋 **ZFS** - Enterprise filesystem
- 📋 **APFS** - Apple filesystem
- 📋 **HFS+** - Legacy Apple filesystem

### Future Possibilities
- Commodore 1541 disk images
- Amiga filesystems
- PlayStation memory cards
- Ancient Unix filesystems
- Custom/proprietary filesystems

## Testing

### Windows Test Script
```powershell
# Run the provided test script
.\scripts\windows\test-ext4-mount.ps1 -SourceDrive E: -MountPoint M: -BuildFirst
```

### Manual Testing
1. Insert a drive with ext4 filesystem (Linux USB, dual-boot partition, etc.)
2. Note the drive letter in Windows (e.g., `E:`)
3. Run Moses mount command
4. Verify files are accessible

## Troubleshooting

### Windows

**"WinFsp not found"**
- Install from http://www.secfs.net/winfsp/
- Restart after installation

**"Access denied"**
- Run as administrator
- Check if drive is in use

**"Mount point already exists"**
- Choose a different drive letter
- Or unmount existing filesystem

**"Cannot read filesystem"**
- Verify the filesystem type
- Check if device is accessible
- Ensure filesystem isn't corrupted

### Build Issues

**"feature mount-windows not found"**
- Update Cargo.toml dependencies
- Clean build: `cargo clean`

**"winfsp crate error"**
- Ensure building on Windows
- WinFsp must be installed

## Performance

Moses Mount provides:
- **Read speeds**: Near-native performance
- **Caching**: Intelligent block and inode caching
- **Memory usage**: Minimal overhead
- **CPU usage**: Negligible for read operations

## Security Considerations

- **Read-only by default**: Prevents accidental modifications
- **User permissions**: Respects OS-level access controls
- **No kernel drivers**: Runs entirely in userspace
- **Sandboxed**: Filesystem errors won't crash the system

## Contributing

Want to add support for a new filesystem? See [EXTENSIBILITY_GUIDE.md](EXTENSIBILITY_GUIDE.md)

### Adding a New Filesystem Reader

1. Implement the reader (see existing examples)
2. Create FilesystemOps wrapper
3. Register in the ops registry
4. Test with mount command

## Future Roadmap

### Phase 1: Complete Core Readers ✅ (ext4 done!)
### Phase 2: Production Stability (current)
### Phase 3: Write Support
### Phase 4: Network Filesystems
### Phase 5: Filesystem Translation
### Phase 6: Cloud Storage Backends

## License

Moses Mount is part of the Moses project, licensed under the same terms.