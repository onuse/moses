# Moses - Cross-Platform Drive Formatter

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/yourusername/moses/ci.yml?branch=main)](https://github.com/yourusername/moses/actions)

Moses makes it easy to format drives with any filesystem on Windows, macOS, and Linux - with native support for all major filesystems!

## ✨ Key Features

- 🖥️ **Cross-Platform** - Windows, macOS, and Linux
- 🎯 **Native EXT4 Support** - Pure Rust implementation, no dependencies  
- 🛡️ **Safe** - Multiple safety checks and dry-run mode
- ⚡ **Fast** - Quick format options with progress tracking
- 🎨 **GUI & CLI** - Choose your preferred interface

## 🚀 Quick Start

### Download Release
**[⬇️ Download Latest Release](https://github.com/yourusername/moses/releases/latest)**

### Build from Source
```bash
git clone https://github.com/yourusername/moses.git
cd moses
cargo build --release
```

See [docs/BUILD.md](docs/BUILD.md) for detailed build instructions.

## 📖 Documentation

- [Building from Source](docs/BUILD.md)
- [Contributing Guide](docs/CONTRIBUTING.md)
- [Development Guide](docs/DEVELOPMENT.md)
- [Windows Setup](docs/platforms/WINDOWS_SETUP.md)
- [EXT4 Formatting Details](docs/EXT4_FORMATTING.md)

## 🎮 Usage

> **⚠️ Important:** Moses requires administrator/root privileges to format drives.

### Windows
```batch
# Run with admin privileges (double-click or from cmd)
run-as-admin.bat

# Or right-click moses.exe and select "Run as administrator"
```

### Linux/macOS
```bash
# Run with sudo
sudo moses

# List all drives
sudo moses list

# Format USB as EXT4
sudo moses format "USB Drive" ext4
```

## 📊 Supported Filesystems

| Filesystem | Windows | macOS | Linux |
|------------|---------|-------|-------|
| EXT4       | ✅ | ✅ | ✅ |
| NTFS       | ✅ | ✅ | ✅ |
| FAT32      | ✅ | ✅ | ✅ |
| exFAT      | ✅ | ✅ | ✅ |

## 🤝 Contributing

Contributions are welcome! Please see our [Contributing Guide](docs/CONTRIBUTING.md).

## 📜 License

MIT License - see [LICENSE](LICENSE) file for details.