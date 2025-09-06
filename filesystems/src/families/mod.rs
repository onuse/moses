// Filesystem Families Organization
// Groups related filesystems together for code reuse and better organization

pub mod fat;
pub mod ext;
pub mod ntfs;

// Future filesystem families
// pub mod bsd;    // FFS/UFS family
// pub mod flash;  // JFFS2/YAFFS/UBIFS
// pub mod optical; // ISO9660/UDF

use moses_core::MosesError;

/// Common trait for filesystem families
pub trait FilesystemFamily {
    /// Name of the filesystem family (e.g., "FAT", "ext", "NTFS")
    fn family_name(&self) -> &str;
    
    /// List of filesystem variants in this family
    fn variants(&self) -> Vec<String>;
    
    /// Common magic signatures for this family
    fn family_signatures(&self) -> Vec<FamilySignature>;
}

/// Signature information for a filesystem family
#[derive(Debug, Clone)]
pub struct FamilySignature {
    /// Offset in the device where signature appears
    pub offset: u64,
    /// The signature bytes
    pub signature: Vec<u8>,
    /// Which variant this signature indicates
    pub variant_hint: Option<String>,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f32,
}

/// Common metadata for filesystem families
#[derive(Debug, Clone)]
pub struct FamilyMetadata {
    /// When this filesystem family was introduced
    pub era_start: u32,
    /// When it became obsolete (if applicable)
    pub era_end: Option<u32>,
    /// Typical block/cluster sizes
    pub common_block_sizes: Vec<u32>,
    /// Maximum volume size across variants
    pub max_volume_size: u64,
    /// Whether this family supports journaling
    pub supports_journaling: bool,
    /// Whether this family supports compression
    pub supports_compression: bool,
}

/// Trait for shared operations within a filesystem family
pub trait FamilyOperations {
    /// Read the superblock/boot sector in a family-specific way
    fn read_metadata(&self, device: &mut dyn std::io::Read) -> Result<Vec<u8>, MosesError>;
    
    /// Validate that this device contains a filesystem from this family
    fn validate_family(&self, device: &mut dyn std::io::Read) -> Result<bool, MosesError>;
    
    /// Detect which specific variant within the family
    fn detect_variant(&self, device: &mut dyn std::io::Read) -> Result<String, MosesError>;
}