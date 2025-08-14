# Windows Setup Guide for Moses

## Prerequisites for Building Moses on Windows

### Required Software

#### 1. Rust & Cargo (Required)
Download and install from: https://rustup.rs/
- Run the installer (rustup-init.exe)
- Choose default installation (option 1)
- This installs Rust, Cargo, and rustup

#### 2. Visual Studio Build Tools (Required)
Download from: https://visualstudio.microsoft.com/downloads/
- Scroll down to "Tools for Visual Studio"
- Download "Build Tools for Visual Studio 2022"
- During installation, select:
  - ✅ "Desktop development with C++"
  - This installs MSVC compiler needed by Rust

#### 3. Node.js (Required for GUI)
Download from: https://nodejs.org/
- Download the LTS version (20.x or later)
- Run the installer with default options
- This includes npm which is needed for the UI

#### 4. WebView2 Runtime (Usually Pre-installed)
- Windows 10/11 usually has this pre-installed
- If missing, download from: https://developer.microsoft.com/en-us/microsoft-edge/webview2/

#### 5. WSL2 (Required for EXT4 formatting)
Open PowerShell as Administrator and run:
```powershell
wsl --install
```
This installs WSL2 and Ubuntu by default.

### Optional but Recommended

#### Git for Windows
Download from: https://git-scm.com/download/win
- Useful for cloning the repository
- Includes Git Bash terminal

## Building Moses

### Quick Build (CLI Only)
```batch
# Open Command Prompt or PowerShell
cd C:\Users\glimm\Documents\Projects\moses

# Build CLI only (fastest)
cargo build --package moses-cli --release

# Test it works
target\release\moses.exe list
```

### Full GUI Build
```batch
# Open Command Prompt or PowerShell
cd C:\Users\glimm\Documents\Projects\moses

# Install UI dependencies
cd ui
npm install
npm run build
cd ..

# Build Tauri app
npm run tauri build

# The installer will be in:
# src-tauri\target\release\bundle\
```

### Development Mode (Hot Reload)
```batch
# This runs the app with hot reload for development
npm run tauri dev
```

## Using Moses

### CLI Usage
```batch
# List all drives
target\release\moses.exe list

# Format Kingston DataTraveler as EXT4
target\release\moses.exe format "Kingston DataTraveler" ext4
```

### GUI Usage
1. Run the built executable from `src-tauri\target\release\moses.exe`
2. Or use the installer from `src-tauri\target\release\bundle\`
3. The GUI will show:
   - List of all drives with icons
   - Drive details (size, type, removable status)
   - Format options (filesystem type, label, quick format)
   - Dry run simulation before actual formatting
   - Safety warnings for system drives

## EXT4 Formatting on Windows

### How It Works
Moses uses WSL2 to format drives as EXT4 on Windows:
1. Detects your USB drive in Windows (e.g., \\.\PHYSICALDRIVE2)
2. Translates the path to WSL (e.g., /dev/sdc)
3. Uses mkfs.ext4 inside WSL to format
4. All automated - you don't need to know Linux!

### Prerequisites for EXT4
1. WSL2 must be installed (see above)
2. First run will auto-install needed tools in WSL

### Testing EXT4 Format
```batch
# Check if system is ready
powershell -ExecutionPolicy Bypass -File check_ext4_ready.ps1

# Format your Kingston DataTraveler
format_kingston_ext4.bat
```

## Troubleshooting

### "cargo: command not found"
- Rust is not installed or not in PATH
- Run the Rust installer from https://rustup.rs/
- Restart your terminal after installation

### "error: Microsoft Visual C++ 14.0 or greater is required"
- Install Visual Studio Build Tools (see above)
- Make sure to select "Desktop development with C++"

### "npm: command not found"
- Node.js is not installed
- Download and install from https://nodejs.org/

### "WSL2 is not installed"
- Run `wsl --install` in Administrator PowerShell
- Restart your computer after installation
- Run `wsl --install -d Ubuntu` if no distribution is installed

### "Failed to enumerate devices"
- Some antivirus software may block device enumeration
- Try running as Administrator
- Check Windows Defender or antivirus logs

## Quick Start Commands Summary
```batch
# One-time setup (as Administrator)
wsl --install

# Clone the project (if using Git)
git clone <repository-url>
cd moses

# Build everything
cargo build --release
cd ui && npm install && npm run build && cd ..
npm run tauri build

# Run the GUI
src-tauri\target\release\moses.exe

# Or just use CLI
target\release\moses.exe list
target\release\moses.exe format "Kingston DataTraveler" ext4
```

## Features Available in UI
✅ Device enumeration - Shows all drives with details
✅ Device type icons - Visual indicators for USB, SSD, HDD
✅ System drive protection - Warns and prevents system drive formatting
✅ Format simulation - Dry run before actual format
✅ EXT4 on Windows - Full WSL2 integration
✅ Real-time progress - Shows formatting progress
✅ Safety confirmations - Multiple safety checks

The GUI provides the same functionality as the CLI but with a user-friendly interface!