# Test script for socket-based worker architecture
Write-Host "Socket Worker Test Script" -ForegroundColor Cyan
Write-Host "=========================" -ForegroundColor Cyan

# Check if Moses is built
$mosesExe = ".\target\release\moses.exe"
$workerExe = ".\target\release\moses-worker.exe"

if (-not (Test-Path $mosesExe)) {
    Write-Host "Error: Moses not found at $mosesExe" -ForegroundColor Red
    Write-Host "Run 'cargo build --release' first" -ForegroundColor Yellow
    exit 1
}

if (-not (Test-Path $workerExe)) {
    Write-Host "Error: Worker not found at $workerExe" -ForegroundColor Red
    Write-Host "Run 'cargo build --release' first" -ForegroundColor Yellow
    exit 1
}

# Test 1: Worker elevation check
Write-Host "`nTest 1: Worker Elevation Check" -ForegroundColor Green
Write-Host "-------------------------------"
Write-Host "Starting worker without elevation (should exit immediately)..."

$proc = Start-Process -FilePath $workerExe -ArgumentList "--socket", "12345" -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 1

if ($proc.HasExited) {
    Write-Host "✓ Worker correctly exited when not elevated" -ForegroundColor Green
} else {
    Write-Host "✗ Worker is still running (unexpected)" -ForegroundColor Red
    Stop-Process -Id $proc.Id -Force
}

# Test 2: Worker with elevation
Write-Host "`nTest 2: Worker Socket Connection" -ForegroundColor Green
Write-Host "---------------------------------"
Write-Host "Starting elevated worker on port 12346..."
Write-Host "(You will see a UAC prompt)" -ForegroundColor Yellow

# Start elevated worker
$startInfo = New-Object System.Diagnostics.ProcessStartInfo
$startInfo.FileName = $workerExe
$startInfo.Arguments = "--socket 12346"
$startInfo.Verb = 'runas'
$startInfo.UseShellExecute = $true
$startInfo.WindowStyle = 'Hidden'

try {
    $elevatedProc = [System.Diagnostics.Process]::Start($startInfo)
    Write-Host "✓ Elevated worker started (PID: $($elevatedProc.Id))" -ForegroundColor Green
    
    # Give it time to start
    Start-Sleep -Seconds 2
    
    # Test TCP connection
    Write-Host "`nTesting TCP connection to port 12346..."
    try {
        $tcpClient = New-Object System.Net.Sockets.TcpClient
        $tcpClient.Connect("127.0.0.1", 12346)
        
        if ($tcpClient.Connected) {
            Write-Host "✓ Successfully connected to worker on port 12346" -ForegroundColor Green
            
            # Send a ping command
            Write-Host "`nSending Ping command..."
            $stream = $tcpClient.GetStream()
            $writer = New-Object System.IO.StreamWriter($stream)
            $reader = New-Object System.IO.StreamReader($stream)
            
            $pingCmd = '{"command":"Ping"}'
            $writer.WriteLine($pingCmd)
            $writer.Flush()
            
            # Read response
            $response = $reader.ReadLine()
            Write-Host "Response: $response" -ForegroundColor Cyan
            
            if ($response -like '*Pong*') {
                Write-Host "✓ Worker responded correctly to Ping" -ForegroundColor Green
            }
            
            # Send shutdown command
            Write-Host "`nSending Shutdown command..."
            $shutdownCmd = '{"command":"Shutdown"}'
            $writer.WriteLine($shutdownCmd)
            $writer.Flush()
            
            $tcpClient.Close()
            Write-Host "✓ Connection closed" -ForegroundColor Green
        }
    } catch {
        Write-Host "✗ Failed to connect: $_" -ForegroundColor Red
    }
    
    # Wait for worker to exit
    Start-Sleep -Seconds 1
    
    if ($elevatedProc.HasExited) {
        Write-Host "✓ Worker shut down gracefully" -ForegroundColor Green
    } else {
        Write-Host "! Worker still running, terminating..." -ForegroundColor Yellow
        Stop-Process -Id $elevatedProc.Id -Force
    }
    
} catch {
    Write-Host "✗ Failed to start elevated worker: $_" -ForegroundColor Red
}

Write-Host "`n=========================" -ForegroundColor Cyan
Write-Host "Socket Worker Test Complete" -ForegroundColor Cyan