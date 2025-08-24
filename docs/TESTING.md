# Moses Testing Framework

## Overview

Moses implements a comprehensive testing framework designed to **guarantee safety** and prevent accidental data loss. Our testing philosophy is: **"Never touch real hardware in tests, always verify safety checks work."**

## 🛡️ Safety Guarantees

Our testing framework provides the following safety guarantees:

1. **No Real Hardware Access**: All tests use mock devices prefixed with `mock://`
2. **System Drive Protection**: Multiple layers of tests verify system drives cannot be formatted
3. **Critical Path Protection**: Mount points like `C:\`, `/`, `/boot` are always protected
4. **Fail-Safe Defaults**: Tests fail closed - any ambiguity results in blocking the operation

## Test Categories

### 1. Safety-Critical Tests (`formatters/tests/safety_tests.rs`)

These are the **most important tests** in the entire project. They verify:

- ✅ System drives are NEVER formatted
- ✅ Critical mount points are protected
- ✅ Device validation works correctly
- ✅ Label validation prevents invalid characters
- ✅ Size validation catches impossible devices

**Example Test:**
```rust
#[test]
fn test_ext4_formatter_refuses_system_drive() {
    let formatter = moses_filesystems::Ext4Formatter;
    let system_drive = create_system_drive();
    
    assert!(!formatter.can_format(&system_drive), 
        "CRITICAL: Formatter claims it can format a system drive!");
}
```

### 2. Mock Device Tests (`core/src/test_utils.rs`)

Provides safe testing infrastructure:

- `MockDevice`: Fake devices that track format attempts
- `MockDeviceManager`: Returns only mock devices
- `MockFormatter`: Records format calls without touching hardware
- `SafetyValidator`: Validates safety rules

**Key Safety Feature:**
```rust
async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
    // CRITICAL: Never format real devices in tests
    if !device.id.starts_with("mock://") {
        return Err(MosesError::Other(
            "SAFETY: Attempted to format non-mock device in test!".to_string()
        ));
    }
    // ... rest of mock implementation
}
```

### 3. Platform Tests (`platform/tests/`)

Tests platform-specific device enumeration:

- Device detection works correctly
- System drives are properly identified
- Device types are classified correctly
- Mount points are detected
- Permission levels are checked

### 4. Integration Tests

End-to-end tests using mock components:

- Complete format workflow with mocks
- GUI interaction tests (Tauri)
- CLI command tests
- Error handling scenarios

## Running Tests

### Quick Test (Safety Only)
```bash
# Run only critical safety tests
cargo test safety

# Windows
cargo test safety
```

### Comprehensive Test Suite
```bash
# Linux/macOS
./scripts/test/run_all_tests.sh

# Windows
scripts\test\run_all_tests.bat
```

### Individual Test Categories
```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --tests

# Documentation tests
cargo test --doc

# Specific package
cargo test --package moses-formatters
```

### Test Coverage
```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir target/coverage

# View report
open target/coverage/index.html
```

## CI/CD Integration

Every push and PR triggers:

1. **Format Check**: Code formatting verification
2. **Clippy**: Linting with strict warnings
3. **Unit Tests**: All packages tested individually
4. **Safety Tests**: Critical safety verification
5. **Integration Tests**: End-to-end testing
6. **Doc Tests**: Example code verification
7. **Pattern Checks**: Dangerous code pattern detection

## Safety Verification Checklist

Before any release, verify:

- [ ] All safety tests pass
- [ ] No real device paths in test code
- [ ] Mock devices used exclusively in tests
- [ ] System drive detection works
- [ ] Critical mount points blocked
- [ ] CI pipeline green on all platforms

## Writing New Tests

### Test Template for Formatters
```rust
#[test]
fn test_formatter_safety() {
    let formatter = YourFormatter;
    
    // Must refuse system drives
    let system = create_system_drive();
    assert!(!formatter.can_format(&system));
    
    // Must allow safe USB
    let usb = create_safe_usb();
    assert!(formatter.can_format(&usb));
    
    // Must validate options
    let invalid_options = create_invalid_options();
    assert!(formatter.validate_options(&invalid_options).is_err());
}
```

### Mock Device Creation
```rust
use moses_core::test_utils::MockDevice;

let mock_usb = MockDevice::new_usb("Test USB", 16); // 16GB USB
let mock_system = MockDevice::new_system_drive();

// Track format attempts
assert_eq!(mock_usb.format_count(), 0);
// ... perform operations
assert_eq!(mock_usb.format_count(), 1);
```

## Test Metrics

Our goal is:
- **100% coverage** for safety-critical code
- **>90% coverage** for core functionality
- **>80% coverage** overall
- **0 tolerance** for safety test failures

## Emergency Procedures

If a safety test fails:

1. **STOP** all development
2. **Alert** the team immediately
3. **Investigate** the root cause
4. **Fix** the issue before any other work
5. **Add** additional tests to prevent regression
6. **Document** the incident in CHANGELOG

## Test Philosophy

> "A drive formatter without comprehensive safety tests is a data destruction tool waiting to happen."

We test not just for correctness, but for **safety above all**. Every test is a guardian against data loss.

## Continuous Improvement

We continuously:
- Add new safety scenarios
- Improve mock device realism
- Expand platform coverage
- Enhance error messages
- Document edge cases

Remember: **Every test you write could save someone's data.**