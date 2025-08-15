# Building Moses from Source

This guide covers building Moses from source on all supported platforms. If you just want to use Moses, consider downloading a pre-built release from the [Releases](https://github.com/yourusername/moses/releases) page.

## Table of Contents
- [Prerequisites](#prerequisites)
- [Windows](#windows)
- [macOS](#macos)
- [Linux](#linux)
- [Building](#building)
- [Troubleshooting](#troubleshooting)

## Prerequisites

All platforms need:
- **Rust 1.70+** - Install from [rustup.rs](https://rustup.rs/)
- **Node.js 18+ & npm** - Install from [nodejs.org](https://nodejs.org/)
- **Git** - To clone the repository

## Windows

### Required Software

1. **Rust**
   ```powershell
   # Download and run rustup-init.exe from https://rustup.rs/
   # Or use winget:
   winget install Rustlang.Rustup
   ```

2. **Visual Studio Build Tools 2022**
   - Download from [Visual Studio Downloads](https://visualstudio.microsoft.com/downloads/)
   - Install "Desktop development with C++" workload
   - Or via command line:
   ```powershell
   winget install Microsoft.VisualStudio.2022.BuildTools
   # Then manually select C++ workload in the installer
   ```

3. **Node.js**
   ```powershell
   winget install OpenJS.NodeJS.LTS
   # Or download from https://nodejs.org/
   ```

4. **WebView2 Runtime** (usually pre-installed on Windows 10/11)
   ```powershell
   # Check if installed:
   Get-AppxPackage -Name Microsoft.WebView2Runtime
   # If not found, download from Microsoft
   ```

5. **Note:** EXT4 formatting is now natively supported on Windows - no additional tools required!

### Build Commands

```batch
# Clone the repository
git clone https://github.com/yourusername/moses.git
cd moses

# Build CLI only (fastest, ~2 minutes)
cargo build --package moses-cli --release

# Build full GUI application (~5-10 minutes)
cd ui
npm install
npm run build
cd ..
npm run tauri build

# Output locations:
# CLI: target\release\moses.exe
# GUI: src-tauri\target\release\moses.exe
# Installer: src-tauri\target\release\bundle\msi\*.msi
```

## macOS

### Required Software

1. **Xcode Command Line Tools**
   ```bash
   xcode-select --install
   ```

2. **Rust**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

3. **Node.js**
   ```bash
   # Using Homebrew (recommended)
   brew install node
   
   # Or download from https://nodejs.org/
   ```

4. **Additional Dependencies**
   ```bash
   # None required - macOS has native WebView
   ```

### Build Commands

```bash
# Clone the repository
git clone https://github.com/yourusername/moses.git
cd moses

# Build CLI only (fastest, ~2 minutes)
cargo build --package moses-cli --release

# Build full GUI application (~5-10 minutes)
cd ui
npm install
npm run build
cd ..
npm run tauri build

# Output locations:
# CLI: target/release/moses
# GUI: src-tauri/target/release/moses
# App Bundle: src-tauri/target/release/bundle/macos/Moses.app
# DMG: src-tauri/target/release/bundle/dmg/Moses_*.dmg
```

### Code Signing (Optional)
For distribution, you'll need to sign and notarize:
```bash
# Set your Apple Developer ID
export APPLE_ID="your-apple-id@example.com"
export APPLE_PASSWORD="your-app-specific-password"
export APPLE_TEAM_ID="YOUR_TEAM_ID"

# Build with signing
npm run tauri build -- --bundles app,dmg
```

## Linux

### Distribution-Specific Prerequisites

#### Ubuntu/Debian
```bash
sudo apt update
sudo apt install -y \
    build-essential \
    curl \
    wget \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libjavascriptcoregtk-4.1-dev
```

#### Fedora/RHEL/CentOS
```bash
sudo dnf install -y \
    gcc \
    gcc-c++ \
    openssl-devel \
    gtk3-devel \
    webkit2gtk4.1-devel \
    libappindicator-gtk3-devel \
    librsvg2-devel
```

#### Arch Linux
```bash
sudo pacman -S --needed \
    base-devel \
    curl \
    wget \
    openssl \
    gtk3 \
    webkit2gtk-4.1 \
    libappindicator-gtk3 \
    librsvg
```

### Install Rust and Node.js

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Node.js via NodeSource (Ubuntu/Debian)
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
sudo apt-get install -y nodejs

# Or via nvm (all distros)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
source ~/.bashrc
nvm install --lts
```

### Build Commands

```bash
# Clone the repository
git clone https://github.com/yourusername/moses.git
cd moses

# Build CLI only (fastest, ~2 minutes)
cargo build --package moses-cli --release

# Build full GUI application (~5-10 minutes)
cd ui
npm install
npm run build
cd ..
npm run tauri build

# Output locations:
# CLI: target/release/moses
# GUI: src-tauri/target/release/moses
# AppImage: src-tauri/target/release/bundle/appimage/Moses_*.AppImage
# Deb: src-tauri/target/release/bundle/deb/*.deb
```

## Building

### Quick Build (CLI Only)
Perfect for testing and command-line usage:
```bash
cargo build --package moses-cli --release
```

### Development Build (GUI with Hot Reload)
For active development with automatic reloading:
```bash
npm run tauri dev
```

### Production Build (Optimized GUI)
Creates installers and optimized binaries:
```bash
# Ensure UI is built first
cd ui && npm install && npm run build && cd ..

# Build Tauri app with all bundles
npm run tauri build
```

### Build Options

```bash
# Debug build (faster compile, larger binary)
cargo build

# Release build (optimized)
cargo build --release

# Specific package only
cargo build --package moses-core
cargo build --package moses-cli
cargo build --package moses-formatters

# Cross-compilation (requires additional setup)
cargo build --target x86_64-pc-windows-gnu  # From Linux to Windows
cargo build --target x86_64-apple-darwin     # From Linux to macOS
```

## Architecture-Specific Builds

### Apple Silicon (M1/M2/M3)
```bash
# Native ARM64 build (default on Apple Silicon)
cargo build --release

# Cross-compile for Intel Macs
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Universal binary
npm run tauri build -- --bundles app --target universal-apple-darwin
```

### Linux ARM (Raspberry Pi)
```bash
# On Raspberry Pi OS
sudo apt install gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

## Verification

After building, verify your build works:

### CLI Verification
```bash
# List devices
./target/release/moses list  # Linux/macOS
target\release\moses.exe list  # Windows

# Check version
./target/release/moses --version
```

### GUI Verification
```bash
# Run the GUI
./src-tauri/target/release/moses  # Linux/macOS
src-tauri\target\release\moses.exe  # Windows
```

## Troubleshooting

### Common Issues

#### "cargo: command not found"
- Rust is not installed or not in PATH
- Run: `source $HOME/.cargo/env` (Linux/macOS)
- Restart terminal (Windows)

#### "error: Microsoft Visual C++ 14.0 or greater is required" (Windows)
- Install Visual Studio Build Tools with C++ workload

#### "error: failed to run custom build command for `webkit2gtk-sys`" (Linux)
- Install missing development packages (see Linux prerequisites)

#### "npm: command not found"
- Node.js is not installed or not in PATH
- Install Node.js from nodejs.org

#### "Error: No suitable WebView2 Runtime found" (Windows)
- Install WebView2 Runtime from Microsoft

#### Permission denied when formatting (Linux)
- Add user to `disk` group: `sudo usermod -a -G disk $USER`
- Log out and back in

### Platform-Specific Issues

#### Windows
- If WSL2 commands fail, ensure virtualization is enabled in BIOS
- Run `wsl --update` to get latest WSL2 version

#### macOS
- If build fails with "xcrun: error", install Xcode Command Line Tools
- For code signing issues, ensure valid certificates in Keychain

#### Linux
- If AppImage doesn't run, install FUSE: `sudo apt install libfuse2`
- For Wayland issues, set: `export WEBKIT_DISABLE_COMPOSITING_MODE=1`

## Performance Tips

1. **Use `cargo build --release`** for 10x performance improvement
2. **Enable LTO** for smaller binaries:
   ```toml
   # In Cargo.toml
   [profile.release]
   lto = true
   ```
3. **Use sccache** for faster rebuilds:
   ```bash
   cargo install sccache
   export RUSTC_WRAPPER=sccache
   ```

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/yourusername/moses/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/moses/discussions)
- **Documentation**: [docs/](./docs/)

## License

See [LICENSE](./LICENSE) file for details.