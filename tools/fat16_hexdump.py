#!/usr/bin/env python3
"""
FAT16 hexdump and analysis tool
Shows critical FAT16 structures in a readable format
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
    """Analyze FAT16 boot sector with field annotations"""
    print("=== FAT16 Boot Sector Analysis ===")
    
    # Jump instruction and OEM
    print(f"0x000-0x002: Jump instruction: {data[0:3].hex()}")
    print(f"0x003-0x00A: OEM Name:         '{data[3:11].decode('ascii', errors='ignore')}'")
    
    # BPB (BIOS Parameter Block)
    bytes_per_sector = struct.unpack('<H', data[11:13])[0]
    sectors_per_cluster = data[13]
    reserved_sectors = struct.unpack('<H', data[14:16])[0]
    num_fats = data[16]
    root_entries = struct.unpack('<H', data[17:19])[0]
    total_sectors_16 = struct.unpack('<H', data[19:21])[0]
    media_descriptor = data[21]
    sectors_per_fat = struct.unpack('<H', data[22:24])[0]
    sectors_per_track = struct.unpack('<H', data[24:26])[0]
    num_heads = struct.unpack('<H', data[26:28])[0]
    hidden_sectors = struct.unpack('<I', data[28:32])[0]
    total_sectors_32 = struct.unpack('<I', data[32:36])[0]
    
    print("\n=== BIOS Parameter Block (BPB) ===")
    print(f"0x00B: Bytes per sector:      {bytes_per_sector} {'✓' if bytes_per_sector == 512 else '✗ Usually 512'}")
    print(f"0x00D: Sectors per cluster:   {sectors_per_cluster}")
    print(f"0x00E: Reserved sectors:      {reserved_sectors} {'✓' if reserved_sectors >= 1 else '✗ Must be >= 1'}")
    print(f"0x010: Number of FATs:        {num_fats} {'✓' if num_fats == 2 else '⚠ Usually 2'}")
    print(f"0x011: Root directory entries:{root_entries} {'✓' if root_entries == 512 else '⚠ Usually 512'}")
    print(f"0x013: Total sectors (16):    {total_sectors_16} {'(using 32-bit field)' if total_sectors_16 == 0 else ''}")
    print(f"0x015: Media descriptor:      0x{media_descriptor:02X} {'✓' if media_descriptor in [0xF8, 0xF0] else '⚠'}")
    print(f"0x016: Sectors per FAT:       {sectors_per_fat}")
    print(f"0x018: Sectors per track:     {sectors_per_track}")
    print(f"0x01A: Number of heads:       {num_heads}")
    print(f"0x01C: Hidden sectors:        {hidden_sectors}")
    print(f"0x020: Total sectors (32):    {total_sectors_32}")
    
    # Extended BPB (FAT16)
    drive_number = data[36]
    reserved1 = data[37]
    boot_signature = data[38]
    volume_serial = struct.unpack('<I', data[39:43])[0] if data[38] == 0x29 else 0
    volume_label = data[43:54].decode('ascii', errors='ignore').strip() if data[38] == 0x29 else ''
    fs_type = data[54:62].decode('ascii', errors='ignore').strip() if data[38] == 0x29 else ''
    
    print("\n=== Extended BPB (FAT16) ===")
    print(f"0x024: Drive number:          0x{drive_number:02X} {'✓' if drive_number in [0x00, 0x80] else '⚠'}")
    print(f"0x025: Reserved:              0x{reserved1:02X}")
    print(f"0x026: Boot signature:        0x{boot_signature:02X} {'✓' if boot_signature == 0x29 else '✗ Should be 0x29'}")
    
    if boot_signature == 0x29:
        print(f"0x027: Volume serial number:  0x{volume_serial:08X}")
        print(f"0x02B: Volume label:          '{volume_label}'")
        print(f"0x036: File system type:      '{fs_type}' {'✓' if 'FAT16' in fs_type else '⚠ Should contain FAT16'}")
    
    # Boot sector signature
    boot_sig = struct.unpack('<H', data[510:512])[0]
    print(f"\n0x1FE: Boot signature:        0x{boot_sig:04X} {'✓' if boot_sig == 0xAA55 else '✗ Must be 0xAA55'}")
    
    # Calculate some derived values
    total_sectors = total_sectors_32 if total_sectors_16 == 0 else total_sectors_16
    root_dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) // bytes_per_sector
    data_start_sector = reserved_sectors + (num_fats * sectors_per_fat) + root_dir_sectors
    data_sectors = total_sectors - data_start_sector
    total_clusters = data_sectors // sectors_per_cluster
    
    print("\n=== Calculated Values ===")
    print(f"Total sectors:                {total_sectors}")
    print(f"Root directory sectors:       {root_dir_sectors}")
    print(f"Data start sector:            {data_start_sector}")
    print(f"Data sectors:                 {data_sectors}")
    print(f"Total clusters:               {total_clusters}")
    
    # FAT16 cluster count validation
    if total_clusters < 4085:
        print(f"Cluster count check:          ✗ {total_clusters} < 4085 (This is FAT12!)")
    elif total_clusters > 65524:
        print(f"Cluster count check:          ✗ {total_clusters} > 65524 (This is FAT32!)")
    else:
        print(f"Cluster count check:          ✓ Valid FAT16 range (4085-65524)")
    
    return {
        'reserved_sectors': reserved_sectors,
        'sectors_per_fat': sectors_per_fat,
        'root_entries': root_entries,
        'bytes_per_sector': bytes_per_sector,
        'num_fats': num_fats
    }

def analyze_fat(device, reserved_sectors, num_entries=20):
    """Analyze FAT16 entries"""
    print(f"\n=== FAT16 Analysis (First {num_entries} entries) ===")
    
    fat_data = read_sectors(device, reserved_sectors, 1)
    
    for i in range(min(num_entries, len(fat_data)//2)):
        entry = struct.unpack('<H', fat_data[i*2:(i+1)*2])[0]
        
        annotation = ""
        if i == 0:
            # First FAT entry contains media descriptor
            media = entry & 0xFF
            annotation = f" <- Media descriptor 0x{media:02X} {'✓' if entry in [0xFFF8, 0xFFF0] else '✗'}"
        elif i == 1:
            annotation = f" <- End of chain marker {'✓' if entry >= 0xFFF8 else '✗'}"
        elif entry == 0x0000:
            annotation = " <- Free cluster"
        elif entry == 0x0001:
            annotation = " <- Reserved (invalid)"
        elif 0x0002 <= entry <= 0xFFEF:
            annotation = f" <- Next cluster in chain"
        elif 0xFFF0 <= entry <= 0xFFF6:
            annotation = " <- Reserved"
        elif entry == 0xFFF7:
            annotation = " <- Bad cluster"
        elif entry >= 0xFFF8:
            annotation = " <- End of chain (EOF)"
            
        print(f"FAT[{i:3d}]: 0x{entry:04X}{annotation}")

def analyze_root_directory(device, params):
    """Analyze root directory entries"""
    print("\n=== Root Directory Analysis ===")
    
    # Calculate root directory location
    root_start_sector = params['reserved_sectors'] + (params['num_fats'] * params['sectors_per_fat'])
    
    print(f"Root directory starts at sector {root_start_sector}")
    
    # Read first sector of root directory
    root_data = read_sectors(device, root_start_sector, 1)
    
    # Parse directory entries (32 bytes each)
    for i in range(min(16, len(root_data)//32)):
        entry = root_data[i*32:(i+1)*32]
        
        # Check first byte
        first_byte = entry[0]
        if first_byte == 0x00:
            print(f"Entry {i:2d}: End of directory")
            break
        elif first_byte == 0xE5:
            print(f"Entry {i:2d}: Deleted entry")
            continue
        elif first_byte == 0x05:
            # Kanji lead byte indicator
            first_byte = 0xE5
        
        # Check if it's a long filename entry
        if entry[11] == 0x0F:
            sequence = entry[0] & 0x3F
            is_last = (entry[0] & 0x40) != 0
            checksum = entry[13]
            print(f"Entry {i:2d}: Long filename entry (seq={sequence}, last={is_last}, checksum=0x{checksum:02X})")
            continue
        
        # Regular directory entry
        filename = entry[0:8].decode('ascii', errors='ignore').rstrip()
        extension = entry[8:11].decode('ascii', errors='ignore').rstrip()
        attributes = entry[11]
        
        # Parse attributes
        attr_str = []
        if attributes & 0x01: attr_str.append("RO")
        if attributes & 0x02: attr_str.append("Hidden")
        if attributes & 0x04: attr_str.append("System")
        if attributes & 0x08: attr_str.append("VolumeLabel")
        if attributes & 0x10: attr_str.append("Dir")
        if attributes & 0x20: attr_str.append("Archive")
        
        first_cluster = struct.unpack('<H', entry[26:28])[0]
        file_size = struct.unpack('<I', entry[28:32])[0]
        
        if attributes & 0x08:  # Volume label
            label = (filename + extension).strip()
            print(f"Entry {i:2d}: Volume Label = '{label}'")
        else:
            name = f"{filename}.{extension}" if extension else filename
            print(f"Entry {i:2d}: {name:12s} [{','.join(attr_str):20s}] cluster={first_cluster:5d} size={file_size:10d}")
        
        # Show hex dump of entry
        print(f"         Raw: {entry[:16].hex()} {entry[16:].hex()}")

def main(device):
    """Main analysis function"""
    print(f"Analyzing FAT16 device: {device}")
    print("=" * 70)
    
    # Read and analyze boot sector
    boot_sector = read_sectors(device, 0, 1)
    params = analyze_boot_sector(boot_sector)
    
    # Analyze FAT
    analyze_fat(device, params['reserved_sectors'])
    
    # Analyze root directory
    analyze_root_directory(device, params)
    
    print("\n=== Raw Hexdump of Boot Sector ===")
    print(hexdump(boot_sector[:256]))

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python fat16_hexdump.py <device>")
        print("Example: python fat16_hexdump.py E:")
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