# Moses Format Testing Checklist

## Pre-Testing Setup

### Safety Checklist
- [ ] **Backup any important data** on the test USB drive
- [ ] Verify test drive is **NOT a system drive**
- [ ] Confirm drive shows as "Removable: Yes" in `moses list`
- [ ] Note the exact device name (e.g., "Kingston DataTraveler 3.0")

### Build Preparation
```batch
# Build release version for testing
cargo build --package moses-cli --release

# Verify build succeeded
target\release\moses.exe --version
```

### System Requirements Check
- [ ] **For EXT4**: WSL2 installed (`wsl --status`)
- [ ] **For NTFS/FAT32/exFAT**: Running on Windows (native support)
- [ ] **Admin privileges**: Available if needed (some formats may require)

## Test Device Information

Record your test device details:
- **Device Name**: ________________________________
- **Device Path**: ________________________________  
- **Size**: ________ GB
- **Current Format**: ______________________________
- **Drive Letter**: ________________________________

## Format Testing Procedure

### 1. EXT4 Format Test

#### Pre-Format Checks
- [ ] Run: `powershell -ExecutionPolicy Bypass -File scripts\windows\check_ext4_ready.ps1`
- [ ] WSL2 is available
- [ ] Ubuntu or other distro installed
- [ ] Device shows in `moses list`

#### Dry Run
```batch
target\release\moses.exe format "YOUR_DEVICE_NAME" ext4 --dry-run
```
- [ ] Dry run completes without errors
- [ ] Warnings are reasonable
- [ ] WSL2 mentioned in output

#### Actual Format
```batch
target\release\moses.exe format "YOUR_DEVICE_NAME" ext4
```
- [ ] Confirmation prompt appears
- [ ] Type "yes" to proceed
- [ ] Format completes successfully
- [ ] No error messages

#### Verification
- [ ] Run: `wsl lsblk -f` 
- [ ] Device shows as ext4
- [ ] In WSL: `wsl sudo mkdir -p /mnt/test && wsl sudo mount /dev/sdX /mnt/test`
- [ ] Can write files in WSL: `wsl echo "test" | sudo tee /mnt/test/test.txt`

### 2. NTFS Format Test

#### Dry Run
```batch
target\release\moses.exe format "YOUR_DEVICE_NAME" ntfs --dry-run
```
- [ ] Dry run completes
- [ ] No external tools required

#### Actual Format
```batch
target\release\moses.exe format "YOUR_DEVICE_NAME" ntfs
```
- [ ] Format completes successfully
- [ ] Drive appears in Windows Explorer
- [ ] Shows as NTFS in Properties

#### Verification
- [ ] Can create files on drive
- [ ] Can create folders
- [ ] Large files (>4GB) supported
- [ ] File permissions work

### 3. FAT32 Format Test

#### Dry Run
```batch
target\release\moses.exe format "YOUR_DEVICE_NAME" fat32 --dry-run
```
- [ ] Dry run completes
- [ ] Warning about 4GB file limit shown
- [ ] For >32GB drives: Warning about Windows limitations

#### Actual Format
```batch
target\release\moses.exe format "YOUR_DEVICE_NAME" fat32
```
- [ ] Format completes successfully
- [ ] Drive appears in Windows Explorer
- [ ] Shows as FAT32 in Properties

#### Verification
- [ ] Can create files on drive
- [ ] Cannot create files >4GB (expected limitation)
- [ ] Drive works in other devices (camera, game console, etc.)

### 4. exFAT Format Test

#### Dry Run
```batch
target\release\moses.exe format "YOUR_DEVICE_NAME" exfat --dry-run
```
- [ ] Dry run completes
- [ ] No file size limitations mentioned

#### Actual Format
```batch
target\release\moses.exe format "YOUR_DEVICE_NAME" exfat
```
- [ ] Format completes successfully
- [ ] Drive appears in Windows Explorer
- [ ] Shows as exFAT in Properties

#### Verification
- [ ] Can create files on drive
- [ ] Can create files >4GB
- [ ] Drive works in modern devices

## Performance Testing

For each format, record:

| Format | Quick Format Time | Full Format Time | Write Speed | Read Speed |
|--------|------------------|------------------|-------------|------------|
| EXT4   | _______ sec      | _______ sec      | _____ MB/s  | _____ MB/s |
| NTFS   | _______ sec      | _______ sec      | _____ MB/s  | _____ MB/s |
| FAT32  | _______ sec      | _______ sec      | _____ MB/s  | _____ MB/s |
| exFAT  | _______ sec      | _______ sec      | _____ MB/s  | _____ MB/s |

## Edge Cases to Test

### Label Testing
- [ ] Maximum length label (format-specific)
- [ ] Special characters in label
- [ ] Empty label
- [ ] Unicode characters

### Size Testing
- [ ] Small USB (< 8GB)
- [ ] Medium USB (16-64GB)
- [ ] Large USB (> 128GB)
- [ ] FAT32 on >32GB drive

### Error Handling
- [ ] Format system drive (should be blocked)
- [ ] Format mounted drive
- [ ] Cancel during format (Ctrl+C)
- [ ] Invalid filesystem type

## Cross-Platform Testing

If you have access to other systems:

### Linux Testing
- [ ] EXT4 formatted drive mounts correctly
- [ ] NTFS drive readable (may need ntfs-3g)
- [ ] FAT32/exFAT work without issues

### macOS Testing
- [ ] exFAT drive works perfectly
- [ ] FAT32 drive works
- [ ] NTFS read-only (unless NTFS-3G installed)
- [ ] EXT4 requires additional software

## Bug Report Template

If you encounter issues:

```markdown
**Format Type**: [EXT4/NTFS/FAT32/exFAT]
**Device**: [Name and size]
**Error Message**: [Exact error]
**Steps to Reproduce**:
1. 
2. 
3. 
**Expected**: 
**Actual**: 
**moses.exe version**: 
**Windows Version**: 
**WSL2 Version** (if applicable): 
```

## Success Criteria

A format is considered successfully implemented if:
- ✅ Dry run provides accurate information
- ✅ Format completes without errors
- ✅ Formatted drive is usable
- ✅ Safety checks prevent system drive formatting
- ✅ Error messages are clear and helpful
- ✅ Performance is reasonable (<1 min for quick format)

## Test Summary

After testing all formats:

- [ ] All formats work on test device
- [ ] Safety features confirmed working
- [ ] No data loss on system drives
- [ ] Error handling is robust
- [ ] Performance is acceptable

**Tested By**: _______________________
**Date**: ___________________________
**Test Device**: ____________________
**Result**: [ ] PASS [ ] FAIL

## Notes

_Record any observations, issues, or suggestions here:_

___________________________________________
___________________________________________
___________________________________________
___________________________________________