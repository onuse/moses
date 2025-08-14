# Testing Windows Device Enumeration

## Quick Test
Run the PowerShell test script to verify your system supports the required commands:
```powershell
powershell.exe -ExecutionPolicy Bypass -File test_device_enum.ps1
```

## Building on Windows

### Prerequisites
1. Install Rust for Windows from https://rustup.rs/
2. Install Visual Studio Build Tools with "Desktop development with C++"

### Build Steps
Open PowerShell or Command Prompt on Windows (not WSL):

```powershell
# Navigate to project directory
cd C:\Users\glimm\Documents\Projects\moses

# Build the CLI
cargo build --package moses-cli

# Run device enumeration
.\target\debug\moses.exe list
```

## Expected Output
You should see output similar to:
```
Available devices:

Device: NVMe HFM512GD3JX013N
  Path: \\.\PHYSICALDRIVE1
  Size: 476.94 GB
  Type: SSD
  Removable: No
  System: Yes (⚠️ PROTECTED)
  Mounted at: ["C:"]

Device: Kingston DataTraveler 3.0
  Path: \\.\PHYSICALDRIVE2
  Size: 57.66 GB
  Type: USB
  Removable: Yes
  System: No

Device: Kingston XS1000
  Path: \\.\PHYSICALDRIVE3
  Size: 1862.67 GB
  Type: USB
  Removable: Yes
  System: No
  Mounted at: ["E:"]
```

## Implementation Details
The Windows device enumeration uses:
- PowerShell's `Get-Disk` cmdlet for basic disk information
- WMI `Win32_DiskDrive` for additional hardware details
- `Get-Partition` for mount point information
- Automatic detection of system drives and removable media
- Admin permission checking (read-only vs full access)

## Safety Features
- System drives are marked with ⚠️ PROTECTED
- Removable drives are prioritized in the listing
- C: drive is automatically detected as unsafe to format
- Permission level checking before any operations