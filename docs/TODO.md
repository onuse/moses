# Moses Project TODO

## Project Vision
Moses aims to be the go-to cross-platform drive formatting tool, making complex formatting tasks simple and accessible across Windows, macOS, and Linux.

## Current Status

### ✅ Completed Features
- Cross-platform device enumeration and management
- Native filesystem formatters (FAT16, FAT32, exFAT, EXT4)
- Partition table support (MBR and GPT)
- Socket-based elevated worker for Windows UAC
- Vue.js + Tauri GUI with modern interface
- Filesystem browsing and analysis
- Safety system with device protection

### 🚧 In Progress
- NTFS write support (currently read-only)
- macOS full implementation
- Performance optimizations

### 📋 Planned Features

#### Short Term
- [ ] NTFS write support completion
- [ ] Improved error messages and user feedback
- [ ] Batch formatting operations
- [ ] Command-line interface improvements

#### Medium Term
- [ ] APFS support for macOS
- [ ] Btrfs support for Linux
- [ ] Advanced partition management (resize, move)
- [ ] Disk cloning capabilities
- [ ] Secure wipe options

#### Long Term
- [ ] Plugin system for custom filesystems
- [ ] Network drive support
- [ ] Cloud storage integration
- [ ] Mobile companion app

## Known Issues
- Double UAC prompts on Windows (partially fixed)
- NTFS formatting currently disabled (safety concerns)
- Limited macOS functionality

## Contributing
See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute to Moses.

## Building
See [BUILD.md](BUILD.md) for detailed build instructions for all platforms.