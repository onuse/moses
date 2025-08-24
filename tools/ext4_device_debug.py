#!/usr/bin/env python3
"""
Debug tool to understand device geometry and find ext4 filesystem on raw devices
"""

import sys
import struct

def find_ext4_superblock(device_path):
    """Search for ext4 superblock magic number on device"""
    print(f"Searching for EXT4 filesystem on {device_path}")
    print("=" * 70)
    
    # Handle Windows device paths
    if sys.platform == 'win32' and not device_path.startswith(r'\\'):
        device_path = r'\\.' + '\\' + device_path.replace(':', '') + ':'
    
    print(f"Opening device: {device_path}")
    
    try:
        with open(device_path, 'rb') as f:
            # First, just try to read the first sector to see if device is accessible
            try:
                f.seek(0)
                first_sector = f.read(512)
                print(f"✓ Device opened successfully, read {len(first_sector)} bytes")
            except Exception as e:
                print(f"❌ Cannot read from device: {e}")
                print("\nTrying alternative approach...")
                
                # Try reading with buffering disabled
                import os
                fd = os.open(device_path, os.O_RDONLY | os.O_BINARY)
                first_sector = os.read(fd, 512)
                os.close(fd)
                print(f"✓ Alternative method worked, read {len(first_sector)} bytes")
                
                # Reopen with alternative method
                f = open(device_path, 'rb', buffering=0)
        # Common locations to check for superblock
        # Offset 1024 is standard for first superblock
        # But on a raw device, we might need to account for partition offset
        
        locations_to_check = [
            (0, "Start of device"),
            (512, "After MBR"),
            (1024, "Standard ext4 offset"),
            (2048, "After 2KB"),
            (32256, "Common partition start (63 sectors)"),
            (1048576, "1MB aligned partition"),
            (2097152, "2MB aligned partition"),
        ]
        
        # Also check every 512 bytes for first 10MB
        print("\nScanning for EXT4 magic (0xEF53) at various offsets:\n")
        
        found_locations = []
        
        # Check specific locations
        for offset, description in locations_to_check:
            try:
                f.seek(offset + 0x38)  # Magic is at offset 0x38 in superblock
                magic_bytes = f.read(2)
                if len(magic_bytes) == 2:
                    magic = struct.unpack('<H', magic_bytes)[0]
                    if magic == 0xEF53:
                        print(f"✓ Found EXT4 magic at offset {offset} ({description})")
                        found_locations.append(offset)
                    else:
                        print(f"  Offset {offset:8} ({description:30}): 0x{magic:04X}")
            except:
                print(f"  Offset {offset:8} ({description:30}): Cannot read")
        
        # Scan first 10MB in 512-byte sectors
        print("\nScanning first 10MB in 512-byte sectors...")
        for sector in range(0, 20480, 1):  # 20480 sectors = 10MB
            offset = sector * 512
            try:
                # Check for superblock at offset + 1024
                f.seek(offset + 1024 + 0x38)
                magic_bytes = f.read(2)
                if len(magic_bytes) == 2:
                    magic = struct.unpack('<H', magic_bytes)[0]
                    if magic == 0xEF53:
                        print(f"✓ Found EXT4 superblock at sector {sector} (offset {offset}, superblock at {offset + 1024})")
                        found_locations.append(offset + 1024)
                        
                        # Read some superblock info
                        f.seek(offset + 1024)
                        sb_data = f.read(256)
                        
                        # Parse basic info
                        s_inodes_count = struct.unpack_from('<I', sb_data, 0x00)[0]
                        s_blocks_count = struct.unpack_from('<I', sb_data, 0x04)[0]
                        s_log_block_size = struct.unpack_from('<I', sb_data, 0x18)[0]
                        block_size = 1024 << s_log_block_size
                        s_volume_name = sb_data[0x78:0x88].rstrip(b'\x00')
                        
                        print(f"  Volume name: {s_volume_name.decode('utf-8', errors='ignore')}")
                        print(f"  Blocks: {s_blocks_count}, Block size: {block_size}")
                        print(f"  Inodes: {s_inodes_count}")
                        print(f"  Filesystem size: {s_blocks_count * block_size / (1024*1024*1024):.2f} GB")
                        
                        break  # Found it, stop scanning
            except:
                continue
        
        if found_locations:
            print(f"\n✅ Found EXT4 filesystem(s) at offsets: {found_locations}")
            print("\nTo fix ext4_hexdump.py, the filesystem likely starts at offset:")
            # The actual filesystem start is 1024 bytes before the superblock
            fs_start = found_locations[0] - 1024 if found_locations[0] >= 1024 else 0
            print(f"  Filesystem start offset: {fs_start} (0x{fs_start:X})")
            print(f"  Superblock at: {found_locations[0]} (0x{found_locations[0]:X})")
        else:
            print("\n❌ No EXT4 filesystem found")
            print("The device might not be formatted with ext4, or might use GPT partitioning")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python ext4_device_debug.py <device>")
        print("Example: python ext4_device_debug.py E:")
        sys.exit(1)
    
    find_ext4_superblock(sys.argv[1])