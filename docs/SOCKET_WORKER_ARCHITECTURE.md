# Socket-Based Worker Architecture

## Overview
Moses uses a socket-based architecture to maintain a persistent elevated worker process, eliminating multiple UAC prompts during disk operations.

## Architecture Components

### 1. Worker Server (Moses Side)
- **Location**: `src-tauri/src/worker_server.rs`
- **Role**: TCP server that spawns and manages the elevated worker
- **Features**:
  - Binds to random localhost port
  - Spawns elevated worker with port number
  - Maintains persistent connection
  - Handles command serialization/deserialization
  - Automatic reconnection if worker dies

### 2. Elevated Worker (Client)
- **Location**: `src-tauri/src/bin/moses-elevated-worker.rs`
- **Role**: Elevated process that performs privileged operations
- **Features**:
  - Checks elevation on startup (exits if not elevated)
  - Connects back to Moses on specified port
  - Processes commands in a loop
  - Single UAC prompt on first launch
  - Stays alive for entire Moses session

### 3. Socket Commands
- **Location**: `src-tauri/src/commands/disk_management_socket.rs`
- **Available Commands**:
  - `clean_disk_socket`: Clean disk with various wipe methods
  - `format_disk_socket`: Format disk with specified filesystem
  - `detect_conflicts_socket`: Analyze disk for conflicts

## Communication Protocol

### Message Format
```json
// Command (Moses → Worker)
{
  "command": "Format",
  "params": {
    "device": {...},
    "options": {...}
  }
}

// Response (Worker → Moses)
{
  "status": "Success",
  "data": "Operation completed"
}
```

### Command Types
```rust
enum WorkerCommand {
    Format { device, options },
    Clean { device, options },
    Analyze { device },
    Convert { device, target_style },
    Prepare { device, target_style, clean_first },
    Ping,      // Keepalive
    Shutdown   // Graceful shutdown
}
```

## Benefits

1. **Single UAC Prompt**: User sees only one elevation request per session
2. **Persistent Connection**: Worker stays alive, ready for operations
3. **Guaranteed Elevation**: Worker existence proves admin rights
4. **Clean Architecture**: Clear separation of privileged operations
5. **Portable Design**: Can work on Linux/macOS with sudo/pkexec

## Usage Flow

1. **First Operation**:
   - User clicks "Clean Disk" or "Format"
   - Moses starts TCP server on random port
   - Moses spawns worker with `--socket <port>`
   - UAC prompt appears (Windows)
   - Worker checks elevation, connects back
   - Operation executes
   - Worker stays connected

2. **Subsequent Operations**:
   - Moses checks existing connection
   - Sends command directly
   - No UAC prompt needed
   - Immediate execution

## Implementation Details

### Worker Lifecycle
```
Moses Start → Worker Server Init → (Idle)
     ↓
First Disk Op → Spawn Worker → UAC Prompt
     ↓              ↓
   Accept ←── Worker Connects
     ↓
Execute Commands ←→ Worker Loop
     ↓
Moses Exit → Shutdown Command → Worker Exit
```

### Error Handling
- Worker exits if not elevated
- Automatic reconnection on connection loss
- Timeout handling for unresponsive worker
- Graceful degradation to direct execution

## Testing

Use the provided test script:
```powershell
.\test-socket-worker.ps1
```

Tests include:
1. Elevation check (worker exits without admin)
2. Socket connection verification
3. Ping/Pong communication
4. Graceful shutdown

## Security Considerations

1. **Localhost Only**: TCP server binds to 127.0.0.1
2. **Random Port**: Different port each session
3. **Elevation Check**: Worker verifies admin rights
4. **Process Isolation**: Privileged ops in separate process
5. **Clean Shutdown**: Proper cleanup on exit