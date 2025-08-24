#!/usr/bin/env python3
"""
Deep EXT4 Filesystem Analyzer and Comparison Tool

Performs comprehensive analysis to identify mount issues that basic superblock
checks might miss. Focuses on structures that can cause "Structure needs cleaning" errors.
"""

import sys
import struct
import datetime
import hashlib
from typing import BinaryIO, Dict, Any, Optional, List, Tuple

class Ext4DeepAnalyzer:
    def __init__(self, file: BinaryIO):
        self.file = file
        self.sb = None
        self.block_size = None
        self.is_windows_device = False
        self.cache = {}  # Cache for Windows sequential reads
        
        # Check if this is a Windows raw device
        if sys.platform == 'win32' and hasattr(file, 'name'):
            if isinstance(file.name, str) and file.name.startswith('\\\\'):
                self.is_windows_device = True
                print("Note: Windows raw device detected, using sequential read mode")
        
    def read_at(self, offset: int, size: int) -> bytes:
        """Read bytes at specific offset"""
        if self.is_windows_device:
            # For Windows raw devices, use sequential reading with caching
            return self.read_sequential(offset, size)
        else:
            # Normal file/device - use seek
            try:
                self.file.seek(offset)
                return self.file.read(size)
            except OSError as e:
                print(f"DEBUG: Failed to seek to offset {offset} (0x{offset:X})")
                print(f"DEBUG: Requested size: {size}")
                raise e
    
    def read_sequential(self, offset: int, size: int) -> bytes:
        """Read from Windows device using sequential access"""
        # Check cache first
        cache_key = (offset, size)
        if cache_key in self.cache:
            return self.cache[cache_key]
        
        # For Windows devices, we need to read from the beginning
        # This is inefficient but necessary for raw device access on Windows
        try:
            self.file.seek(0)
            
            # Read in chunks up to the desired offset
            chunk_size = 65536  # 64KB chunks
            bytes_to_skip = offset
            
            while bytes_to_skip > 0:
                skip_size = min(chunk_size, bytes_to_skip)
                skipped = self.file.read(skip_size)
                if len(skipped) < skip_size:
                    raise IOError(f"Cannot reach offset {offset}, device ended at {offset - bytes_to_skip + len(skipped)}")
                bytes_to_skip -= skip_size
            
            # Now read the actual data
            data = self.file.read(size)
            if len(data) < size:
                print(f"Warning: Only read {len(data)} bytes instead of {size} at offset {offset}")
            
            # Cache small reads
            if size <= 4096:
                self.cache[cache_key] = data
            
            return data
        except Exception as e:
            print(f"Error reading at offset {offset}: {e}")
            return b''
    
    def parse_superblock(self, offset: int = 1024) -> Dict[str, Any]:
        """Parse ext4 superblock at given offset"""
        data = self.read_at(offset, 1024)
        
        sb = {}
        
        # Basic fields (0x00 - 0x3F)
        sb['s_inodes_count'] = struct.unpack_from('<I', data, 0x00)[0]
        sb['s_blocks_count_lo'] = struct.unpack_from('<I', data, 0x04)[0]
        sb['s_r_blocks_count_lo'] = struct.unpack_from('<I', data, 0x08)[0]
        sb['s_free_blocks_count_lo'] = struct.unpack_from('<I', data, 0x0C)[0]
        sb['s_free_inodes_count'] = struct.unpack_from('<I', data, 0x10)[0]
        sb['s_first_data_block'] = struct.unpack_from('<I', data, 0x14)[0]
        sb['s_log_block_size'] = struct.unpack_from('<I', data, 0x18)[0]
        sb['s_log_cluster_size'] = struct.unpack_from('<I', data, 0x1C)[0]
        sb['s_blocks_per_group'] = struct.unpack_from('<I', data, 0x20)[0]
        sb['s_clusters_per_group'] = struct.unpack_from('<I', data, 0x24)[0]
        sb['s_inodes_per_group'] = struct.unpack_from('<I', data, 0x28)[0]
        sb['s_mtime'] = struct.unpack_from('<I', data, 0x2C)[0]
        sb['s_wtime'] = struct.unpack_from('<I', data, 0x30)[0]
        sb['s_mnt_count'] = struct.unpack_from('<H', data, 0x34)[0]
        sb['s_max_mnt_count'] = struct.unpack_from('<H', data, 0x36)[0]
        sb['s_magic'] = struct.unpack_from('<H', data, 0x38)[0]
        sb['s_state'] = struct.unpack_from('<H', data, 0x3A)[0]
        sb['s_errors'] = struct.unpack_from('<H', data, 0x3C)[0]
        sb['s_minor_rev_level'] = struct.unpack_from('<H', data, 0x3E)[0]
        
        # More fields (0x40 - 0x7F)
        sb['s_lastcheck'] = struct.unpack_from('<I', data, 0x40)[0]
        sb['s_checkinterval'] = struct.unpack_from('<I', data, 0x44)[0]
        sb['s_creator_os'] = struct.unpack_from('<I', data, 0x48)[0]
        sb['s_rev_level'] = struct.unpack_from('<I', data, 0x4C)[0]
        sb['s_def_resuid'] = struct.unpack_from('<H', data, 0x50)[0]
        sb['s_def_resgid'] = struct.unpack_from('<H', data, 0x52)[0]
        sb['s_first_ino'] = struct.unpack_from('<I', data, 0x54)[0]
        sb['s_inode_size'] = struct.unpack_from('<H', data, 0x58)[0]
        sb['s_block_group_nr'] = struct.unpack_from('<H', data, 0x5A)[0]
        sb['s_feature_compat'] = struct.unpack_from('<I', data, 0x5C)[0]
        sb['s_feature_incompat'] = struct.unpack_from('<I', data, 0x60)[0]
        sb['s_feature_ro_compat'] = struct.unpack_from('<I', data, 0x64)[0]
        sb['s_uuid'] = data[0x68:0x78]
        sb['s_volume_name'] = data[0x78:0x88].rstrip(b'\x00')
        sb['s_last_mounted'] = data[0x88:0xC8].rstrip(b'\x00')
        
        # Algorithm usage bitmap
        sb['s_algorithm_usage_bitmap'] = struct.unpack_from('<I', data, 0xC8)[0]
        
        # Performance hints
        sb['s_prealloc_blocks'] = struct.unpack_from('<B', data, 0xCC)[0]
        sb['s_prealloc_dir_blocks'] = struct.unpack_from('<B', data, 0xCD)[0]
        sb['s_reserved_gdt_blocks'] = struct.unpack_from('<H', data, 0xCE)[0]
        
        # Journal fields
        sb['s_journal_uuid'] = data[0xD0:0xE0]
        sb['s_journal_inum'] = struct.unpack_from('<I', data, 0xE0)[0]
        sb['s_journal_dev'] = struct.unpack_from('<I', data, 0xE4)[0]
        sb['s_last_orphan'] = struct.unpack_from('<I', data, 0xE8)[0]
        
        # Hash seed
        sb['s_hash_seed'] = [struct.unpack_from('<I', data, 0xEC + i*4)[0] for i in range(4)]
        
        # Default mount options
        sb['s_def_hash_version'] = struct.unpack_from('<B', data, 0xFC)[0]
        sb['s_jnl_backup_type'] = struct.unpack_from('<B', data, 0xFD)[0]
        sb['s_desc_size'] = struct.unpack_from('<H', data, 0xFE)[0]
        sb['s_default_mount_opts'] = struct.unpack_from('<I', data, 0x100)[0]
        sb['s_first_meta_bg'] = struct.unpack_from('<I', data, 0x104)[0]
        sb['s_mkfs_time'] = struct.unpack_from('<I', data, 0x108)[0]
        
        # Extended fields for ext4
        if sb['s_rev_level'] >= 1 and len(data) >= 0x200:
            sb['s_blocks_count_hi'] = struct.unpack_from('<I', data, 0x150)[0]
            sb['s_r_blocks_count_hi'] = struct.unpack_from('<I', data, 0x154)[0]
            sb['s_free_blocks_count_hi'] = struct.unpack_from('<I', data, 0x158)[0]
            sb['s_min_extra_isize'] = struct.unpack_from('<H', data, 0x15C)[0]
            sb['s_want_extra_isize'] = struct.unpack_from('<H', data, 0x15E)[0]
            sb['s_flags'] = struct.unpack_from('<I', data, 0x160)[0]
            sb['s_raid_stride'] = struct.unpack_from('<H', data, 0x164)[0]
            sb['s_mmp_interval'] = struct.unpack_from('<H', data, 0x166)[0]
            sb['s_mmp_block'] = struct.unpack_from('<Q', data, 0x168)[0]
            sb['s_raid_stripe_width'] = struct.unpack_from('<I', data, 0x170)[0]
            sb['s_log_groups_per_flex'] = struct.unpack_from('<B', data, 0x174)[0]
            sb['s_checksum_type'] = struct.unpack_from('<B', data, 0x175)[0]
            sb['s_encryption_level'] = struct.unpack_from('<B', data, 0x176)[0]
            sb['s_reserved_pad'] = struct.unpack_from('<B', data, 0x177)[0]
            sb['s_kbytes_written'] = struct.unpack_from('<Q', data, 0x178)[0]
            sb['s_snapshot_inum'] = struct.unpack_from('<I', data, 0x180)[0]
            sb['s_snapshot_id'] = struct.unpack_from('<I', data, 0x184)[0]
            sb['s_snapshot_r_blocks_count'] = struct.unpack_from('<Q', data, 0x188)[0]
            sb['s_snapshot_list'] = struct.unpack_from('<I', data, 0x190)[0]
            sb['s_error_count'] = struct.unpack_from('<I', data, 0x194)[0]
            sb['s_first_error_time'] = struct.unpack_from('<I', data, 0x198)[0]
            sb['s_first_error_ino'] = struct.unpack_from('<I', data, 0x19C)[0]
            sb['s_first_error_block'] = struct.unpack_from('<Q', data, 0x1A0)[0]
            sb['s_first_error_func'] = data[0x1A8:0x1C8].rstrip(b'\x00')
            sb['s_first_error_line'] = struct.unpack_from('<I', data, 0x1C8)[0]
            sb['s_last_error_time'] = struct.unpack_from('<I', data, 0x1CC)[0]
            sb['s_last_error_ino'] = struct.unpack_from('<I', data, 0x1D0)[0]
            sb['s_last_error_line'] = struct.unpack_from('<I', data, 0x1D4)[0]
            sb['s_last_error_block'] = struct.unpack_from('<Q', data, 0x1D8)[0]
            sb['s_last_error_func'] = data[0x1E0:0x200].rstrip(b'\x00')
            
            # Checksum
            if sb['s_feature_ro_compat'] & 0x400:  # METADATA_CSUM
                sb['s_checksum'] = struct.unpack_from('<I', data, 0x3FC)[0]
            
        # Calculate derived values
        sb['block_size'] = 1024 << sb['s_log_block_size']
        sb['blocks_count'] = sb['s_blocks_count_lo']
        if 's_blocks_count_hi' in sb:
            sb['blocks_count'] |= (sb['s_blocks_count_hi'] << 32)
            
        return sb
    
    def parse_block_group_descriptor(self, bg_num: int) -> Dict[str, Any]:
        """Parse a block group descriptor"""
        if not self.sb:
            return None
            
        # Calculate descriptor location
        desc_size = self.sb.get('s_desc_size', 32)
        if desc_size == 0:
            desc_size = 32
            
        # Block group descriptors start at block 1 (or 0 if block size > 1024)
        if self.block_size == 1024:
            desc_block = 2
        else:
            desc_block = 1
            
        offset = desc_block * self.block_size + bg_num * desc_size
        data = self.read_at(offset, desc_size)
        
        bg = {}
        bg['bg_block_bitmap_lo'] = struct.unpack_from('<I', data, 0x00)[0]
        bg['bg_inode_bitmap_lo'] = struct.unpack_from('<I', data, 0x04)[0]
        bg['bg_inode_table_lo'] = struct.unpack_from('<I', data, 0x08)[0]
        bg['bg_free_blocks_count_lo'] = struct.unpack_from('<H', data, 0x0C)[0]
        bg['bg_free_inodes_count_lo'] = struct.unpack_from('<H', data, 0x0E)[0]
        bg['bg_used_dirs_count_lo'] = struct.unpack_from('<H', data, 0x10)[0]
        bg['bg_flags'] = struct.unpack_from('<H', data, 0x12)[0]
        bg['bg_exclude_bitmap_lo'] = struct.unpack_from('<I', data, 0x14)[0]
        bg['bg_block_bitmap_csum_lo'] = struct.unpack_from('<H', data, 0x18)[0]
        bg['bg_inode_bitmap_csum_lo'] = struct.unpack_from('<H', data, 0x1A)[0]
        bg['bg_itable_unused_lo'] = struct.unpack_from('<H', data, 0x1C)[0]
        bg['bg_checksum'] = struct.unpack_from('<H', data, 0x1E)[0]
        
        # 64-bit fields if present
        if desc_size >= 64:
            bg['bg_block_bitmap_hi'] = struct.unpack_from('<I', data, 0x20)[0]
            bg['bg_inode_bitmap_hi'] = struct.unpack_from('<I', data, 0x24)[0]
            bg['bg_inode_table_hi'] = struct.unpack_from('<I', data, 0x28)[0]
            bg['bg_free_blocks_count_hi'] = struct.unpack_from('<H', data, 0x2C)[0]
            bg['bg_free_inodes_count_hi'] = struct.unpack_from('<H', data, 0x2E)[0]
            bg['bg_used_dirs_count_hi'] = struct.unpack_from('<H', data, 0x30)[0]
            bg['bg_itable_unused_hi'] = struct.unpack_from('<H', data, 0x32)[0]
            bg['bg_exclude_bitmap_hi'] = struct.unpack_from('<I', data, 0x34)[0]
            bg['bg_block_bitmap_csum_hi'] = struct.unpack_from('<H', data, 0x38)[0]
            bg['bg_inode_bitmap_csum_hi'] = struct.unpack_from('<H', data, 0x3A)[0]
            
        return bg
    
    def verify_block_group_checksum(self, bg_num: int, bg: Dict[str, Any]) -> bool:
        """Verify block group descriptor checksum"""
        if not (self.sb['s_feature_ro_compat'] & 0x10):  # GDT_CSUM not enabled
            return True
            
        # TODO: Implement actual checksum calculation
        # For now, just check if checksum field is non-zero
        return bg.get('bg_checksum', 0) != 0
    
    def check_inode(self, inode_num: int) -> Dict[str, Any]:
        """Check a specific inode"""
        if not self.sb or inode_num < 1:
            return None
            
        # Calculate inode location
        inodes_per_group = self.sb['s_inodes_per_group']
        inode_size = self.sb['s_inode_size']
        
        # Which block group?
        bg_num = (inode_num - 1) // inodes_per_group
        local_inode = (inode_num - 1) % inodes_per_group
        
        # Get block group descriptor
        bg = self.parse_block_group_descriptor(bg_num)
        if not bg:
            return None
            
        # Calculate inode table location
        inode_table_block = bg['bg_inode_table_lo']
        if 'bg_inode_table_hi' in bg:
            inode_table_block |= (bg['bg_inode_table_hi'] << 32)
            
        # Read inode
        offset = inode_table_block * self.block_size + local_inode * inode_size
        
        data = self.read_at(offset, inode_size)
        if not data or len(data) < inode_size:
            return None
        
        inode = {}
        inode['i_mode'] = struct.unpack_from('<H', data, 0x00)[0]
        inode['i_uid'] = struct.unpack_from('<H', data, 0x02)[0]
        inode['i_size_lo'] = struct.unpack_from('<I', data, 0x04)[0]
        inode['i_atime'] = struct.unpack_from('<I', data, 0x08)[0]
        inode['i_ctime'] = struct.unpack_from('<I', data, 0x0C)[0]
        inode['i_mtime'] = struct.unpack_from('<I', data, 0x10)[0]
        inode['i_dtime'] = struct.unpack_from('<I', data, 0x14)[0]
        inode['i_gid'] = struct.unpack_from('<H', data, 0x18)[0]
        inode['i_links_count'] = struct.unpack_from('<H', data, 0x1A)[0]
        inode['i_blocks_lo'] = struct.unpack_from('<I', data, 0x1C)[0]
        inode['i_flags'] = struct.unpack_from('<I', data, 0x20)[0]
        
        return inode
    
    def check_root_inode(self) -> List[str]:
        """Check root inode (inode 2) for issues"""
        issues = []
        
        root_inode = self.check_inode(2)
        if not root_inode:
            issues.append("Cannot read root inode")
            return issues
            
        # Check if it's a directory
        if (root_inode['i_mode'] & 0xF000) != 0x4000:
            issues.append(f"Root inode is not a directory (mode: 0x{root_inode['i_mode']:04X})")
            
        # Check if it has proper permissions
        perms = root_inode['i_mode'] & 0x1FF
        if perms == 0:
            issues.append("Root inode has no permissions")
            
        # Check links count
        if root_inode['i_links_count'] < 2:
            issues.append(f"Root inode links count too low: {root_inode['i_links_count']}")
            
        return issues
    
    def check_orphan_list(self) -> List[str]:
        """Check for orphaned inodes"""
        issues = []
        
        if self.sb['s_last_orphan'] != 0:
            issues.append(f"Orphan list not empty (first orphan: inode {self.sb['s_last_orphan']})")
            
        return issues
    
    def check_error_state(self) -> List[str]:
        """Check for filesystem errors recorded in superblock"""
        issues = []
        
        # Check error count (ext4 specific)
        if 's_error_count' in self.sb and self.sb['s_error_count'] > 0:
            issues.append(f"Filesystem has {self.sb['s_error_count']} recorded errors")
            if self.sb.get('s_first_error_time', 0) > 0:
                first_error = datetime.datetime.fromtimestamp(self.sb['s_first_error_time'])
                issues.append(f"  First error at: {first_error}")
                if self.sb.get('s_first_error_func'):
                    func = self.sb['s_first_error_func'].decode('utf-8', errors='ignore')
                    issues.append(f"  First error function: {func}")
            if self.sb.get('s_last_error_time', 0) > 0:
                last_error = datetime.datetime.fromtimestamp(self.sb['s_last_error_time'])
                issues.append(f"  Last error at: {last_error}")
                
        return issues
    
    def check_block_groups(self) -> List[str]:
        """Check all block groups for consistency"""
        issues = []
        
        num_groups = (self.sb['blocks_count'] + self.sb['s_blocks_per_group'] - 1) // self.sb['s_blocks_per_group']
        
        total_free_blocks = 0
        total_free_inodes = 0
        
        for bg_num in range(min(num_groups, 5)):  # Check first 5 block groups
            bg = self.parse_block_group_descriptor(bg_num)
            
            # Check for invalid bitmap/table locations
            if bg['bg_block_bitmap_lo'] == 0:
                issues.append(f"Block group {bg_num}: Invalid block bitmap location (0)")
            if bg['bg_inode_bitmap_lo'] == 0:
                issues.append(f"Block group {bg_num}: Invalid inode bitmap location (0)")
            if bg['bg_inode_table_lo'] == 0:
                issues.append(f"Block group {bg_num}: Invalid inode table location (0)")
                
            # Verify checksum if GDT_CSUM is enabled
            if self.sb['s_feature_ro_compat'] & 0x10:
                if bg.get('bg_checksum', 0) == 0:
                    issues.append(f"Block group {bg_num}: Missing checksum")
                    
            # Accumulate free counts
            free_blocks = bg['bg_free_blocks_count_lo']
            if 'bg_free_blocks_count_hi' in bg:
                free_blocks |= (bg['bg_free_blocks_count_hi'] << 16)
            total_free_blocks += free_blocks
            
            free_inodes = bg['bg_free_inodes_count_lo']
            if 'bg_free_inodes_count_hi' in bg:
                free_inodes |= (bg['bg_free_inodes_count_hi'] << 16)
            total_free_inodes += free_inodes
            
        # Note: We're only checking first 5 groups, so don't validate totals
        
        return issues
    
    def analyze(self) -> Dict[str, Any]:
        """Perform complete deep analysis"""
        result = {
            'issues': [],
            'warnings': [],
            'info': []
        }
        
        # Parse primary superblock
        self.sb = self.parse_superblock()
        self.block_size = self.sb['block_size']
        result['superblock'] = self.sb
        
        # Basic validation
        if self.sb['s_magic'] != 0xEF53:
            result['issues'].append(f"Invalid magic number: 0x{self.sb['s_magic']:04X}")
            return result
            
        # Check filesystem state
        if self.sb['s_state'] != 0x0001:
            result['issues'].append(f"Filesystem not cleanly unmounted (state: 0x{self.sb['s_state']:04X})")
            
        # Check for orphaned inodes
        orphan_issues = self.check_orphan_list()
        result['issues'].extend(orphan_issues)
        
        # Check error state
        error_issues = self.check_error_state()
        result['issues'].extend(error_issues)
        
        # Check root inode
        root_issues = self.check_root_inode()
        result['issues'].extend(root_issues)
        
        # Check block groups
        bg_issues = self.check_block_groups()
        result['issues'].extend(bg_issues)
        
        # Check for journal issues
        if self.sb['s_feature_compat'] & 0x0004:  # HAS_JOURNAL
            if self.sb['s_journal_inum'] == 0:
                result['issues'].append("Journal enabled but journal inode is 0")
            elif self.sb['s_feature_incompat'] & 0x0004:  # RECOVER
                result['issues'].append("Journal needs recovery")
                
        # Check for 64-bit filesystem on 32-bit values
        if self.sb['s_feature_incompat'] & 0x0080:  # 64BIT
            if 's_blocks_count_hi' not in self.sb:
                result['warnings'].append("64-bit feature enabled but high bits not set")
                
        # Additional info
        result['info'].append(f"Filesystem size: {self.sb['blocks_count'] * self.block_size / (1024*1024*1024):.2f} GB")
        result['info'].append(f"Block groups: {(self.sb['blocks_count'] + self.sb['s_blocks_per_group'] - 1) // self.sb['s_blocks_per_group']}")
        
        return result
    
    def print_analysis(self, result: Dict[str, Any], label: str = ""):
        """Print deep analysis results"""
        sb = result['superblock']
        
        print(f"\n{'='*70}")
        if label:
            print(f"DEEP ANALYSIS: {label}")
        else:
            print(f"EXT4 DEEP ANALYSIS")
        print(f"{'='*70}")
        
        print(f"\nBasic Info:")
        print(f"  Magic: 0x{sb['s_magic']:04X} (should be 0xEF53)")
        print(f"  State: 0x{sb['s_state']:04X} (0x0001 = clean)")
        print(f"  Volume name: {sb['s_volume_name'].decode('utf-8', errors='ignore')}")
        print(f"  Last mounted: {sb['s_last_mounted'].decode('utf-8', errors='ignore')}")
        
        if result['info']:
            print(f"\nFilesystem Info:")
            for info in result['info']:
                print(f"  {info}")
        
        if result['issues']:
            print(f"\n❌ CRITICAL ISSUES (will prevent mount):")
            for issue in result['issues']:
                print(f"  - {issue}")
        
        if result['warnings']:
            print(f"\n⚠️  WARNINGS (may cause problems):")
            for warning in result['warnings']:
                print(f"  - {warning}")
                
        if not result['issues'] and not result['warnings']:
            print(f"\n✅ No issues found - filesystem should mount cleanly")

def compare_deep(file1_path: str, file2_path: str):
    """Perform deep comparison of two filesystems"""
    print("\n" + "="*80)
    print("EXT4 DEEP FILESYSTEM COMPARISON")
    print("="*80)
    
    def open_device(path):
        """Open a device or image file, handling Windows drive letters"""
        device = path
        # Handle Windows device paths
        if sys.platform == 'win32' and len(device) == 2 and device[1] == ':':
            # Build the path using chr() to avoid escaping issues
            device = chr(92) + chr(92) + '.' + chr(92) + device
        return open(device, 'rb')
    
    with open_device(file1_path) as f1, open_device(file2_path) as f2:
        analyzer1 = Ext4DeepAnalyzer(f1)
        analyzer2 = Ext4DeepAnalyzer(f2)
        
        result1 = analyzer1.analyze()
        result2 = analyzer2.analyze()
        
        analyzer1.print_analysis(result1, file1_path)
        analyzer2.print_analysis(result2, file2_path)
        
        # Compare specific differences
        print(f"\n{'='*70}")
        print("COMPARISON SUMMARY:")
        print(f"{'='*70}")
        
        if result1['issues'] and not result2['issues']:
            print(f"\n{file1_path} has issues that {file2_path} doesn't have")
        elif result2['issues'] and not result1['issues']:
            print(f"\n{file2_path} has issues that {file1_path} doesn't have")
        elif result1['issues'] and result2['issues']:
            print(f"\nBoth filesystems have issues")
        else:
            print(f"\nBoth filesystems appear clean")

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <device_or_image> [<comparison_device>]")
        print(f"Example (Linux): {sys.argv[0]} /dev/sdb")
        print(f"Example (Windows): {sys.argv[0]} E:")
        print(f"Compare: {sys.argv[0]} moses_format.img linux_format.img")
        sys.exit(1)
    
    if len(sys.argv) == 3:
        compare_deep(sys.argv[1], sys.argv[2])
    else:
        device = sys.argv[1]
        
        # Handle Windows device paths (same as FAT16 script)
        if sys.platform == 'win32' and not device.startswith(r'\\'):
            device = r'\\.' + '\\' + device.replace(':', '') + ':'
        
        with open(device, 'rb') as f:
            analyzer = Ext4DeepAnalyzer(f)
            result = analyzer.analyze()
            analyzer.print_analysis(result, sys.argv[1])

if __name__ == "__main__":
    main()