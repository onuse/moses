#!/usr/bin/env python3
"""
Hexdump tool for exFAT analysis
Shows critical exFAT structures in a readable format
"""

import sys
import struct

def read_sectors(device, start_sector, count, sector_size=512):
    """Read sectors from device"""
    with open(device, 'rb') as f:
        f.seek(start_sector * sector_size)
        return f.read(count * sector_size)

def hexdump(data, offset=0, show_ascii=True):
    """Create a hexdump of data"""
    result = []
    for i in range(0, len(data), 16):
        hex_bytes = ' '.join(f'{b:02X}' for b in data[i:i+16])
        if show_ascii:
            ascii_bytes = ''.join(chr(b) if 32 <= b < 127 else '.' for b in data[i:i+16])
            result.append(f'{offset+i:08X}: {hex_bytes:<48} |{ascii_bytes}|')
        else:
            result.append(f'{offset+i:08X}: {hex_bytes}')
    return '\n'.join(result)

def analyze_boot_sector(data):
    """Analyze boot sector with field annotations"""
    print("=== Boot Sector (Sector 0) ===")
    print(f"0x000-0x002: Jump Boot:      {data[0:3].hex()} {'✓' if data[0:3] == b'\\xEB\\x76\\x90' else '✗ Should be EB7690'}")
    print(f"0x003-0x00A: FS Name:        '{data[3:11].decode('ascii', errors='ignore')}' {'✓' if data[3:11] == b'EXFAT   ' else '✗ Should be EXFAT   '}")
    print(f"0x00B-0x03F: Must be zero:   {'✓ All zeros' if all(b == 0 for b in data[11:64]) else '✗ Contains non-zero bytes'}")
    
    print("\nKey Parameters:")
    partition_offset = struct.unpack('<Q', data[64:72])[0]
    volume_length = struct.unpack('<Q', data[72:80])[0]
    fat_offset = struct.unpack('<I', data[80:84])[0]
    fat_length = struct.unpack('<I', data[84:88])[0]
    cluster_heap_offset = struct.unpack('<I', data[88:92])[0]
    cluster_count = struct.unpack('<I', data[92:96])[0]
    first_cluster_root = struct.unpack('<I', data[96:100])[0]
    volume_serial = struct.unpack('<I', data[100:104])[0]
    fs_revision = struct.unpack('<H', data[104:106])[0]
    volume_flags = struct.unpack('<H', data[106:108])[0]
    
    print(f"0x040: Partition Offset:     {partition_offset} (0x{partition_offset:016X})")
    print(f"0x048: Volume Length:        {volume_length} bytes ({volume_length/1024/1024/1024:.2f} GB)")
    print(f"0x050: FAT Offset:           {fat_offset} sectors")
    print(f"0x054: FAT Length:           {fat_length} sectors")
    print(f"0x058: Cluster Heap Offset:  {cluster_heap_offset} sectors")
    print(f"0x05C: Cluster Count:        {cluster_count}")
    print(f"0x060: First Cluster Root:   {first_cluster_root}")
    print(f"0x064: Volume Serial:        0x{volume_serial:08X}")
    print(f"0x068: FS Revision:          0x{fs_revision:04X} {'✓' if fs_revision == 0x0100 else '✗ Should be 0x0100'}")
    print(f"0x06A: Volume Flags:         0x{volume_flags:04X}")
    
    print(f"\n0x06C: Bytes/Sector Shift:   {data[108]} (= {1 << data[108]} bytes)")
    print(f"0x06D: Sectors/Cluster Shift:{data[109]} (= {1 << data[109]} sectors)")
    print(f"0x06E: Number of FATs:       {data[110]} {'✓' if data[110] in [1,2] else '✗ Should be 1 or 2'}")
    print(f"0x06F: Drive Select:         0x{data[111]:02X} {'✓' if data[111] == 0x80 else '⚠ Usually 0x80'}")
    print(f"0x070: Percent In Use:       {data[112]}%")
    
    boot_sig = struct.unpack('<H', data[510:512])[0]
    print(f"\n0x1FE: Boot Signature:       0x{boot_sig:04X} {'✓' if boot_sig == 0xAA55 else '✗ Should be 0xAA55'}")

def analyze_checksum(device):
    """Analyze boot checksum"""
    print("\n=== Boot Checksum Analysis ===")
    
    # Read boot region sectors 0-10
    boot_region = read_sectors(device, 0, 11)
    
    # Calculate checksum
    checksum = 0
    for i, byte in enumerate(boot_region):
        # Skip VolumeFlags (106-107) and PercentInUse (112)
        if i == 106 or i == 107 or i == 112:
            continue
        
        if checksum & 1:
            checksum = 0x80000000 + (checksum >> 1) + byte
        else:
            checksum = (checksum >> 1) + byte
    
    checksum = checksum & 0xFFFFFFFF
    
    # Read stored checksum
    checksum_sector = read_sectors(device, 11, 1)
    stored_checksum = struct.unpack('<I', checksum_sector[0:4])[0]
    
    print(f"Calculated checksum: 0x{checksum:08X}")
    print(f"Stored checksum:     0x{stored_checksum:08X}")
    print(f"Status: {'✓ Match' if checksum == stored_checksum else '✗ MISMATCH!'}")
    
    # Check if all values in checksum sector are the same
    all_same = all(checksum_sector[i:i+4] == checksum_sector[0:4] for i in range(0, 512, 4))
    print(f"Checksum sector fill: {'✓ All entries same' if all_same else '✗ Entries differ'}")

def analyze_fat(device, fat_offset, num_entries=16):
    """Analyze FAT entries"""
    print(f"\n=== FAT Analysis (First {num_entries} entries) ===")
    
    fat_data = read_sectors(device, fat_offset, 1)
    
    for i in range(min(num_entries, len(fat_data)//4)):
        entry = struct.unpack('<I', fat_data[i*4:(i+1)*4])[0]
        
        annotation = ""
        if i == 0:
            annotation = f" <- Media descriptor {'✓' if entry == 0xFFFFFFF8 else '✗ Should be 0xFFFFFFF8'}"
        elif i == 1:
            annotation = f" <- End of chain {'✓' if entry == 0xFFFFFFFF else '✗ Should be 0xFFFFFFFF'}"
        elif entry == 0xFFFFFFFF:
            annotation = " <- End of chain (allocated)"
        elif entry == 0x00000000:
            annotation = " <- Free"
        elif 2 <= entry < 0xFFFFFFF7:
            annotation = f" <- Next cluster"
            
        print(f"FAT[{i:3d}]: 0x{entry:08X}{annotation}")

def analyze_root_dir(device, cluster_heap_offset, first_cluster_root, sectors_per_cluster):
    """Analyze root directory entries"""
    print("\n=== Root Directory Analysis ===")
    
    # Calculate root directory offset
    cluster_offset = (first_cluster_root - 2) * sectors_per_cluster * 512
    root_offset = cluster_heap_offset * 512 + cluster_offset
    
    print(f"Root directory at sector {cluster_heap_offset + (first_cluster_root - 2) * sectors_per_cluster}")
    print(f"Offset: 0x{root_offset:08X}\n")
    
    with open(device, 'rb') as f:
        f.seek(root_offset)
        
        for i in range(16):  # Read up to 16 entries
            entry = f.read(32)
            if len(entry) < 32:
                break
                
            entry_type = entry[0]
            
            if entry_type == 0x00:
                print(f"Entry {i:2d}: End of directory")
                break
            elif entry_type == 0x83:  # Volume label
                char_count = entry[1]
                label = entry[2:2+char_count*2].decode('utf-16-le', errors='ignore')
                print(f"Entry {i:2d}: Volume Label = '{label}'")
            elif entry_type == 0x81:  # Bitmap
                flags = entry[1]
                first_cluster = struct.unpack('<I', entry[20:24])[0]
                data_length = struct.unpack('<Q', entry[24:32])[0]
                print(f"Entry {i:2d}: Allocation Bitmap (cluster {first_cluster}, size {data_length} bytes)")
            elif entry_type == 0x82:  # Upcase
                checksum = struct.unpack('<I', entry[4:8])[0]
                first_cluster = struct.unpack('<I', entry[20:24])[0]
                data_length = struct.unpack('<Q', entry[24:32])[0]
                print(f"Entry {i:2d}: Upcase Table (cluster {first_cluster}, size {data_length} bytes, checksum 0x{checksum:08X})")
            elif entry_type == 0xA0:  # Volume GUID
                guid = entry[6:22]
                print(f"Entry {i:2d}: Volume GUID = {guid.hex()}")
            elif entry_type == 0x85:  # File
                secondary_count = entry[1]
                attrs = struct.unpack('<H', entry[4:6])[0]
                print(f"Entry {i:2d}: File Entry (secondary_count={secondary_count}, attrs=0x{attrs:04X})")
            elif entry_type == 0xC0:  # Stream
                flags = entry[1]
                name_len = entry[3]
                first_cluster = struct.unpack('<I', entry[20:24])[0]
                data_length = struct.unpack('<Q', entry[24:32])[0]
                print(f"Entry {i:2d}: Stream Extension (name_len={name_len}, cluster {first_cluster}, size {data_length})")
            elif entry_type == 0xC1:  # File name
                print(f"Entry {i:2d}: File Name")
            else:
                print(f"Entry {i:2d}: Type 0x{entry_type:02X} (Unknown/Empty)")
            
            # Show hex dump of entry
            print(f"         Raw: {entry[:16].hex()} {entry[16:].hex()}")

def main(device):
    """Main analysis function"""
    print(f"Analyzing exFAT device: {device}")
    print("=" * 70)
    
    # Read and analyze boot sector
    boot_sector = read_sectors(device, 0, 1)
    analyze_boot_sector(boot_sector)
    
    # Analyze checksum
    analyze_checksum(device)
    
    # Get parameters for further analysis
    fat_offset = struct.unpack('<I', boot_sector[80:84])[0]
    cluster_heap_offset = struct.unpack('<I', boot_sector[88:92])[0]
    first_cluster_root = struct.unpack('<I', boot_sector[96:100])[0]
    sectors_per_cluster = 1 << boot_sector[109]
    
    # Analyze FAT
    analyze_fat(device, fat_offset)
    
    # Analyze root directory
    analyze_root_dir(device, cluster_heap_offset, first_cluster_root, sectors_per_cluster)
    
    print("\n=== Raw Hexdump of Boot Sector ===")
    print(hexdump(boot_sector[:256]))

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python exfat_hexdump.py <device>")
        print("Example: python exfat_hexdump.py \\\\.\\E:")
        sys.exit(1)
    
    device = sys.argv[1]
    
    # Handle Windows device paths
    if sys.platform == 'win32' and not device.startswith(r'\\'):
        device = r'\\.' + '\\' + device.replace(':', '') + ':'
    
    try:
        main(device)
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)