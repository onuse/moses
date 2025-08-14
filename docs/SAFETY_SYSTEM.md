# Moses Safety System

## Overview

Moses implements a **mandatory safety enforcement system** that makes it impossible for formatters to accidentally format system drives or cause data loss without explicit checks and acknowledgments.

## The Problem

Without enforcement, formatter implementers might:
- Forget to check if a drive is a system drive
- Not realize a drive has critical mount points
- Skip important safety validations
- Create formatters that could destroy user's OS

## The Solution: Mandatory Safety Checks

### 1. SafetyCheck Object

Every formatter MUST create and complete a `SafetyCheck` object before formatting:

```rust
// MANDATORY: Create safety check
let mut safety_check = SafetyCheck::new(device, self.name());

// MUST verify not a system drive
safety_check.verify_not_system_drive()?;

// MUST verify mount points are safe
safety_check.verify_safe_mount_points()?;

// MUST acknowledge data loss
safety_check.acknowledge_data_loss(backup_confirmed)?;

// MUST assess risk level
let risk = safety_check.assess_risk();
if risk > RiskLevel::Medium {
    return Err(MosesError::UnsafeDevice("Risk too high"));
}

// MUST validate all checks passed
let validation = safety_check.validate()?;
```

### 2. Risk Levels

The system automatically calculates risk levels:

- **Safe** 🟢 - Removable drive, no mounts, not system
- **Low** 🟡 - Non-system drive with non-critical mounts
- **Medium** 🟠 - Important drive but user confirmed
- **High** 🔴 - System-adjacent drive
- **Critical** ⚫ - System drive or critical mounts
- **Forbidden** 🚫 - Should NEVER format

### 3. SafeFormatter Wrapper

For additional safety, formatters can be wrapped in `SafeFormatter`:

```rust
// Wrap any formatter in SafeFormatter for automatic safety
let safe_formatter = SafeFormatter::new(MyFormatter);

// SafeFormatter enforces all checks automatically
registry.register(
    "my-format",
    Arc::new(safe_formatter),
    metadata
);
```

## Safety Features

### 1. System Drive Protection
- Automatically detects system drives
- Blocks formatting unless explicit override with reason
- Tracks who attempted to format system drives

### 2. Mount Point Protection
Critical mount points are protected:
- `/`, `/boot`, `/system`, `/usr`, `/var`, `/etc`, `/home`
- `C:\`, `C:\Windows`, `C:\Program Files`, `C:\Users`
- `/System`, `/Library`, `/Applications` (macOS)

### 3. Data Loss Acknowledgment
- Formatters MUST acknowledge data will be lost
- High-risk operations require backup confirmation
- Estimated data size is tracked

### 4. Custom Safety Checks
Formatters can add their own checks:
```rust
safety_check.add_custom_check(
    "raid_check",
    device_not_in_raid,
    "Device is not part of RAID array"
);
```

### 5. Audit Trail
Every safety validation is logged:
```rust
SafetyValidation {
    check_id: "usb-drive-1234567890",
    device_id: "\\.\PHYSICALDRIVE2",
    risk_level: RiskLevel::Safe,
    timestamp: "2024-01-15T10:30:00Z",
    formatter: "ext4",
}
```

## Implementation Requirements

### For Formatter Developers

1. **MUST create SafetyCheck**
```rust
let mut safety_check = SafetyCheck::new(device, self.name());
```

2. **MUST perform all checks**
```rust
safety_check.verify_not_system_drive()?;
safety_check.verify_safe_mount_points()?;
safety_check.acknowledge_data_loss(true)?;
```

3. **MUST validate before proceeding**
```rust
let validation = safety_check.validate()?;
```

4. **MUST respect risk levels**
```rust
if risk > RiskLevel::Medium {
    return Err(MosesError::UnsafeDevice("Risk too high"));
}
```

## Example: Safe Formatter

```rust
async fn format(&self, device: &Device, options: &FormatOptions) -> Result<(), MosesError> {
    // Create mandatory safety check
    let mut safety_check = SafetyCheck::new(device, self.name());
    
    // Verify system drive
    safety_check.verify_not_system_drive()
        .map_err(|e| {
            eprintln!("🚨 SAFETY: Attempted to format system drive!");
            e
        })?;
    
    // Verify mount points
    safety_check.verify_safe_mount_points()
        .map_err(|e| {
            eprintln!("🚨 SAFETY: Critical mount points detected!");
            e
        })?;
    
    // Custom checks
    if device.size < MIN_SIZE {
        safety_check.add_custom_check("size", false, "Too small");
        return Err(MosesError::InvalidInput("Device too small"));
    }
    
    // Acknowledge data loss
    safety_check.acknowledge_data_loss(true)?;
    
    // Check risk
    let risk = safety_check.assess_risk();
    if risk > RiskLevel::Medium {
        return Err(MosesError::UnsafeDevice(
            format!("Risk level {:?} too high", risk)
        ));
    }
    
    // Validate
    let validation = safety_check.validate()?;
    println!("✅ Safety check passed: {}", validation.check_id);
    
    // Now safe to format
    do_actual_format(device, options).await
}
```

## Testing Safety

### Unit Tests
```rust
#[test]
fn test_blocks_system_drive() {
    let system_drive = create_system_drive();
    let mut check = SafetyCheck::new(&system_drive, "test");
    
    // Should fail
    assert!(check.verify_not_system_drive().is_err());
    assert_eq!(check.assess_risk(), RiskLevel::Forbidden);
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_formatter_safety() {
    let formatter = SafeExt4Formatter;
    let system_drive = create_system_drive();
    
    // Should refuse to format
    assert!(!formatter.can_format(&system_drive));
    
    // Format should fail with safety error
    let result = formatter.format(&system_drive, &options).await;
    assert!(matches!(result, Err(MosesError::UnsafeDevice(_))));
}
```

## Safety Compliance

### Certification
Formatters can be certified as "Moses Safety Compliant" if they:
1. ✅ Always create SafetyCheck
2. ✅ Perform all mandatory checks
3. ✅ Respect risk levels
4. ✅ Pass safety test suite
5. ✅ Never bypass safety system

### Non-Compliant Formatters
Formatters that don't use SafetyCheck:
- ❌ Will fail code review
- ❌ Won't be accepted in official registry
- ❌ Will trigger warnings when loaded
- ❌ May be wrapped in SafeFormatter automatically

## Benefits

1. **Impossible to Accidentally Format System Drives**
   - Safety checks are mandatory, not optional
   - System drives are detected automatically

2. **Clear Risk Communication**
   - Users see exact risk level
   - Warnings are explicit and actionable

3. **Audit Trail**
   - Every format attempt is logged
   - Can track who tried to format what

4. **Consistent Safety Across All Formatters**
   - Same safety standards for all filesystems
   - No formatter can bypass safety

5. **Legal Protection**
   - Demonstrates due diligence
   - Clear evidence of safety measures

## Conclusion

The Moses Safety System makes it **structurally impossible** to create unsafe formatters. By requiring explicit safety checks and validations, we ensure that:

- 🛡️ System drives are protected
- 🔒 Critical data is safeguarded  
- ⚠️ Risks are clearly communicated
- 📝 All operations are audited
- ✅ Safety is guaranteed, not hoped for

**Remember: Safety is not optional in Moses - it's mandatory!**