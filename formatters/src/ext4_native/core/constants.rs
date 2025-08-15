// EXT4 Filesystem Constants
// All constants from the ext4 specification

// Magic numbers
pub const EXT4_SUPER_MAGIC: u16 = 0xEF53;
pub const EXT4_EXTENT_MAGIC: u16 = 0xF30A;
pub const JBD2_MAGIC_NUMBER: u32 = 0xC03B3998;

// Block sizes
pub const EXT4_MIN_BLOCK_SIZE: u32 = 1024;
pub const EXT4_MAX_BLOCK_SIZE: u32 = 65536;
pub const EXT4_DEFAULT_BLOCK_SIZE: u32 = 4096;

// Filesystem limits
pub const EXT4_BLOCKS_PER_GROUP: u32 = 32768;
pub const EXT4_INODES_PER_GROUP: u32 = 8192;
pub const EXT4_DEFAULT_INODE_SIZE: u16 = 256;
pub const EXT4_GOOD_OLD_INODE_SIZE: u16 = 128;

// Special inode numbers
pub const EXT4_BAD_INO: u32 = 1;          // Bad blocks inode
pub const EXT4_ROOT_INO: u32 = 2;         // Root directory inode
pub const EXT4_ACL_IDX_INO: u32 = 3;      // ACL index inode (deprecated)
pub const EXT4_ACL_DATA_INO: u32 = 4;     // ACL data inode (deprecated)
pub const EXT4_BOOT_LOADER_INO: u32 = 5;  // Boot loader inode
pub const EXT4_UNDEL_DIR_INO: u32 = 6;    // Undelete directory inode
pub const EXT4_RESIZE_INO: u32 = 7;       // Resize inode
pub const EXT4_JOURNAL_INO: u32 = 8;      // Journal inode
pub const EXT4_EXCLUDE_INO: u32 = 9;      // Exclude inode
pub const EXT4_REPLICA_INO: u32 = 10;     // Replica inode
pub const EXT4_FIRST_INO: u32 = 11;       // First non-reserved inode

// Feature flags - Compatible
pub const EXT4_FEATURE_COMPAT_DIR_PREALLOC: u32 = 0x0001;
pub const EXT4_FEATURE_COMPAT_IMAGIC_INODES: u32 = 0x0002;
pub const EXT4_FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;
pub const EXT4_FEATURE_COMPAT_EXT_ATTR: u32 = 0x0008;
pub const EXT4_FEATURE_COMPAT_RESIZE_INODE: u32 = 0x0010;
pub const EXT4_FEATURE_COMPAT_DIR_INDEX: u32 = 0x0020;
pub const EXT4_FEATURE_COMPAT_SPARSE_SUPER2: u32 = 0x0200;

// Feature flags - Incompatible
pub const EXT4_FEATURE_INCOMPAT_COMPRESSION: u32 = 0x0001;
pub const EXT4_FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
pub const EXT4_FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
pub const EXT4_FEATURE_INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
pub const EXT4_FEATURE_INCOMPAT_META_BG: u32 = 0x0010;
pub const EXT4_FEATURE_INCOMPAT_EXTENTS: u32 = 0x0040;
pub const EXT4_FEATURE_INCOMPAT_64BIT: u32 = 0x0080;
pub const EXT4_FEATURE_INCOMPAT_MMP: u32 = 0x0100;
pub const EXT4_FEATURE_INCOMPAT_FLEX_BG: u32 = 0x0200;
pub const EXT4_FEATURE_INCOMPAT_EA_INODE: u32 = 0x0400;
pub const EXT4_FEATURE_INCOMPAT_DIRDATA: u32 = 0x1000;
pub const EXT4_FEATURE_INCOMPAT_CSUM_SEED: u32 = 0x2000;
pub const EXT4_FEATURE_INCOMPAT_LARGEDIR: u32 = 0x4000;
pub const EXT4_FEATURE_INCOMPAT_INLINE_DATA: u32 = 0x8000;
pub const EXT4_FEATURE_INCOMPAT_ENCRYPT: u32 = 0x10000;

// Feature flags - Read-only compatible
pub const EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
pub const EXT4_FEATURE_RO_COMPAT_LARGE_FILE: u32 = 0x0002;
pub const EXT4_FEATURE_RO_COMPAT_BTREE_DIR: u32 = 0x0004;
pub const EXT4_FEATURE_RO_COMPAT_HUGE_FILE: u32 = 0x0008;
pub const EXT4_FEATURE_RO_COMPAT_GDT_CSUM: u32 = 0x0010;
pub const EXT4_FEATURE_RO_COMPAT_DIR_NLINK: u32 = 0x0020;
pub const EXT4_FEATURE_RO_COMPAT_EXTRA_ISIZE: u32 = 0x0040;
pub const EXT4_FEATURE_RO_COMPAT_QUOTA: u32 = 0x0100;
pub const EXT4_FEATURE_RO_COMPAT_BIGALLOC: u32 = 0x0200;
pub const EXT4_FEATURE_RO_COMPAT_METADATA_CSUM: u32 = 0x0400;
pub const EXT4_FEATURE_RO_COMPAT_REPLICA: u32 = 0x0800;
pub const EXT4_FEATURE_RO_COMPAT_READONLY: u32 = 0x1000;
pub const EXT4_FEATURE_RO_COMPAT_PROJECT: u32 = 0x2000;

// Filesystem states
pub const EXT4_VALID_FS: u16 = 0x0001;    // Cleanly unmounted
pub const EXT4_ERROR_FS: u16 = 0x0002;    // Errors detected
pub const EXT4_ORPHAN_FS: u16 = 0x0004;   // Orphans being recovered

// Error handling behaviors
pub const EXT4_ERRORS_CONTINUE: u16 = 1;  // Continue on errors
pub const EXT4_ERRORS_RO: u16 = 2;        // Remount read-only on errors
pub const EXT4_ERRORS_PANIC: u16 = 3;     // Panic on errors

// Creator OS codes
pub const EXT4_OS_LINUX: u32 = 0;
pub const EXT4_OS_HURD: u32 = 1;
pub const EXT4_OS_MASIX: u32 = 2;
pub const EXT4_OS_FREEBSD: u32 = 3;
pub const EXT4_OS_LITES: u32 = 4;

// Revision levels
pub const EXT4_GOOD_OLD_REV: u32 = 0;     // Original format
pub const EXT4_DYNAMIC_REV: u32 = 1;      // Dynamic format

// Inode flags
pub const EXT4_SECRM_FL: u32 = 0x00000001;        // Secure deletion
pub const EXT4_UNRM_FL: u32 = 0x00000002;         // Undelete
pub const EXT4_COMPR_FL: u32 = 0x00000004;        // Compress file
pub const EXT4_SYNC_FL: u32 = 0x00000008;         // Synchronous updates
pub const EXT4_IMMUTABLE_FL: u32 = 0x00000010;    // Immutable file
pub const EXT4_APPEND_FL: u32 = 0x00000020;       // Append only
pub const EXT4_NODUMP_FL: u32 = 0x00000040;       // No dump
pub const EXT4_NOATIME_FL: u32 = 0x00000080;      // No atime updates
pub const EXT4_INDEX_FL: u32 = 0x00001000;        // Hash indexed directory
pub const EXT4_JOURNAL_DATA_FL: u32 = 0x00004000; // Journal file data
pub const EXT4_NOTAIL_FL: u32 = 0x00008000;       // No tail merging
pub const EXT4_DIRSYNC_FL: u32 = 0x00010000;      // Directory sync
pub const EXT4_TOPDIR_FL: u32 = 0x00020000;       // Top of directory tree
pub const EXT4_HUGE_FILE_FL: u32 = 0x00040000;    // Huge file
pub const EXT4_EXTENTS_FL: u32 = 0x00080000;      // Inode uses extents
pub const EXT4_EA_INODE_FL: u32 = 0x00200000;     // Inode for EA
pub const EXT4_EOFBLOCKS_FL: u32 = 0x00400000;    // EOF blocks
pub const EXT4_INLINE_DATA_FL: u32 = 0x10000000;  // Inline data
pub const EXT4_PROJINHERIT_FL: u32 = 0x20000000;  // Project inherit
pub const EXT4_RESERVED_FL: u32 = 0x80000000;     // Reserved

// File types for directory entries
pub const EXT4_FT_UNKNOWN: u8 = 0;
pub const EXT4_FT_REG_FILE: u8 = 1;
pub const EXT4_FT_DIR: u8 = 2;
pub const EXT4_FT_CHRDEV: u8 = 3;
pub const EXT4_FT_BLKDEV: u8 = 4;
pub const EXT4_FT_FIFO: u8 = 5;
pub const EXT4_FT_SOCK: u8 = 6;
pub const EXT4_FT_SYMLINK: u8 = 7;

// Block group flags
pub const EXT4_BG_INODE_UNINIT: u16 = 0x0001;  // Inode table/bitmap not initialized
pub const EXT4_BG_BLOCK_UNINIT: u16 = 0x0002;  // Block bitmap not initialized
pub const EXT4_BG_INODE_ZEROED: u16 = 0x0004;  // Inode table zeroed

// Inode mode bits
pub const S_IFMT: u16 = 0xF000;   // Format mask
pub const S_IFSOCK: u16 = 0xC000; // Socket
pub const S_IFLNK: u16 = 0xA000;  // Symbolic link
pub const S_IFREG: u16 = 0x8000;  // Regular file
pub const S_IFBLK: u16 = 0x6000;  // Block device
pub const S_IFDIR: u16 = 0x4000;  // Directory
pub const S_IFCHR: u16 = 0x2000;  // Character device
pub const S_IFIFO: u16 = 0x1000;  // FIFO

// Permission bits
pub const S_ISUID: u16 = 0x0800;  // Set UID
pub const S_ISGID: u16 = 0x0400;  // Set GID
pub const S_ISVTX: u16 = 0x0200;  // Sticky bit
pub const S_IRUSR: u16 = 0x0100;  // User read
pub const S_IWUSR: u16 = 0x0080;  // User write
pub const S_IXUSR: u16 = 0x0040;  // User execute
pub const S_IRGRP: u16 = 0x0020;  // Group read
pub const S_IWGRP: u16 = 0x0010;  // Group write
pub const S_IXGRP: u16 = 0x0008;  // Group execute
pub const S_IROTH: u16 = 0x0004;  // Other read
pub const S_IWOTH: u16 = 0x0002;  // Other write
pub const S_IXOTH: u16 = 0x0001;  // Other execute

// Default values
pub const EXT4_DEFAULT_RESERVED_BLOCKS_PERCENT: u32 = 5;
pub const EXT4_DEFAULT_HASH_VERSION: u8 = 1; // Half MD4
pub const EXT4_DEFAULT_MOUNT_OPTS: u32 = 0;
pub const EXT4_DEFAULT_ERRORS: u16 = EXT4_ERRORS_CONTINUE;