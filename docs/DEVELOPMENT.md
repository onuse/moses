# Moses Development Setup

## Prerequisites

### System Dependencies (Linux)

Before building Moses on Linux, you need to install the following system packages:

```bash
sudo apt-get update
sudo apt-get install -y \
    pkg-config \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    librsvg2-dev
```

### Rust

Install Rust using rustup:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Node.js

Install Node.js (version 18+ recommended) for the frontend development.

## Building the Project

### Build Core Libraries
```bash
cargo build
```

### Build Tauri Application
```bash
cd src-tauri
cargo build
```

### Run Development Server
```bash
# Install frontend dependencies
npm install

# Run the Tauri app in development mode
npm run tauri dev
```

## Project Structure

- `core/` - Platform-agnostic business logic
- `daemon/` - Privileged formatting service (future)
- `platform/` - Platform-specific implementations
- `formatters/` - Filesystem formatter implementations
- `cli/` - Command-line interface
- `src-tauri/` - Tauri application backend
- `ui/` - Vue.js frontend application

## Current Status

### MVP Features Implemented:
- ✅ Basic project structure
- ✅ Core abstractions (Device, Formatter, etc.)
- ✅ Mock device enumeration
- ✅ Tauri GUI with Vue.js
- ✅ Dry-run simulation mode
- ✅ Safety checks (prevent system drive formatting)

### TODO for Production:
- [ ] Real device enumeration for each platform
- [ ] Actual formatting implementation
- [ ] Privileged daemon for format operations
- [ ] Bundle external tools (ext2fsd for Windows)
- [ ] Comprehensive testing
- [ ] Code signing for distribution

## Running the Application

After installing the system dependencies mentioned above:

1. Build the frontend:
```bash
npm run build
```

2. Run the Tauri app:
```bash
npm run tauri dev
```

The application will start with a mock implementation that demonstrates:
- Device listing (mock USB and system drives)
- Format simulation
- Safety warnings for system drives
- EXT4 formatting requirements on Windows