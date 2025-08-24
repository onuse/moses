# NTFS Implementation Roadmap for Moses

## Executive Summary
NTFS (New Technology File System) is Microsoft's proprietary filesystem, introduced with Windows NT 3.1 in 1993. It's significantly more complex than FAT filesystems, featuring journaling, encryption, compression, permissions, and many other advanced features. This roadmap outlines a phased approach to implementing NTFS support in Moses.

## Core NTFS Concepts

### 1. Everything is a File
In NTFS, everything is stored as files in the Master File Table (MFT), including metadata:
- `$MFT` - Master File Table itself
- `$MFTMirr` - MFT mirror (backup of first 4 records)
- `$LogFile` - Transaction journal
- `$Volume` - Volume information
- `$AttrDef` - Attribute definitions
- `$Root` - Root directory
- `$Bitmap` - Cluster allocation bitmap
- `$Boot` - Boot sector
- `$BadClus` - Bad cluster list
- `$Secure` - Security descriptors
- `$UpCase` - Unicode uppercase table
- `$Extend` - Extended metadata directory

### 2. MFT Records
- Each file/directory has an MFT record (1024 bytes typically)
- Records contain attributes, not raw data
- Small files (<700 bytes) stored entirely in MFT (resident data)
- Large files use extents (runs) pointing to clusters

### 3. Attributes
NTFS stores everything as attributes:
- `$STANDARD_INFORMATION` (0x10) - Timestamps, basic permissions
- `$ATTRIBUTE_LIST` (0x20) - For records spanning multiple MFT entries
- `$FILE_NAME` (0x30) - Filename(s) - can have multiple (8.3 + long)
- `$OBJECT_ID` (0x40) - Unique identifier
- `$SECURITY_DESCRIPTOR` (0x50) - ACLs and permissions
- `$VOLUME_NAME` (0x60) - Volume label
- `$VOLUME_INFORMATION` (0x70) - NTFS version
- `$DATA` (0x80) - File contents
- `$INDEX_ROOT` (0x90) - B-tree root for directories
- `$INDEX_ALLOCATION` (0xA0) - B-tree nodes for large directories
- `$BITMAP` (0xB0) - Allocation bitmap for indexes
- `$REPARSE_POINT` (0xC0) - Symbolic links, mount points
- `$EA_INFORMATION` (0xD0) - Extended attributes info
- `$EA` (0xE0) - Extended attributes
- `$LOGGED_UTILITY_STREAM` (0x100) - EFS, transactions

### 4. Runlists (Extents)
- Non-resident data stored as runlists
- Compressed format: [header byte][length bytes][offset bytes]
- Supports sparse files and compression

### 5. Indexes (B+ Trees)
- Directories use B+ trees for scalability
- Sorted by filename for fast lookups
- Can be resident (small dirs) or non-resident (large dirs)

## Implementation Phases

### Phase 1: Read-Only Support (Core)
**Goal**: Read files and directories from NTFS volumes

#### 1.1 Boot Sector & Basic Detection
```rust
// structures/boot_sector.rs
- Parse NTFS boot sector (BIOS Parameter Block)
- Validate NTFS signature ("NTFS    ")
- Extract cluster size, MFT location, MFTMirr location
- Calculate volume size and geometry
```

#### 1.2 MFT Record Parser
```rust
// structures/mft_record.rs
- Parse MFT record header (FILE_RECORD_HEADER)
- Validate multi-sector signatures ("FILE" or "BAAD")
- Apply USA (Update Sequence Array) fixups
- Parse attribute headers and enumerate attributes
```

#### 1.3 Attribute Parsers
```rust
// structures/attributes.rs
- Implement parsers for core attributes:
  - STANDARD_INFORMATION (timestamps)
  - FILE_NAME (all three namespaces: POSIX, Win32, DOS)
  - DATA (resident and non-resident)
  - INDEX_ROOT, INDEX_ALLOCATION (directories)
```

#### 1.4 Runlist Decoder
```rust
// structures/runlist.rs
- Decode compressed runlist format
- Handle sparse runs (virtual cluster numbers)
- Build cluster chain for reading
```

#### 1.5 Basic Reader
```rust
// reader.rs
- Implement FilesystemReader trait
- Read root directory from MFT record 5
- Navigate directory B-trees
- Read file data (resident and non-resident)
- Handle small files in MFT
```

### Phase 2: Advanced Read Support
**Goal**: Handle complex NTFS features for reading

#### 2.1 Compressed Files
```rust
// compression.rs
- LZNT1 decompression (NTFS native compression)
- Handle compression units (16 clusters)
- Sparse file support
```

#### 2.2 Attribute Lists
```rust
// For files spanning multiple MFT records
- Parse ATTRIBUTE_LIST
- Follow references to other MFT records
- Merge attributes from multiple records
```

#### 2.3 Reparse Points
```rust
// reparse.rs
- Symbolic links
- Junction points
- Mount points
- Detect and report OneDrive/Dropbox placeholders
```

#### 2.4 Security & Permissions
```rust
// security.rs
- Parse $Secure file
- Decode Security Descriptors
- Report file permissions (read-only for now)
```

#### 2.5 Unicode Support
```rust
// unicode.rs
- Load $UpCase table
- Implement NTFS case-insensitive comparison
- Handle all three filename namespaces
```

### Phase 3: Write Support (Basic)
**Goal**: Create and modify files on NTFS

#### 3.1 Journal ($LogFile)
```rust
// journal.rs
- Parse $LogFile structure
- Implement transaction support
- Write journal records for operations
- Recovery/rollback mechanisms
```

#### 3.2 Bitmap Management
```rust
// bitmap.rs
- Read/write $Bitmap
- Allocate/free clusters
- Find contiguous space
```

#### 3.3 MFT Management
```rust
// mft_manager.rs
- Allocate new MFT records
- Extend MFT when full
- Update MFT mirror
```

#### 3.4 File Creation
```rust
// write_operations.rs
- Create MFT records
- Write resident data
- Allocate clusters for non-resident data
- Update directory indexes
```

#### 3.5 File Modification
```rust
- Extend/truncate files
- Convert resident <-> non-resident
- Update timestamps
- Maintain journal consistency
```

### Phase 4: Advanced Write Support
**Goal**: Full NTFS feature support

#### 4.1 Directory Operations
```rust
- Create/delete directories
- Rebalance B-trees
- Handle large directories
```

#### 4.2 Hard Links
```rust
- Multiple FILE_NAME attributes
- Reference counting
- Maintain consistency
```

#### 4.3 Compression
```rust
- LZNT1 compression
- Transparent compression/decompression
- Compression unit management
```

#### 4.4 Sparse Files
```rust
- Sparse run encoding
- Hole punching
- Efficient space usage
```

### Phase 5: Format Support
**Goal**: Create new NTFS volumes

#### 5.1 Basic Formatter
```rust
// formatter.rs
- Create boot sector
- Initialize MFT with system files
- Create root directory
- Set up $Bitmap and $BadClus
```

#### 5.2 Advanced Format Options
```rust
- Cluster size selection
- MFT zone reservation
- Quick vs full format
- Bad sector scanning
```

### Phase 6: Advanced Features (Optional)
**Goal**: Enterprise and advanced features

#### 6.1 Encryption (EFS)
```rust
- Encrypted File System support
- Key management
- Transparent encryption/decryption
```

#### 6.2 Quotas
```rust
- User quotas from $Quota
- Usage tracking
- Enforcement policies
```

#### 6.3 Change Journal
```rust
- USN (Update Sequence Number) journal
- Track all changes
- Used by backup/sync software
```

#### 6.4 Volume Shadow Copy
```rust
- VSS integration
- Previous versions
- Snapshot support
```

## Technical Challenges

### 1. Proprietary Format
- No official documentation from Microsoft
- Reverse-engineered specifications may have gaps
- Behavior changes between Windows versions

### 2. Complexity
- Many interdependent structures
- Multiple ways to store same information
- Edge cases and legacy compatibility

### 3. Journaling
- Must maintain consistency
- Proper transaction handling
- Recovery from partial writes

### 4. Performance
- B-tree operations
- Efficient runlist decoding
- Caching strategies
- Large directory handling

### 5. Compatibility
- Windows version differences (XP/7/8/10/11)
- NTFS versions (1.2, 3.0, 3.1)
- Undocumented features

## Development Strategy

### Testing Approach
1. **Unit Tests**: Each parser/structure independently
2. **Integration Tests**: Read known NTFS images
3. **Fuzzing**: Handle malformed data gracefully
4. **Real Device Testing**: Various Windows versions
5. **Comparison Testing**: Against Windows chkdsk/fsutil

### Reference Implementation Study
- **NTFS-3G**: Linux NTFS driver (GPL)
- **ntfsprogs**: NTFS utilities
- **Windows DDK**: Documentation and headers
- **Linux kernel NTFS**: Read-only implementation
- **Rust ntfs crate**: Existing Rust implementation

### Validation Tools
- Windows `chkdsk` for consistency checking
- `fsutil` for NTFS operations
- WinHex/HxD for hex comparison
- Windows Event Viewer for NTFS errors

## Resource Requirements

### Documentation
- "Windows Internals" books
- Linux NTFS documentation
- Forensics guides (Brian Carrier's work)
- Reverse engineering notes

### Test Data
- Various NTFS volume images
- Different cluster sizes (512B - 64KB)
- Compressed/encrypted files
- Sparse files
- Large directories (>10000 files)
- Junction points and symlinks

### Development Time Estimate
- Phase 1 (Basic Read): 3-4 weeks
- Phase 2 (Advanced Read): 2-3 weeks  
- Phase 3 (Basic Write): 4-6 weeks
- Phase 4 (Advanced Write): 3-4 weeks
- Phase 5 (Format): 2-3 weeks
- Phase 6 (Optional): 4-6 weeks

**Total: 3-5 months for full implementation**

## Risk Mitigation

### Legal Considerations
- No Microsoft code or documentation used
- Clean-room implementation
- Based on publicly available information
- Similar to NTFS-3G approach

### Data Safety
- Read-only mode by default
- Extensive testing before write support
- Clear warnings for experimental features
- Backup recommendations

### Compatibility Testing
- Test matrix for Windows versions
- Automated regression testing
- Community beta testing
- Gradual rollout of features

## Success Criteria

### Phase 1 Complete When:
- Can list all files/directories
- Can read any file content
- Handles 95% of real-world NTFS volumes
- No crashes on malformed data

### Phase 3 Complete When:
- Can create/modify files
- Windows accepts modified volumes
- Journal maintains consistency
- No data corruption

### Phase 5 Complete When:
- Can format new NTFS volumes
- Windows recognizes formatted volumes
- Can boot Windows from Moses-formatted NTFS

## Alternatives Consideration

### Option 1: Wrapper Around Existing Tools
- Use Windows API on Windows
- Use NTFS-3G on Linux/Mac
- Pros: Faster, more reliable
- Cons: Platform-specific, less control

### Option 2: Limited Implementation
- Read-only support only
- No write/format support
- Pros: Safer, simpler
- Cons: Less useful

### Option 3: Port Existing Implementation
- Port NTFS-3G to Rust
- Or use existing Rust ntfs crate
- Pros: Proven code
- Cons: License issues, dependencies

## Recommendation

Start with **Phase 1** (Basic Read Support) as it provides immediate value with lower risk. This allows users to browse and copy files from NTFS drives. Progress through phases based on user demand and testing results.

Consider using existing Rust crates (like `ntfs`) for reference or as a starting point, while ensuring license compatibility.

The most critical aspect is extensive testing - NTFS corruption can make Windows unbootable, so reliability must be the top priority over features.