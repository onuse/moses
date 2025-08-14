# Dependency Management Strategy for Moses

## Philosophy

We follow a **"Pragmatic Security-First"** approach to dependency management:

1. **Security Updates**: Always apply immediately
2. **Minor Updates**: Apply monthly for actively maintained dependencies  
3. **Major Updates**: Evaluate carefully, update quarterly if benefits outweigh risks
4. **Build/CI Dependencies**: Keep current to prevent breakage
5. **Core Dependencies**: Conservative updates with thorough testing

## Dependency Categories

### 🔴 Critical - Always Keep Current
These should always be on latest stable versions:

- **GitHub Actions** - Deprecation breaks CI
- **Security-critical crates** (anything handling untrusted input)
- **Build tools** (cargo, npm, etc.)
- **Tauri** - Security framework for GUI

### 🟡 Important - Update Regularly
Update monthly or when security issues arise:

- **tokio** - Async runtime (currently 1.47.1)
- **serde** - Serialization (currently 1.0.x)
- **clap** - CLI framework (currently 4.5.45)
- **Frontend frameworks** (React, Vite)

### 🟢 Stable - Conservative Updates
Update quarterly or for specific features/fixes:

- **which** - Tool detection
- **dirs** - System directories
- **chrono** - Date/time handling
- **uuid** - ID generation

## Update Schedule

### Weekly
- Check for security advisories
- Run `cargo audit` (when installed)
- Run `npm audit`

### Monthly  
- Update minor versions of important dependencies
- Update npm packages
- Update rust-toolchain if needed

### Quarterly
- Evaluate major version updates
- Update stable dependencies
- Review and update this strategy

## Commands for Maintenance

```bash
# Rust Dependencies
cargo update                    # Update to latest compatible versions
cargo tree -d                   # Check for duplicate dependencies
cargo install cargo-audit       # Install security audit tool
cargo audit                     # Check for vulnerabilities

# NPM Dependencies
npm audit                       # Check for vulnerabilities
npm audit fix                   # Auto-fix vulnerabilities
npm update                      # Update to latest compatible
npm outdated                    # List outdated packages

# Check for updates (install cargo-outdated first)
cargo install cargo-outdated
cargo outdated
```

## Current Status (as of commit)

### Rust Dependencies
- ✅ All minor versions current
- ✅ No known security vulnerabilities
- Tokio: 1.47.1 (latest is 1.x)
- Clap: 4.5.45 (latest 4.x)

### NPM Dependencies  
- ⚠️ 2 moderate vulnerabilities in esbuild/vite
- Action needed: Update vite to latest version

### GitHub Actions
- ✅ All updated to latest versions
- checkout: v5
- cache: v4
- upload-artifact: v4
- setup-rust-toolchain: v1.13.0
- setup-node: v4.4.0

## Decision Matrix

When to update a dependency:

| Scenario | Action |
|----------|--------|
| Security vulnerability | Update immediately |
| CI/Build breaks | Update immediately |
| Deprecation warning | Update within 1 month |
| Major version available | Evaluate benefits, update if valuable |
| Minor version available | Update monthly batch |
| Patch version available | Update in weekly batch |

## Benefits of This Approach

1. **Security**: Vulnerabilities patched quickly
2. **Stability**: Core deps remain stable
3. **Compatibility**: Avoid breaking changes
4. **Maintainability**: Regular, predictable updates
5. **Performance**: Get improvements without risk

## Risks of "Always Latest"

- Breaking changes in major versions
- Incompatibilities between dependencies  
- Unnecessary churn and testing overhead
- Potential for new bugs

## Risks of "Never Update"

- Security vulnerabilities
- Missing performance improvements
- Incompatibility with new tools/platforms
- Technical debt accumulation
- Deprecation breaking builds

## Conclusion

We aim for the middle ground: keeping dependencies **secure and functional** while avoiding unnecessary churn. Security and build tools stay current, core libraries update conservatively with testing.