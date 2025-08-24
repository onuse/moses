#!/usr/bin/env python3
"""
Simple sequential read to find ext4 on Windows
"""

import sys
import struct

def main(device_path):
    # Handle Windows device paths
    if sys.platform == 'win32' and not device_path.startswith(r'\\'):
        device_path = r'\\.' + '\\' + device_path.replace(':', '') + ':'
    
    print(f"Reading from: {device_path}")
    
    with open(device_path, 'rb') as f:
        # Read first 10MB in chunks
        print("Reading device sequentially...")
        
        chunk_size = 4096
        offset = 0
        found = False
        
        for i in range(2560):  # 2560 * 4096 = 10MB
            chunk = f.read(chunk_size)
            if len(chunk) < chunk_size:
                print(f"End of device at offset {offset}")
                break
            
            # Look for ext4 magic at offset 1024 within each 4K block
            if len(chunk) > 1024 + 0x3A:
                # Check at offset 1024 in this chunk (standard superblock location)
                if offset == 0:  # First chunk
                    magic_offset = 1024 + 0x38
                    if magic_offset < len(chunk):
                        magic = struct.unpack_from('<H', chunk, magic_offset)[0]
                        if magic == 0xEF53:
                            print(f"\n✅ Found EXT4 at offset {offset + 1024}!")
                            
                            # Parse some basic info
                            sb_start = 1024
                            s_inodes_count = struct.unpack_from('<I', chunk, sb_start + 0x00)[0]
                            s_blocks_count = struct.unpack_from('<I', chunk, sb_start + 0x04)[0]
                            s_log_block_size = struct.unpack_from('<I', chunk, sb_start + 0x18)[0]
                            block_size = 1024 << s_log_block_size
                            s_volume_name = chunk[sb_start + 0x78:sb_start + 0x88].rstrip(b'\x00')
                            
                            print(f"  Volume name: '{s_volume_name.decode('utf-8', errors='ignore')}'")
                            print(f"  Blocks: {s_blocks_count}, Block size: {block_size}")
                            print(f"  Filesystem size: {s_blocks_count * block_size / (1024*1024*1024):.2f} GB")
                            
                            # Now check block group 0 descriptor
                            if block_size == 1024:
                                bgd_offset = 2048  # Block 2 for 1K blocks
                            else:
                                bgd_offset = block_size  # Block 1 for larger blocks
                            
                            print(f"\nBlock group 0 descriptor should be at offset: {bgd_offset}")
                            
                            # Read block group descriptor
                            f.seek(bgd_offset)
                            bgd_data = f.read(32)
                            
                            bg_inode_table = struct.unpack_from('<I', bgd_data, 0x08)[0]
                            print(f"  Inode table for BG0: block {bg_inode_table}")
                            print(f"  Inode table offset: {bg_inode_table * block_size}")
                            
                            # Calculate root inode location
                            inode_size = 256  # Typical for ext4
                            root_inode_offset = bg_inode_table * block_size + inode_size  # Inode 2 is at offset 1
                            print(f"\nRoot inode (inode 2) should be at offset: {root_inode_offset}")
                            
                            found = True
                            break
                
                # Also check if this chunk itself starts with superblock
                magic_at_38 = struct.unpack_from('<H', chunk, 0x38)[0] if len(chunk) > 0x3A else 0
                if magic_at_38 == 0xEF53:
                    print(f"\n✅ Found EXT4 superblock at offset {offset - 1024}!")
                    found = True
                    break
            
            offset += chunk_size
            if offset % (1024 * 1024) == 0:
                print(f"  Read {offset // (1024 * 1024)}MB...")
        
        if not found:
            print("\n❌ No EXT4 filesystem found in first 10MB")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python ext4_simple_read.py <device>")
        sys.exit(1)
    
    main(sys.argv[1])