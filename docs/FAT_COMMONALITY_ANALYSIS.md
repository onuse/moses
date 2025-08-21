# FAT16/FAT32 Commonality Analysis

## Common Components Between FAT16 and FAT32

### 1. Boot Sector Structure (Partial)
Both share the first 36 bytes of the BPB (BIOS Parameter Block):
- Jump instruction (3 bytes)
- OEM Name (8 bytes)  
- Bytes per sector (2 bytes)
- Sectors per cluster (1 byte)
- Reserved sectors (2 bytes)
- Number of FATs (1 byte)
- Root entries (2 bytes) - **0 for FAT32**
- Total sectors 16 (2 bytes)
- Media descriptor (1 byte)
- Sectors per FAT16 (2 bytes) - **0 for FAT32**
- Sectors per track (2 bytes)
- Number of heads (2 bytes)
- Hidden sectors (4 bytes)
- Total sectors 32 (4 bytes)

### 2. Differences in Boot Sector
**FAT16 Extended BPB (offset 36-62):**
- Drive number (1 byte)
- Reserved (1 byte)
- Boot signature (1 byte)
- Volume ID (4 bytes)
- Volume label (11 bytes)
- Filesystem type (8 bytes) "FAT16   "

**FAT32 Extended BPB (offset 36-90):**
- Sectors per FAT32 (4 bytes)
- Extended flags (2 bytes)
- FS version (2 bytes)
- Root cluster (4 bytes)
- FS info sector (2 bytes)
- Backup boot sector (2 bytes)
- Reserved (12 bytes)
- Drive number (1 byte)
- Reserved (1 byte)
- Boot signature (1 byte)
- Volume ID (4 bytes)
- Volume label (11 bytes)
- Filesystem type (8 bytes) "FAT32   "

### 3. Common Operations

#### Cluster Size Calculation
```rust
fn calculate_cluster_size(total_sectors: u64) -> u8 {
    // Similar logic, different thresholds
    // FAT16: max 65524 clusters
    // FAT32: min 65525 clusters
}
```

#### FAT Table Writing
- FAT16: 16-bit entries
- FAT32: 28-bit entries (upper 4 bits reserved)

#### Volume Serial Number Generation
```rust
fn generate_volume_serial() -> u32 {
    // Same for both
    SystemTime::now()...
}
```

#### MBR/Partition Table Creation
- Same partitioner can be used
- Different partition type codes (0x0B/0x0C for FAT32)

## Proposed Modular Architecture

### 1. Create `fat_common` Module
```rust
// fat_common/mod.rs
pub mod boot_sector;
pub mod cluster_calc;
pub mod volume_serial;
pub mod fat_constants;

pub struct FatCommonParams {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub media_descriptor: u8,
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub volume_serial: u32,
    pub volume_label: [u8; 11],
}
```

### 2. Shared Boot Sector Builder
```rust
pub struct BootSectorBuilder {
    common: FatCommonParams,
}

impl BootSectorBuilder {
    pub fn new() -> Self { ... }
    pub fn with_oem_name(mut self, name: &str) -> Self { ... }
    pub fn with_volume_label(mut self, label: &str) -> Self { ... }
    
    pub fn build_fat16(self, fat16_params: Fat16Specific) -> [u8; 512] { ... }
    pub fn build_fat32(self, fat32_params: Fat32Specific) -> [u8; 512] { ... }
}
```

### 3. FAT Table Writer Trait
```rust
pub trait FatTableWriter {
    fn write_fat_entry(&mut self, cluster: u32, value: u32);
    fn mark_end_of_chain(&mut self, cluster: u32);
    fn mark_bad_cluster(&mut self, cluster: u32);
}

pub struct Fat16TableWriter { ... }
pub struct Fat32TableWriter { ... }
```

### 4. Cluster Calculator
```rust
pub struct ClusterCalculator;

impl ClusterCalculator {
    pub fn calculate_for_fat16(total_sectors: u64) -> Result<FatParams, Error> {
        // Must result in 4085-65524 clusters
    }
    
    pub fn calculate_for_fat32(total_sectors: u64) -> Result<FatParams, Error> {
        // Must result in >= 65525 clusters
    }
}
```

## Benefits of Modularization

1. **Code Reuse**: ~40% of code is identical between FAT16/FAT32
2. **Consistency**: Same calculations, same results
3. **Maintainability**: Fix bugs in one place
4. **Testing**: Shared test utilities
5. **Future FAT12**: Easy to add with same architecture

## Implementation Order

1. ✅ Extract common constants to `fat_constants.rs`
2. ✅ Create `FatCommonParams` struct
3. ✅ Build shared boot sector builder
4. ✅ Implement FAT32 using shared components
5. ✅ Refactor FAT16 to use shared components
6. ✅ Test both implementations