# NTFS Writer - Remaining Gaps for Full Functionality

## 🔴 Critical Gaps (Blocking Core Features)

### 1. Resident Data Modification
**Priority: HIGHEST**
- **Location:** `writer_ops.rs:60-73`
- **Issue:** Cannot modify data for files ≤700 bytes stored directly in MFT
- **Impact:** Most text files, configs, and small files can't have content written
- **Solution:** 
  - Parse and update the DATA attribute value in the MFT record
  - Handle conversion from resident to non-resident when file grows

### 2. Directory Creation
**Priority: HIGH**
- **Location:** `ops_rw_v2.rs:292`
- **Issue:** `mkdir` returns NotSupported
- **Impact:** Cannot create directory structures
- **Solution:**
  - Create MFT record with INDEX_ROOT attribute
  - Initialize empty B+ tree
  - Add "." and ".." entries
  - Update parent directory index

### 3. Subdirectory Navigation
**Priority: HIGH**
- **Location:** `ops_rw_v2.rs:56`, `path_resolver.rs`
- **Issue:** Only root directory operations work
- **Impact:** Cannot create/access files in nested directories
- **Solution:**
  - Integrate PathResolver with writer
  - Implement recursive path resolution
  - Cache directory MFT record mappings

## 🟡 Important Gaps (Affecting Robustness)

### 4. Volume Bitmap Persistence
**Priority: MEDIUM**
- **Location:** `writer.rs:254-268`
- **Issue:** Bitmap changes not written back to $Bitmap file
- **Impact:** Free space tracking becomes incorrect
- **Solution:**
  - Write updated bitmap to $Bitmap DATA attribute
  - Handle both resident and non-resident bitmap storage

### 5. Non-Resident Data Allocation
**Priority: MEDIUM**  
- **Location:** `writer_ops.rs:183-187`, `mft_writer.rs`
- **Issue:** Cannot pre-allocate clusters for new files
- **Impact:** Can only create empty files
- **Solution:**
  - Implement cluster allocation from volume bitmap
  - Create proper data runs in MFT record
  - Add with_non_resident_data() to MftRecordBuilder

### 6. File Size and Timestamp Updates
**Priority: MEDIUM**
- **Location:** `writer_ops.rs:88`
- **Issue:** STANDARD_INFORMATION not updated after writes
- **Impact:** File metadata doesn't reflect changes
- **Solution:**
  - Update size in STANDARD_INFORMATION attribute
  - Update modification timestamps
  - Handle both FILE_NAME and STANDARD_INFO updates

## 📋 Implementation Order

```
Phase 1: Core Functionality (Required for basic usability)
├── 1. Resident Data Modification 
├── 2. Directory Creation
└── 3. Subdirectory Support

Phase 2: Robustness (Required for production)
├── 4. Volume Bitmap Persistence
├── 5. Non-Resident Data Allocation  
└── 6. Metadata Updates

Phase 3: Advanced Features (Nice to have)
├── Sparse file support
├── Compressed files
├── Encrypted files
└── Hard links
```

## 🧪 Test Cases Needed

1. **Resident Data Test**
   - Create small file (<700 bytes)
   - Write content
   - Read back and verify

2. **Directory Test**
   - Create directory
   - Create file in directory
   - List directory contents

3. **Nested Structure Test**
   - Create `/dir1/dir2/file.txt`
   - Write and read from nested file
   - Navigate directory tree

4. **Large File Test**
   - Create file >1MB
   - Write data at various offsets
   - Verify data integrity

## 🎯 Next Steps

Start with **Resident Data Modification** as it's the most critical gap:

1. Add method to update resident DATA attribute value
2. Implement MFT record attribute replacement 
3. Test with small text files
4. Move to directory creation once small files work