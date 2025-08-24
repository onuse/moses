#!/usr/bin/env python3
"""
exFAT comparison tool - compares two exFAT volumes to find differences
Usage: python exfat_compare.py <device1> <device2>
"""

import sys
import struct
import os

def read_sector(device, sector_num, sector_size=512):
    """Read a specific sector from device"""
    with open(device, 'rb') as f:
        f.seek(sector_num * sector_size)
        return f.read(sector_size)

def parse_boot_sector(data):
    """Parse exFAT boot sector fields"""
    fields = {}
    
    # Basic fields
    fields['jump_boot'] = data[0:3].hex()
    fields['fs_name'] = data[3:11].decode('ascii', errors='ignore')
    fields['must_be_zero'] = data[11:64].hex()
    
    # Key parameters (all little-endian)
    fields['partition_offset'] = struct.unpack('<Q', data[64:72])[0]
    fields['volume_length'] = struct.unpack('<Q', data[72:80])[0]
    fields['fat_offset'] = struct.unpack('<I', data[80:84])[0]
    fields['fat_length'] = struct.unpack('<I', data[84:88])[0]
    fields['cluster_heap_offset'] = struct.unpack('<I', data[88:92])[0]
    fields['cluster_count'] = struct.unpack('<I', data[92:96])[0]
    fields['first_cluster_root'] = struct.unpack('<I', data[96:100])[0]
    fields['volume_serial'] = struct.unpack('<I', data[100:104])[0]
    fields['fs_revision'] = struct.unpack('<H', data[104:106])[0]
    fields['volume_flags'] = struct.unpack('<H', data[106:108])[0]
    fields['bytes_per_sector_shift'] = data[108]
    fields['sectors_per_cluster_shift'] = data[109]
    fields['num_fats'] = data[110]
    fields['drive_select'] = data[111]
    fields['percent_in_use'] = data[112]
    fields['reserved'] = data[113:120].hex()
    fields['boot_signature'] = struct.unpack('<H', data[510:512])[0]
    
    return fields

def calculate_checksum(sectors_data):
    """Calculate exFAT boot checksum using official algorithm"""
    checksum = 0
    
    for i, byte in enumerate(sectors_data):
        # Skip VolumeFlags (106-107) and PercentInUse (112)
        if i == 106 or i == 107 or i == 112:
            continue
        
        # Official algorithm: rotate right and add
        if checksum & 1:
            checksum = 0x80000000 + (checksum >> 1) + byte
        else:
            checksum = (checksum >> 1) + byte
            
    return checksum & 0xFFFFFFFF

def compare_fat_entries(dev1, dev2, fat_offset, num_entries=20):
    """Compare first N FAT entries"""
    print("\n=== FAT Entries Comparison ===")
    with open(dev1, 'rb') as f1, open(dev2, 'rb') as f2:
        f1.seek(fat_offset * 512)
        f2.seek(fat_offset * 512)
        
        for i in range(num_entries):
            entry1 = struct.unpack('<I', f1.read(4))[0]
            entry2 = struct.unpack('<I', f2.read(4))[0]
            
            if entry1 != entry2:
                print(f"FAT[{i}]: 0x{entry1:08X} vs 0x{entry2:08X}", end="")
                if i == 0:
                    print(" (Media descriptor)")
                elif i == 1:
                    print(" (End of chain marker)")
                else:
                    print()

def read_root_directory(device, cluster_heap_offset, first_cluster_root, 
                        sectors_per_cluster, bytes_per_sector=512):
    """Read root directory entries"""
    entries = []
    
    with open(device, 'rb') as f:
        # Calculate root directory offset
        # Clusters start at 2, so actual cluster = first_cluster_root - 2
        cluster_offset = (first_cluster_root - 2) * sectors_per_cluster * bytes_per_sector
        root_offset = cluster_heap_offset * bytes_per_sector + cluster_offset
        
        f.seek(root_offset)
        
        # Read up to 16 entries (512 bytes)
        for i in range(16):
            entry_data = f.read(32)
            if len(entry_data) < 32:
                break
                
            entry_type = entry_data[0]
            if entry_type == 0x00:  # End of directory
                break
            elif entry_type == 0x83:  # Volume label
                char_count = entry_data[1]
                label = entry_data[2:2+char_count*2].decode('utf-16-le', errors='ignore')
                entries.append(f"Volume Label: '{label}'")
            elif entry_type == 0x81:  # Bitmap
                first_cluster = struct.unpack('<I', entry_data[20:24])[0]
                data_length = struct.unpack('<Q', entry_data[24:32])[0]
                entries.append(f"Bitmap: cluster {first_cluster}, size {data_length}")
            elif entry_type == 0x82:  # Upcase
                checksum = struct.unpack('<I', entry_data[4:8])[0]
                first_cluster = struct.unpack('<I', entry_data[20:24])[0]
                data_length = struct.unpack('<Q', entry_data[24:32])[0]
                entries.append(f"Upcase: cluster {first_cluster}, size {data_length}, checksum 0x{checksum:08X}")
            elif entry_type == 0xA0:  # Volume GUID
                guid = entry_data[6:22].hex()
                entries.append(f"Volume GUID: {guid}")
            elif entry_type == 0x85:  # File entry
                attrs = struct.unpack('<H', entry_data[4:6])[0]
                entries.append(f"File Entry: attrs=0x{attrs:04X}")
            else:
                entries.append(f"Entry type 0x{entry_type:02X}")
    
    return entries

def compare_devices(dev1_path, dev2_path):
    """Compare two exFAT devices"""
    print(f"Comparing exFAT volumes:")
    print(f"  Device 1: {dev1_path}")
    print(f"  Device 2: {dev2_path}")
    print("=" * 60)
    
    # Read boot sectors
    boot1 = read_sector(dev1_path, 0)
    boot2 = read_sector(dev2_path, 0)
    
    # Parse boot sectors
    fields1 = parse_boot_sector(boot1)
    fields2 = parse_boot_sector(boot2)
    
    # Compare fields
    print("\n=== Boot Sector Comparison ===")
    for key in fields1:
        if fields1[key] != fields2[key]:
            print(f"DIFF {key:25s}: {fields1[key]} vs {fields2[key]}")
        else:
            print(f"OK   {key:25s}: {fields1[key]}")
    
    # Check boot checksums
    print("\n=== Boot Checksum Verification ===")
    
    # Read sectors 0-10 for checksum calculation
    boot_region1 = b''
    boot_region2 = b''
    for i in range(11):
        boot_region1 += read_sector(dev1_path, i)
        boot_region2 += read_sector(dev2_path, i)
    
    # Read actual checksum sectors
    checksum_sector1 = read_sector(dev1_path, 11)
    checksum_sector2 = read_sector(dev2_path, 11)
    
    stored_checksum1 = struct.unpack('<I', checksum_sector1[0:4])[0]
    stored_checksum2 = struct.unpack('<I', checksum_sector2[0:4])[0]
    
    calc_checksum1 = calculate_checksum(boot_region1)
    calc_checksum2 = calculate_checksum(boot_region2)
    
    print(f"Device 1: Stored=0x{stored_checksum1:08X}, Calculated=0x{calc_checksum1:08X} {'✓' if stored_checksum1 == calc_checksum1 else '✗'}")
    print(f"Device 2: Stored=0x{stored_checksum2:08X}, Calculated=0x{calc_checksum2:08X} {'✓' if stored_checksum2 == calc_checksum2 else '✗'}")
    
    # Compare FAT entries
    compare_fat_entries(dev1_path, dev2_path, fields1['fat_offset'])
    
    # Compare root directory
    print("\n=== Root Directory Entries ===")
    sectors_per_cluster1 = 1 << fields1['sectors_per_cluster_shift']
    sectors_per_cluster2 = 1 << fields2['sectors_per_cluster_shift']
    
    entries1 = read_root_directory(dev1_path, fields1['cluster_heap_offset'], 
                                   fields1['first_cluster_root'], sectors_per_cluster1)
    entries2 = read_root_directory(dev2_path, fields2['cluster_heap_offset'],
                                   fields2['first_cluster_root'], sectors_per_cluster2)
    
    print("Device 1 root entries:")
    for entry in entries1:
        print(f"  {entry}")
    
    print("\nDevice 2 root entries:")
    for entry in entries2:
        print(f"  {entry}")
    
    # Check for critical differences
    print("\n=== Critical Issues ===")
    issues = []
    
    if fields1['fs_name'] != 'EXFAT   ':
        issues.append("FS name is not 'EXFAT   '")
    
    if stored_checksum1 != calc_checksum1:
        issues.append("Boot checksum mismatch")
    
    if fields1['num_fats'] not in [1, 2]:
        issues.append(f"Invalid number of FATs: {fields1['num_fats']}")
    
    if not any('Volume GUID' in e for e in entries1):
        issues.append("Missing Volume GUID entry in root directory")
    
    if not any('Bitmap' in e for e in entries1):
        issues.append("Missing Bitmap entry in root directory")
    
    if not any('Upcase' in e for e in entries1):
        issues.append("Missing Upcase entry in root directory")
    
    if issues:
        for issue in issues:
            print(f"  ✗ {issue}")
    else:
        print("  ✓ No critical issues found")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python exfat_compare.py <device1> <device2>")
        print("Example: python exfat_compare.py \\\\.\\E: \\\\.\\F:")
        sys.exit(1)
    
    # Handle Windows device paths
    dev1 = sys.argv[1]
    dev2 = sys.argv[2]
    
    # On Windows, ensure proper formatting
    if sys.platform == 'win32':
        if not dev1.startswith(r'\\'):
            dev1 = r'\\.' + '\\' + dev1.replace(':', '') + ':'
        if not dev2.startswith(r'\\'):
            dev2 = r'\\.' + '\\' + dev2.replace(':', '') + ':'
    
    try:
        compare_devices(dev1, dev2)
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)