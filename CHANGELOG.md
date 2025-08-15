# Changelog

All notable changes to Moses will be documented in this file.

## [Unreleased]

### Added
- **Native EXT4 Support**: Complete pure-Rust implementation of EXT4 filesystem formatter
  - No external dependencies required (removed WSL2 requirement on Windows)
  - Cross-platform support (Windows, Linux, macOS)
  - Full e2fsck validation compliance
  - Implements lost+found directory (inode 11)
  - CRC16/CRC32c checksums matching Linux kernel implementation
  - Support for up to 8TB volumes

### Changed
- Updated UI to reflect native EXT4 support
- Removed all WSL2 references from documentation and code
- Improved error messages and user feedback

### Fixed
- EXT4 formatter now passes all e2fsck validation checks
- Fixed group descriptor checksums using exact Linux kernel CRC16 algorithm
- Correct inode bitmap initialization
- Proper unused inode count calculation

### Removed
- WSL2 dependency for Windows users
- Legacy ext4_windows module (moved to legacy feature flag)

## [0.1.0] - Initial Release

### Added
- Cross-platform drive formatting GUI and CLI
- Support for NTFS, FAT32, exFAT filesystems
- Safety checks to prevent system drive formatting
- Dry-run mode for testing
- Progress tracking during format operations