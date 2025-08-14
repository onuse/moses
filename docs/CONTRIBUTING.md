# Contributing to Moses

Thank you for your interest in contributing to Moses! This document provides guidelines and instructions for contributing to the project.

## Table of Contents
- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Project Structure](#project-structure)
- [Adding New Filesystems](#adding-new-filesystems)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)

## Code of Conduct

- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on constructive criticism
- Respect differing viewpoints and experiences

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/yourusername/moses.git
   cd moses
   ```
3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/originalowner/moses.git
   ```
4. **Create a branch** for your feature:
   ```bash
   git checkout -b feature/your-feature-name
   ```

## Development Setup

See [BUILD.md](./BUILD.md) for detailed build instructions for your platform.

### Quick Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node.js dependencies
cd ui && npm install && cd ..

# Run in development mode
npm run tauri dev

# Run tests
cargo test --all
```

## How to Contribute

### Reporting Bugs

1. Check if the bug has already been reported in [Issues](https://github.com/yourusername/moses/issues)
2. Create a new issue with:
   - Clear title and description
   - Steps to reproduce
   - Expected vs actual behavior
   - System information (OS, version, etc.)
   - Screenshots if applicable

### Suggesting Features

1. Check [Issues](https://github.com/yourusername/moses/issues) and [Discussions](https://github.com/yourusername/moses/discussions) for similar ideas
2. Create a new issue or discussion with:
   - Use case description
   - Proposed solution
   - Alternative solutions considered
   - Mockups/examples if applicable

### Contributing Code

#### Small Changes (Bug fixes, typos)
1. Create a branch from `main`
2. Make your changes
3. Test thoroughly
4. Submit a pull request

#### Large Changes (New features, refactoring)
1. Open an issue first to discuss the change
2. Wait for maintainer feedback
3. Implement according to agreed approach
4. Submit pull request with reference to the issue

## Project Structure

```
moses/
├── core/                 # Core business logic
│   ├── src/
│   │   ├── device.rs    # Device abstractions
│   │   ├── filesystem.rs # Filesystem traits
│   │   └── lib.rs       # Public API
│   └── Cargo.toml
├── platform/            # Platform-specific code
│   ├── src/
│   │   ├── windows/     # Windows implementation
│   │   ├── macos/       # macOS implementation
│   │   └── linux/       # Linux implementation
│   └── Cargo.toml
├── formatters/          # Filesystem formatters
│   ├── src/
│   │   ├── ext4.rs      # EXT4 formatter
│   │   ├── ntfs.rs      # NTFS formatter
│   │   └── lib.rs
│   └── Cargo.toml
├── cli/                 # Command-line interface
│   └── src/main.rs
├── src-tauri/           # Tauri backend
│   └── src/lib.rs
├── ui/                  # Vue.js frontend
│   └── src/
│       └── App.vue
└── docs/               # Documentation
```

## Adding New Filesystems

To add support for a new filesystem:

### 1. Create Formatter Implementation

Create `formatters/src/yourfs.rs`:

```rust
use moses_core::{Device, FilesystemFormatter, FormatOptions, MosesError, Platform, SimulationReport};
use std::time::Duration;

pub struct YourFsFormatter;

#[async_trait::async_trait]
impl FilesystemFormatter for YourFsFormatter {
    fn name(&self) -> &'static str {
        "yourfs"
    }
    
    fn supported_platforms(&self) -> Vec<Platform> {
        vec![Platform::Linux, Platform::Windows, Platform::MacOS]
    }
    
    fn can_format(&self, device: &Device) -> bool {
        // Check if device can be formatted
        !device.is_system
    }
    
    async fn format(
        &self,
        device: &Device,
        options: &FormatOptions,
    ) -> Result<(), MosesError> {
        // Implement formatting logic
        todo!()
    }
    
    // Implement other required methods...
}
```

### 2. Register in formatters/src/lib.rs

```rust
pub mod yourfs;
pub use yourfs::YourFsFormatter;
```

### 3. Add Platform-Specific Implementation

If needed, create platform-specific versions:
- `formatters/src/yourfs_windows.rs`
- `formatters/src/yourfs_linux.rs`
- `formatters/src/yourfs_macos.rs`

### 4. Update UI

Add to `ui/src/App.vue`:

```vue
<option value="yourfs">YourFS</option>
```

### 5. Update CLI

Add support in `cli/src/main.rs`:

```rust
"yourfs" => {
    // Handle YourFS formatting
}
```

### 6. Add Tests

Create `formatters/tests/yourfs_test.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_yourfs_format() {
        // Test formatting logic
    }
}
```

## Testing

### Unit Tests
```bash
# Run all tests
cargo test --all

# Run specific package tests
cargo test --package moses-formatters

# Run with output
cargo test --all -- --nocapture
```

### Integration Tests
```bash
# Test CLI
./target/debug/moses list
./target/debug/moses format --dry-run /dev/sdx ext4

# Test GUI
npm run tauri dev
```

### Platform-Specific Testing

#### Windows
```powershell
# Test WSL2 integration
wsl --status
powershell -ExecutionPolicy Bypass -File check_ext4_ready.ps1
```

#### Linux
```bash
# Test with loop device
dd if=/dev/zero of=test.img bs=1M count=100
sudo losetup /dev/loop0 test.img
./target/debug/moses format /dev/loop0 ext4
```

## Pull Request Process

### Before Submitting

1. **Update documentation** if needed
2. **Add tests** for new functionality
3. **Run all tests** and ensure they pass:
   ```bash
   cargo test --all
   cargo clippy --all -- -D warnings
   cargo fmt --all -- --check
   ```
4. **Update CHANGELOG.md** with your changes
5. **Squash commits** if needed for clarity

### PR Guidelines

1. **Title**: Use conventional commits format
   - `feat:` New feature
   - `fix:` Bug fix
   - `docs:` Documentation changes
   - `test:` Test additions/changes
   - `refactor:` Code refactoring
   - `chore:` Maintenance tasks

2. **Description**: Include:
   - What changes were made
   - Why the changes were necessary
   - How to test the changes
   - Related issue numbers

3. **Size**: Keep PRs focused and reasonably sized
   - Split large changes into multiple PRs
   - One feature/fix per PR

### Review Process

1. Maintainers will review within 3-5 days
2. Address feedback promptly
3. Re-request review after changes
4. PRs need at least one approval to merge

## Development Tips

### Rust Guidelines

- Use `clippy` for linting: `cargo clippy`
- Format with `rustfmt`: `cargo fmt`
- Document public APIs with `///` comments
- Use `Result<T, MosesError>` for error handling
- Prefer `async/await` for I/O operations

### TypeScript/Vue Guidelines

- Use TypeScript for type safety
- Follow Vue 3 Composition API patterns
- Use ESLint: `npm run lint`
- Format with Prettier: `npm run format`

### Commit Messages

Write clear, concise commit messages:
```
feat: add BTRFS formatter for Linux

- Implement BTRFS formatting using btrfs-progs
- Add dry-run simulation
- Include progress reporting
- Add tests for error cases

Closes #123
```

## Getting Help

- **Discord**: [Join our Discord](https://discord.gg/moses) (if applicable)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/moses/discussions)
- **Issues**: [GitHub Issues](https://github.com/yourusername/moses/issues)

## Recognition

Contributors will be:
- Listed in [CONTRIBUTORS.md](./CONTRIBUTORS.md)
- Mentioned in release notes
- Given credit in commit messages

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (see [LICENSE](./LICENSE)).