# NTFS Writer - Remaining Implementation Tasks

## ✅ Completed
- [x] Core writer infrastructure with transaction management
- [x] MFT record creation and serialization (MftRecordBuilder)
- [x] MFT record write-back to disk
- [x] Windows FILETIME timestamp handling
- [x] Boot sector and formatter implementation
- [x] Bitmap management (MFT and volume)
- [x] Basic file creation and deletion
- [x] Non-resident data writing (clusters)
- [x] Safety checks and verification

## 🚧 Critical - Required for Basic Functionality

### 1. Directory Index Updates (HIGH PRIORITY)
**File:** `writer_ops_ext.rs`, `index_writer.rs`
**Issue:** Files created don't appear in directory listings
**Tasks:**
- [ ] Implement actual INDEX_ROOT attribute updates
- [ ] Parse and modify B+ tree structure
- [ ] Sort entries correctly (Unicode collation)
- [ ] Handle INDEX_ALLOCATION for large directories
- [ ] Persist changes back to MFT record

### 2. MFT Record Reading for Arbitrary Records
**File:** `mft.rs`, `writer.rs`
**Issue:** Can only read MFT record 0, need to read any record
**Tasks:**
- [ ] Implement `read_mft_record_by_number()` in MftReader
- [ ] Handle MFT fragmentation (multiple data runs)
- [ ] Cache frequently accessed records

## 📝 Important - Core Features

### 3. Resident Data Modification
**File:** `writer_ops.rs`
**Issue:** Can't modify small files stored directly in MFT
**Tasks:**
- [ ] Implement resident data update in MFT record
- [ ] Handle resident to non-resident conversion
- [ ] Update file size in STANDARD_INFORMATION

### 4. Directory Creation
**File:** `writer_ops.rs`, `ops_rw_v2.rs`
**Issue:** Method exists but returns NotSupported
**Tasks:**
- [ ] Create directory MFT record with INDEX_ROOT
- [ ] Initialize empty B+ tree index
- [ ] Add "." and ".." entries
- [ ] Update parent directory index

### 5. Subdirectory Navigation
**File:** `ops_rw_v2.rs`, `path_resolver.rs`
**Issue:** Only root directory operations work
**Tasks:**
- [ ] Integrate PathResolver with writer
- [ ] Handle multi-level path resolution
- [ ] Cache directory MFT mappings

## 🔧 Enhancements - Quality of Life

### 6. Volume Bitmap Persistence
**File:** `writer.rs`
**Issue:** Bitmap changes not written back to $Bitmap
**Tasks:**
- [ ] Write updated bitmap to $Bitmap DATA attribute
- [ ] Handle bitmap growth for volume expansion

### 7. File Attribute Updates
**File:** `writer_ops.rs`
**Issue:** Timestamps and sizes not updated on write
**Tasks:**
- [ ] Update STANDARD_INFORMATION on file changes
- [ ] Update FILE_NAME attributes
- [ ] Handle attribute list for complex files

### 8. Non-Resident Data Allocation
**File:** `mft_writer.rs`, `writer_ops.rs`
**Issue:** Can't create files with pre-allocated space
**Tasks:**
- [ ] Allocate clusters for new files
- [ ] Create proper data runs
- [ ] Update volume bitmap

## 🧪 Testing Requirements

### Test Scenarios Needed:
1. **Basic Operations**
   - [ ] Format volume and verify with Windows
   - [ ] Create file and verify it appears in listing
   - [ ] Write data and read it back
   - [ ] Delete file and verify removal

2. **Edge Cases**
   - [ ] Large directory with 100+ files
   - [ ] Resident to non-resident conversion
   - [ ] Fragmented files
   - [ ] Unicode filenames

3. **Compatibility**
   - [ ] Mount on Windows and verify
   - [ ] Mount on Linux (ntfs-3g) and verify
   - [ ] Check with chkdsk/fsck

## 📚 Implementation Notes

### Directory Index Structure
- NTFS uses B+ trees for directory indexes
- Entries sorted by Unicode collation rules
- Small dirs: INDEX_ROOT (resident in MFT)
- Large dirs: INDEX_ALLOCATION (non-resident)

### Key Data Structures
```rust
// Index entry format (simplified)
struct IndexEntry {
    mft_reference: u64,    // 6 bytes ref + 2 bytes sequence
    entry_length: u16,
    stream_length: u16,
    flags: u32,            // 0x01 = has subnode, 0x02 = last
    parent_reference: u64,
    // FILE_NAME attribute follows
}
```

### Priority Order for Implementation
1. Directory index updates (makes files visible)
2. MFT record reading (enables directory updates)
3. Resident data (completes basic file ops)
4. Directory creation (full filesystem ops)
5. Subdirectory support (complete navigation)

## 🎯 Next Steps

1. Start with implementing `read_mft_record_by_number()` in `mft.rs`
2. Complete the directory index update in `writer_ops_ext.rs`
3. Test with a simple file creation and verify with hex editor
4. Iterate on the B+ tree manipulation logic

The foundation is solid - these remaining pieces will complete a functional NTFS writer!