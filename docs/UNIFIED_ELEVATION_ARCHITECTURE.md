# Unified Elevation Architecture

## Overview
Moses now uses a **unified socket-based worker architecture** for ALL operations requiring administrator privileges. This ensures users see only ONE UAC prompt per session, regardless of how many privileged operations they perform.

## Operations Using Socket Worker

### ✅ Disk Management Operations
- **Clean Disk** (`clean_disk_socket`)
  - Quick clean
  - Zero wipe
  - DoD 5220 wipe
  - Random wipe

- **Format Disk** (`format_disk_socket`)
  - All filesystem types (FAT16, FAT32, NTFS, ext4, exFAT)
  - With or without partition table creation
  - With optional pre-clean

### ✅ Filesystem Analysis Operations
- **Analyze Filesystem** (`analyze_filesystem_socket`)
  - Deep filesystem structure analysis
  - Unknown filesystem detection
  - Corruption detection

- **Detect Filesystem Type** (`detect_filesystem_socket`)
  - Automatic filesystem type detection
  - Cached results to avoid repeated analysis

### ✅ Disk Conversion Operations
- **MBR/GPT Conversion** (via worker `Convert` command)
- **Disk Preparation** (via worker `Prepare` command)

## Single UAC Prompt Guarantee

### First Operation Flow
```
User Action (Clean/Format/Analyze)
    ↓
Moses starts TCP server
    ↓
Spawns elevated worker
    ↓
🔐 UAC PROMPT (only time!)
    ↓
Worker connects back
    ↓
Operation executes
    ↓
Worker stays connected
```

### Subsequent Operations Flow
```
User Action (Any privileged op)
    ↓
Moses checks connection
    ↓
Sends command to worker
    ↓
Operation executes
    ↓
NO UAC PROMPT! ✅
```

## Implementation Details

### Commands Migration Status
| Old Command | New Socket Command | Status |
|------------|-------------------|--------|
| `execute_format_elevated` | `format_disk_socket` | ✅ Complete |
| `clean_disk` (with elevation) | `clean_disk_socket` | ✅ Complete |
| `analyze_filesystem_elevated` | `analyze_filesystem_socket` | ✅ Complete |
| `detect_filesystem_elevated` | `detect_filesystem_socket` | ✅ Complete |
| `request_elevated_filesystem_detection` | `detect_filesystem_socket` | ✅ Complete |

### Worker Command Types
```rust
enum WorkerCommand {
    Format { device, options },      // Format operations
    Clean { device, options },        // Disk cleaning
    Analyze { device },              // Filesystem analysis & detection
    Convert { device, target_style }, // MBR/GPT conversion
    Prepare { device, ... },         // Disk preparation
    Ping,                           // Keepalive check
    Shutdown                        // Graceful shutdown
}
```

## Benefits of Unified Architecture

1. **User Experience**
   - Single UAC prompt per session
   - No repeated elevation requests
   - Seamless operation flow

2. **Security**
   - Worker validates elevation on startup
   - Worker exits if not elevated
   - Clear privilege boundary

3. **Performance**
   - No repeated process spawning
   - Instant command execution
   - Persistent connection

4. **Maintenance**
   - Single elevation path
   - Consistent error handling
   - Easy to add new privileged operations

## Testing the Architecture

### Manual Test
1. Start Moses
2. Click "Analyze Filesystem" on unknown drive
   - Should see ONE UAC prompt
   - Analysis completes
3. Click "Clean Disk"
   - NO UAC prompt
   - Clean executes immediately
4. Click "Format"
   - NO UAC prompt
   - Format executes immediately

### Automated Test
```powershell
# Run the socket worker test
.\test-socket-worker.ps1
```

## Adding New Privileged Operations

To add a new operation that requires elevation:

1. Add command to `WorkerCommand` enum in `worker_server.rs`
2. Add handler in worker's `handle_socket_mode` function
3. Create socket command in `disk_management_socket.rs`
4. Register command in `lib.rs`
5. Update UI to use socket command

Example:
```rust
// 1. Add to WorkerCommand
enum WorkerCommand {
    // ...
    MyNewOperation { device: Device, params: MyParams },
}

// 2. Handle in worker
WorkerCommand::MyNewOperation { device, params } => {
    // Perform privileged operation
}

// 3. Create Tauri command
#[tauri::command]
pub async fn my_operation_socket(device_id: String, params: MyParams) -> Result<String, String> {
    // Use worker server to execute
}
```

## Conclusion

ALL UAC-provoking actions now use the unified socket-based worker architecture. This means:
- ✅ Format operations use socket worker
- ✅ Clean operations use socket worker  
- ✅ Filesystem analysis uses socket worker
- ✅ Filesystem detection uses socket worker
- ✅ Future privileged operations will use socket worker

Users will see **exactly ONE UAC prompt** per Moses session, creating a smooth, professional experience.