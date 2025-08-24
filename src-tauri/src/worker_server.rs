// Socket-based communication with elevated worker process
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde::{Deserialize, Serialize};
use moses_core::{Device, FormatOptions};
use moses_filesystems::disk_manager::CleanOptions;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", content = "params")]
pub enum WorkerCommand {
    Format {
        device: Device,
        options: FormatOptions,
    },
    Clean {
        device: Device,
        options: CleanOptions,
    },
    Analyze {
        device: Device,
    },
    Convert {
        device: Device,
        target_style: String,
    },
    Prepare {
        device: Device,
        target_style: String,
        clean_first: bool,
    },
    ReadDirectory {
        device: Device,
        path: String,
    },
    Ping, // Keepalive
    Shutdown, // Graceful shutdown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum WorkerResponse {
    Success(String),
    Error(String),
    Progress { percent: u8, message: String },
    Log { level: String, message: String },
    DirectoryListing(String), // JSON serialized directory listing
    Pong,
}

pub struct WorkerServer {
    listener: Option<TcpListener>,
    connection: Arc<Mutex<Option<TcpStream>>>,
    port: u16,
    log_sender: Arc<Mutex<Option<mpsc::UnboundedSender<(String, String)>>>>,
    spawning: Arc<Mutex<bool>>,
}

impl WorkerServer {
    pub async fn new() -> Result<Self, String> {
        // Bind to any available port on localhost
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("Failed to bind TCP listener: {}", e))?;
        
        let port = listener.local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?
            .port();
        
        log::info!("Worker server listening on port {}", port);
        
        Ok(Self {
            listener: Some(listener),
            connection: Arc::new(Mutex::new(None)),
            port,
            log_sender: Arc::new(Mutex::new(None)),
            spawning: Arc::new(Mutex::new(false)),
        })
    }
    
    #[allow(dead_code)]
    pub fn port(&self) -> u16 {
        self.port
    }
    
    /// Ensure the worker is connected, spawning it if necessary
    pub async fn ensure_connected(&self) -> Result<(), String> {
        // Loop to handle waiting for another thread's spawn
        loop {
            // First check if we have a working connection
            {
                let mut conn = self.connection.lock().await;
                
                // Check if we already have a connection
                if let Some(ref mut stream) = *conn {
                    // Try to set TCP keepalive to detect broken connections
                    let _ = stream.set_nodelay(true);
                    
                    log::info!("Checking existing worker connection...");
                    // Send a ping to check if connection is alive
                    match self.ping_worker(stream).await {
                        Ok(()) => {
                            log::info!("Worker connection is alive");
                            return Ok(());
                        },
                        Err(e) => {
                            log::warn!("Worker ping failed: {}, will reconnect...", e);
                            // Connection is dead, remove it
                            *conn = None;
                        }
                    }
                } else {
                    log::info!("No existing worker connection, will spawn new worker");
                }
            }
            
            // Check if another thread is already spawning
            {
                let mut spawning = self.spawning.lock().await;
                if *spawning {
                    log::info!("Another thread is already spawning a worker, waiting...");
                    drop(spawning);
                    
                    // Wait for the other thread to finish spawning
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    
                    // Check if connection is now available
                    let conn = self.connection.lock().await;
                    if conn.is_some() {
                        log::info!("Connection established by another thread");
                        return Ok(());
                    }
                    
                    // If still no connection, continue the loop to try again
                    continue;
                }
                
                // Mark that we're spawning
                *spawning = true;
            }
            
            // Spawn the elevated worker
            let spawn_result = self.spawn_elevated_worker().await;
            
            // Clear the spawning flag regardless of result
            {
                let mut spawning = self.spawning.lock().await;
                *spawning = false;
            }
            
            spawn_result?;
            
            // Store the new connection
            let mut conn = self.connection.lock().await;
            
            // Accept the connection (with timeout)
            if let Some(listener) = &self.listener {
                let accept_future = listener.accept();
                let timeout = tokio::time::timeout(Duration::from_secs(30), accept_future);
                
                match timeout.await {
                    Ok(Ok((stream, addr))) => {
                        log::info!("Worker connected from {}", addr);
                        *conn = Some(stream);
                        return Ok(());
                    }
                    Ok(Err(e)) => return Err(format!("Failed to accept connection: {}", e)),
                    Err(_) => return Err("Worker connection timeout".to_string()),
                }
            } else {
                return Err("Listener not available".to_string());
            }
        }
    }
    
    /// Send a command to the worker and get response
    pub async fn execute_command(&self, command: WorkerCommand) -> Result<WorkerResponse, String> {
        // Try up to 2 times in case of connection issues
        for attempt in 0..2 {
            match self.execute_command_internal(&command).await {
                Ok(response) => return Ok(response),
                Err(e) if e.contains("10054") || e.contains("broken pipe") || e.contains("connection") => {
                    log::warn!("Connection error on attempt {}: {}", attempt + 1, e);
                    // Reset connection and retry
                    {
                        let mut conn = self.connection.lock().await;
                        *conn = None;
                    }
                    if attempt == 0 {
                        log::info!("Reconnecting to worker...");
                        continue;
                    }
                    return Err(format!("Worker connection failed after retry: {}", e));
                }
                Err(e) => return Err(e),
            }
        }
        Err("Failed to execute command after retries".to_string())
    }
    
    /// Internal implementation of execute_command
    async fn execute_command_internal(&self, command: &WorkerCommand) -> Result<WorkerResponse, String> {
        self.ensure_connected().await?;
        
        let mut conn = self.connection.lock().await;
        let stream = conn.as_mut().ok_or("No worker connection")?;
        
        // Send command
        let cmd_json = serde_json::to_string(command)
            .map_err(|e| format!("Failed to serialize command: {}", e))?;
        
        stream.write_all(cmd_json.as_bytes()).await
            .map_err(|e| format!("Failed to send command: {}", e))?;
        stream.write_all(b"\n").await
            .map_err(|e| format!("Failed to send newline: {}", e))?;
        stream.flush().await
            .map_err(|e| format!("Failed to flush: {}", e))?;
        
        // Read response, filtering out log messages
        let mut reader = BufReader::new(stream);
        loop {
            let mut response_line = String::new();
            reader.read_line(&mut response_line).await
                .map_err(|e| format!("Failed to read response: {}", e))?;
            
            let response: WorkerResponse = serde_json::from_str(&response_line)
                .map_err(|e| format!("Failed to parse response: {}", e))?;
            
            match response {
                WorkerResponse::Log { level, message } => {
                    // Forward log to system logger
                    log::log!(
                        match level.as_str() {
                            "ERROR" => log::Level::Error,
                            "WARN" => log::Level::Warn,
                            "INFO" => log::Level::Info,
                            "DEBUG" => log::Level::Debug,
                            _ => log::Level::Trace,
                        },
                        "[Worker] {}",
                        message
                    );
                    
                    // Store log for UI if we have a sender
                    if let Some(ref sender) = *self.log_sender.lock().await {
                        let _ = sender.send((level, message));
                    }
                    // Continue reading for the actual response
                }
                _ => return Ok(response), // This is the actual command response
            }
        }
    }
    
    /// Ping the worker to check if it's alive
    async fn ping_worker(&self, stream: &mut TcpStream) -> Result<(), String> {
        let ping = serde_json::to_string(&WorkerCommand::Ping)
            .map_err(|e| format!("Failed to serialize ping: {}", e))?;
        
        stream.write_all(ping.as_bytes()).await
            .map_err(|e| format!("Failed to send ping: {}", e))?;
        stream.write_all(b"\n").await
            .map_err(|e| format!("Failed to send newline: {}", e))?;
        stream.flush().await
            .map_err(|e| format!("Failed to flush: {}", e))?;
        
        // Try to read pong response with timeout
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        
        let read_future = reader.read_line(&mut response_line);
        let timeout = tokio::time::timeout(Duration::from_secs(2), read_future);
        
        match timeout.await {
            Ok(Ok(_)) => {
                let response: WorkerResponse = serde_json::from_str(&response_line)
                    .map_err(|e| format!("Invalid pong response: {}", e))?;
                
                match response {
                    WorkerResponse::Pong => Ok(()),
                    _ => Err("Unexpected response to ping".to_string()),
                }
            }
            _ => Err("Ping timeout".to_string()),
        }
    }
    
    /// Spawn the elevated worker process
    async fn spawn_elevated_worker(&self) -> Result<(), String> {
        log::info!("Spawning elevated worker process...");
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            use std::env;
            use std::os::windows::process::CommandExt;
            use moses_platform::windows::elevation::is_elevated;
            
            let worker_exe = env::current_exe()
                .map_err(|e| format!("Failed to get executable path: {}", e))?
                .parent()
                .ok_or_else(|| "Failed to get executable directory".to_string())?
                .join("moses-worker.exe");
            
            if is_elevated() {
                // Already elevated, spawn directly
                Command::new(&worker_exe)
                    .arg("--socket")
                    .arg(self.port.to_string())
                    .spawn()
                    .map_err(|e| format!("Failed to spawn worker: {}", e))?;
            } else {
                // Request elevation via PowerShell
                let ps_script = format!(
                    r#"
                    $worker = '{}'
                    $port = '{}'
                    
                    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
                    $startInfo.FileName = $worker
                    $startInfo.Arguments = "--socket $port"
                    $startInfo.Verb = 'runas'
                    $startInfo.UseShellExecute = $true
                    
                    try {{
                        $process = [System.Diagnostics.Process]::Start($startInfo)
                        Write-Output "Worker started"
                        exit 0
                    }} catch {{
                        Write-Error "Failed to start elevated worker: $_"
                        exit 1
                    }}
                    "#,
                    worker_exe.display(),
                    self.port
                );
                
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                let output = Command::new("powershell")
                    .args(&[
                        "-NoProfile",
                        "-ExecutionPolicy", "Bypass",
                        "-Command", &ps_script
                    ])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output()
                    .map_err(|e| format!("Failed to run PowerShell: {}", e))?;
                
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("Failed to spawn elevated worker: {}", stderr));
                }
            }
            
            Ok(())
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // On Unix, use pkexec or sudo
            use std::process::Command;
            use std::env;
            
            let worker_exe = env::current_exe()
                .map_err(|e| format!("Failed to get executable path: {}", e))?
                .parent()
                .ok_or_else(|| "Failed to get executable directory".to_string())?
                .join("moses-worker");
            
            // Try pkexec first, then sudo
            let result = Command::new("pkexec")
                .arg(&worker_exe)
                .arg("--socket")
                .arg(self.port.to_string())
                .spawn();
            
            if result.is_err() {
                Command::new("sudo")
                    .arg(&worker_exe)
                    .arg("--socket")
                    .arg(self.port.to_string())
                    .spawn()
                    .map_err(|e| format!("Failed to spawn worker with sudo: {}", e))?;
            }
            
            Ok(())
        }
    }
    
    /// Shutdown the worker gracefully
    #[allow(dead_code)]
    pub async fn shutdown(&self) -> Result<(), String> {
        let mut conn = self.connection.lock().await;
        
        if let Some(ref mut stream) = *conn {
            // Send shutdown command
            let _ = self.execute_command(WorkerCommand::Shutdown).await;
            
            // Close the connection
            let _ = stream.shutdown().await;
        }
        
        *conn = None;
        Ok(())
    }
    
    /// Set up log streaming channel
    #[allow(dead_code)]
    pub fn setup_log_channel(&self) -> mpsc::UnboundedReceiver<(String, String)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let sender_arc = self.log_sender.clone();
        
        tokio::spawn(async move {
            let mut guard = sender_arc.lock().await;
            *guard = Some(tx);
        });
        
        rx
    }
}

// Global instance of the worker server
use once_cell::sync::Lazy;

pub static WORKER_SERVER: Lazy<Arc<Mutex<Option<WorkerServer>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Initialize the worker server
pub async fn init_worker_server() -> Result<(), String> {
    let server = WorkerServer::new().await?;
    let mut guard = WORKER_SERVER.lock().await;
    *guard = Some(server);
    Ok(())
}

/// Get the worker server instance
pub async fn get_worker_server() -> Result<Arc<Mutex<Option<WorkerServer>>>, String> {
    let guard = WORKER_SERVER.lock().await;
    if guard.is_none() {
        drop(guard);
        init_worker_server().await?;
    }
    Ok(WORKER_SERVER.clone())
}