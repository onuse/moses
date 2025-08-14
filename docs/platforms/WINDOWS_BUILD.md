# Building Moses on Windows

Since you're developing in WSL but want to build the application, you have two options:

## Option 1: Install WSL Dependencies (Recommended)

In your WSL terminal, you need to install the GTK dependencies. If you have sudo access:

```bash
sudo apt-get update
sudo apt-get install -y pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev
```

If you don't have sudo configured, you can either:
- Configure passwordless sudo for your user (development only)
- Ask your system administrator to install these packages

## Option 2: Native Windows Development

Install these on Windows (not WSL):

1. **Install Rust for Windows**
   - Download from: https://rustup.rs/
   - Run the installer in Windows (not WSL)

2. **Install Visual Studio Build Tools**
   - Required for compiling native Windows code
   - Download from: https://visualstudio.microsoft.com/downloads/
   - Select "Desktop development with C++"

3. **Install Node.js for Windows**
   - Download from: https://nodejs.org/

4. **Build from Windows Terminal/PowerShell**
   ```powershell
   cd C:\Users\glimm\Documents\Projects\moses
   cargo build
   npm install
   npm run tauri dev
   ```

## Option 3: Development Without GUI (Current Workaround)

You can still work on and test the core functionality without building the Tauri GUI:

```bash
# Build only the core libraries and CLI (no GUI dependencies needed)
cargo build --workspace --exclude app

# Test the CLI
./target/debug/moses list
```

## Current Workaround

The project is structured so that the core formatting logic is separate from the GUI. You can:

1. Develop and test the core modules (device enumeration, formatters)
2. Work on the CLI version
3. The GUI frontend can be developed separately with mock data

To build just the non-GUI components:
```bash
cd /mnt/c/Users/glimm/Documents/Projects/moses
cargo build --package moses-core
cargo build --package moses-cli
cargo build --package moses-formatters
cargo build --package moses-platform
cargo build --package moses-daemon
```

These will build successfully without GTK dependencies!