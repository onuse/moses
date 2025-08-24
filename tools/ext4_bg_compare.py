#!/usr/bin/env python3
"""
Compare block group descriptors between two ext4 filesystems
"""

import sys
import struct

def read_sequential(f, offset, size):
    """Sequential read for Windows devices"""
    f.seek(0)
    bytes_to_skip = offset
    chunk_size = 65536
    
    while bytes_to_skip > 0:
        skip_size = min(chunk_size, bytes_to_skip)
        skipped = f.read(skip_size)
        if len(skipped) < skip_size:
            return b''
        bytes_to_skip -= skip_size
    
    return f.read(size)

def analyze_block_groups(device_path, label):
    """Analyze block group descriptors"""
    # Handle Windows device paths
    if sys.platform == 'win32' and not device_path.startswith(r'\\'):
        device_path = r'\\.' + '\\' + device_path.replace(':', '') + ':'
    
    print(f"\n{'='*70}")
    print(f"BLOCK GROUP ANALYSIS: {label}")
    print(f"Device: {device_path}")
    print(f"{'='*70}")
    
    with open(device_path, 'rb') as f:
        # Read superblock
        sb_data = read_sequential(f, 1024, 1024)
        
        # Parse key superblock fields
        s_blocks_count_lo = struct.unpack_from('<I', sb_data, 0x04)[0]
        s_blocks_per_group = struct.unpack_from('<I', sb_data, 0x20)[0]
        s_log_block_size = struct.unpack_from('<I', sb_data, 0x18)[0]
        s_desc_size = struct.unpack_from('<H', sb_data, 0xFE)[0] if len(sb_data) > 0xFE else 0
        s_feature_incompat = struct.unpack_from('<I', sb_data, 0x60)[0]
        s_feature_ro_compat = struct.unpack_from('<I', sb_data, 0x64)[0]
        
        block_size = 1024 << s_log_block_size
        desc_size = s_desc_size if s_desc_size else 32
        num_groups = (s_blocks_count_lo + s_blocks_per_group - 1) // s_blocks_per_group
        
        print(f"Block size: {block_size} bytes")
        print(f"Blocks per group: {s_blocks_per_group}")
        print(f"Descriptor size: {desc_size} bytes")
        print(f"Number of block groups: {num_groups}")
        print(f"Features incompat: 0x{s_feature_incompat:08X}")
        print(f"Features ro_compat: 0x{s_feature_ro_compat:08X}")
        
        # Check feature flags
        has_64bit = bool(s_feature_incompat & 0x80)
        has_gdt_csum = bool(s_feature_ro_compat & 0x10)
        has_metadata_csum = bool(s_feature_ro_compat & 0x400)
        
        print(f"\nFeatures:")
        print(f"  64-bit: {has_64bit}")
        print(f"  GDT checksums: {has_gdt_csum}")
        print(f"  Metadata checksums: {has_metadata_csum}")
        
        # Read block group descriptors
        if block_size == 1024:
            bgd_offset = 2048  # Block 2 for 1K blocks
        else:
            bgd_offset = block_size  # Block 1 for larger blocks
        
        print(f"\nBlock Group Descriptors (first 10):")
        print(f"{'BG':3} {'Block Bitmap':12} {'Inode Bitmap':12} {'Inode Table':12} {'Free Blocks':11} {'Free Inodes':11} {'Checksum':8}")
        print("-" * 85)
        
        for bg_num in range(min(10, num_groups)):
            offset = bgd_offset + bg_num * desc_size
            bgd_data = read_sequential(f, offset, desc_size)
            
            if len(bgd_data) < 32:
                print(f"{bg_num:3d} ERROR: Cannot read descriptor")
                continue
            
            # Parse descriptor
            bg_block_bitmap_lo = struct.unpack_from('<I', bgd_data, 0x00)[0]
            bg_inode_bitmap_lo = struct.unpack_from('<I', bgd_data, 0x04)[0]
            bg_inode_table_lo = struct.unpack_from('<I', bgd_data, 0x08)[0]
            bg_free_blocks_lo = struct.unpack_from('<H', bgd_data, 0x0C)[0]
            bg_free_inodes_lo = struct.unpack_from('<H', bgd_data, 0x0E)[0]
            bg_checksum = struct.unpack_from('<H', bgd_data, 0x1E)[0]
            
            # Check for issues
            issues = []
            if bg_block_bitmap_lo == 0:
                issues.append("NO_BLOCK_BMP")
            if bg_inode_bitmap_lo == 0:
                issues.append("NO_INODE_BMP")
            if bg_inode_table_lo == 0:
                issues.append("NO_INODE_TBL")
            if has_gdt_csum and bg_checksum == 0:
                issues.append("NO_CHECKSUM")
            
            if issues:
                print(f"{bg_num:3d} {bg_block_bitmap_lo:12d} {bg_inode_bitmap_lo:12d} {bg_inode_table_lo:12d} {bg_free_blocks_lo:11d} {bg_free_inodes_lo:11d} {bg_checksum:8d} ❌ {', '.join(issues)}")
            else:
                print(f"{bg_num:3d} {bg_block_bitmap_lo:12d} {bg_inode_bitmap_lo:12d} {bg_inode_table_lo:12d} {bg_free_blocks_lo:11d} {bg_free_inodes_lo:11d} {bg_checksum:8d} ✓")
        
        # Check for sparse_super feature
        print(f"\nSparse Super Feature: {'Yes' if (s_feature_ro_compat & 0x01) else 'No'}")
        if s_feature_ro_compat & 0x01:
            print("  Backup superblocks should be at block groups: 0, 1, 3, 5, 7, 9, 25, 27, ...")

def main():
    if len(sys.argv) < 2:
        print("Usage: python ext4_bg_compare.py <device1> [<device2>]")
        print("Example: python ext4_bg_compare.py E:")
        print("Compare: python ext4_bg_compare.py E: F:")
        sys.exit(1)
    
    if len(sys.argv) == 2:
        analyze_block_groups(sys.argv[1], "Single Device")
    else:
        analyze_block_groups(sys.argv[1], "Device 1")
        analyze_block_groups(sys.argv[2], "Device 2")

if __name__ == "__main__":
    main()