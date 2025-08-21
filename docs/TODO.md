# Moses Project TODO

This document tracks the current development status, ongoing work, and future plans for Moses.

## 🎯 Project Vision
Moses aims to be the go-to cross-platform drive formatting tool, with a special focus on making "impossible" formats easy (like EXT4 on Windows).

## ✅ Completed Features

### Core Infrastructure
- [x] Project structure with modular architecture
- [x] Core device abstraction traits (`DeviceManager`, `FilesystemFormatter`)
- [x] Platform module structure (Windows, Linux, macOS scaffolding)
- [x] Error handling system with `MosesError` type
- [x] Tauri + Vue.js GUI framework setup

### Windows Implementation
- [x] Full device enumeration using PowerShell + WMI
- [x] Device type detection (USB, SSD, HDD, SD Card)
- [x] System drive protection
- [x] Mount point detection
- [x] Permission level checking (Admin vs ReadOnly)

### EXT4 on Windows (MVP Feature!)
- [x] WSL2 detection and validation
- [x] Device path translation (Windows → WSL)
- [x] mkfs.ext4 execution via WSL2
- [x] Auto-installation of required tools in WSL
- [x] Full format implementation with progress
- [x] Integration in both CLI and GUI

### User Interface
- [x] Device list with visual indicators (icons)
- [x] Format options panel (filesystem, label, quick format)
- [x] Dry-run simulation before format
- [x] Safety confirmations
- [x] Real-time progress updates

### Documentation
- [x] Comprehensive BUILD.md for all platforms
- [x] CONTRIBUTING.md for GitHub contributors
- [x] GitHub-ready README with badges
- [x] Windows-specific setup guides
- [x] GitHub Actions CI/CD workflows

## 🚧 In Progress

### Current Sprint (FAT32 Implementation Complete!)
- [x] Implement native FAT32 formatter without external tools
- [x] Create modular FAT validation framework 
- [x] Fix FAT32 device access issues (physical drive vs volume paths)
- [x] Test FAT32 formatter on real hardware
- [x] Add FAT32 to UI filesystem options

## 📋 Short-term TODOs (Next Week)

### Critical Path to v0.1.0 Release
1. **Complete Windows EXT4 Testing**
   - [ ] Format test drive successfully
   - [ ] Verify formatted drive works in Linux
   - [ ] Document any issues/workarounds

2. **Linux Implementation**
   - [ ] Native device enumeration (partially done)
   - [ ] Native EXT4 formatting using mkfs.ext4
   - [ ] Privilege escalation (pkexec/sudo)
   - [ ] Test on Ubuntu, Fedora, Arch

3. **macOS Scaffolding**
   - [ ] Basic device enumeration using DiskUtil
   - [ ] Format capability assessment
   - [ ] Document limitations

4. **Other Filesystems**
   - [ ] NTFS formatter implementation
   - [x] FAT32 formatter implementation (COMPLETE - native, no external tools)
   - [x] FAT16 formatter implementation (COMPLETE - native, validated)
   - [ ] exFAT formatter implementation
   - [ ] Add partitioner support to exFAT formatter
   - [ ] Add partitioner support to ext4 formatter

5. **Polish for Release**
   - [ ] Add application icon/logo
   - [ ] Create screenshots for README
   - [ ] Write CHANGELOG.md
   - [ ] Tag v0.1.0 release

## 🗓️ Medium-term Goals (Next Month)

### Enhanced Features
- [ ] **Progress Reporting**
  - Real-time progress from mkfs commands
  - Progress bar in GUI
  - ETA calculation
  - [ ] Add progress callbacks to disk operations
  - [ ] Add progress indicators for long operations

- [ ] **Advanced Options**
  - Cluster size selection
  - Compression options (for NTFS)
  - Filesystem-specific options

- [ ] **Better Device Info**
  - SMART status
  - Device health warnings
  - Partition table info (GPT/MBR)

- [ ] **Bundled Tools**
  - Bundle mke2fs.exe for Windows (no WSL dependency)
  - Static binaries for all platforms
  - Automatic tool download/update

### Platform Improvements
- [ ] **Windows**
  - WinUSB API for direct device access
  - Windows Storage Spaces support
  - BitLocker detection

- [ ] **Linux**
  - Loop device support for testing
  - LVM support
  - LUKS encryption detection

- [ ] **macOS**
  - Full IOKit integration
  - APFS formatting
  - FileVault detection

## 🚀 Long-term Roadmap (3-6 Months)

### Major Features
1. **Plugin System**
   - [ ] Dynamic formatter loading
   - [ ] Community-contributed formatters
   - [ ] Formatter marketplace/registry

2. **Advanced Operations**
   - [ ] Partition management (create, resize, delete)
   - [ ] Disk cloning/imaging
   - [ ] Secure wipe (DOD, Gutmann)
   - [ ] Bad sector scanning

3. **Network Features**
   - [ ] Format network-attached storage
   - [ ] iSCSI target formatting
   - [ ] Remote formatting via daemon

4. **Enterprise Features**
   - [ ] Bulk formatting operations
   - [ ] Formatting profiles/templates
   - [ ] Audit logging
   - [ ] Active Directory integration

5. **Exotic Filesystems**
   - [ ] ZFS
   - [ ] BTRFS  
   - [ ] XFS
   - [ ] ReiserFS
   - [ ] Amiga/Atari retro filesystems

## 🐛 Known Issues

### High Priority
- [ ] WSL2 device mapping may fail with >3 USB devices
- [ ] No rollback if format fails mid-operation
- [ ] GUI doesn't refresh after format completes
- [ ] Get actual device CHS geometry instead of hardcoding 63/255
- [ ] Auto-analyze unknown filesystems and update device info

### Medium Priority
- [ ] CLI doesn't support --no-confirm flag
- [ ] Device names truncated in GUI for long names
- [ ] No localization/i18n support

### Low Priority
- [ ] Code duplication between ext4_windows.rs and ext4_linux.rs
- [ ] Missing unit tests for formatters
- [ ] No telemetry/usage analytics

## 🧪 Testing Requirements

### Before Each Release
- [ ] Format test on Windows 10, Windows 11
- [ ] Format test on Ubuntu, Fedora, Arch
- [ ] Format test on macOS (Intel and Apple Silicon)
- [ ] Test all supported filesystems
- [ ] Test with various drive sizes (small USB to large HDD)
- [ ] Test error scenarios (disconnect during format)

### Automated Testing
- [ ] Unit tests for all formatters
- [ ] Integration tests with mock devices
- [ ] E2E tests with Tauri
- [ ] Performance benchmarks

## 💡 Ideas & Experiments

### Cool Features to Explore
- **AI-Powered Suggestions**: "This drive was previously NTFS, suggest keeping it?"
- **Cloud Backup Before Format**: Auto-backup to S3/GCS before formatting
- **Format Scheduling**: Schedule formats for maintenance windows
- **Mobile App**: Control Moses remotely from phone
- **Docker Integration**: Format container volumes
- **Blockchain**: Store format history on blockchain (why? why not?)

### Technical Experiments
- Rust async improvements with Tokio
- WebAssembly version for browser
- GPU-accelerated secure wipe
- Machine learning for bad sector prediction

## 📝 Notes for Next Session

### Recent Accomplishments (2025-08-20)
1. **FAT32 Implementation Complete**: Native FAT32 formatter without external tools
2. **Modular FAT Architecture**: Created shared FAT components with ~40% code reuse
3. **Socket-Based Worker**: Unified all UAC operations through persistent worker
4. **Fixed Device Access**: Resolved physical drive vs volume path issues

### Architecture Improvements Made
- **Socket-Based Worker System**: Single UAC prompt per session using TCP localhost
- **FAT Module Structure**: 
  - `fat_common/` - Shared components (constants, boot sector, cluster calc, FAT writer)
  - `fat16/` and `fat32/` - Specific implementations
  - Comprehensive validation framework for both
- **Critical Windows Fixes**:
  - Always use physical drive paths for formatting
  - Keep file handles open throughout operations
  - Dismount volumes before formatting

When continuing development:
1. **UI Enhancements**: Add MBR/GPT conversion UI, conflict warnings, 'Prepare Disk' wizard
2. **NTFS Reader**: Implement NTFS filesystem reader
3. **Progress System**: Add real-time progress callbacks and indicators
4. **Build command**: `cargo build --release` in src-tauri directory
5. **Test FAT32**: Format USB drives through Moses UI and verify Windows recognition

### Key Files to Review
- `formatters/src/fat32/formatter_native.rs` - Native FAT32 implementation
- `formatters/src/fat_common/` - Shared FAT components
- `formatters/src/utils.rs` - Device access utilities
- `src-tauri/src/commands/disk_management_socket.rs` - Socket-based worker commands
- `src-tauri/src/bin/moses-elevated-worker.rs` - Elevated worker process
- `platform/src/windows/device.rs` - Device enumeration
- `src-tauri/src/lib.rs` - GUI backend integration

### Development Environment Notes
- Working in WSL on Windows machine
- Main test device: Kingston DataTraveler 3.0 (57.66 GB, PHYSICALDRIVE2)
- WSL2 with Ubuntu installed
- Can't use sudo in WSL without password

---

*Last Updated: 2025-08-20*
*Status: FAT16 and FAT32 implementations complete and working*
*Next Focus: UI enhancements and NTFS reader implementation*