use std::process::Command;
use std::env;
use std::os::windows::process::CommandExt;
use moses_core::MosesError;

/// Check if the current process is running with elevated privileges
pub fn is_elevated() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    
    unsafe {
        let process = GetCurrentProcess();
        let mut token_handle = windows::Win32::Foundation::HANDLE::default();
        
        if OpenProcessToken(process, TOKEN_QUERY, &mut token_handle).is_err() {
            return false;
        }
        
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut return_length = 0u32;
        
        let result = GetTokenInformation(
            token_handle,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );
        
        let _ = CloseHandle(token_handle);
        
        result.is_ok() && elevation.TokenIsElevated != 0
    }
}

/// Request UAC elevation by restarting the application with admin privileges
pub fn request_elevation_for_operation(operation: &str, device_path: &str) -> Result<bool, MosesError> {
    // If already elevated, return true
    if is_elevated() {
        return Ok(true);
    }
    
    // Get the current executable path
    let current_exe = env::current_exe()
        .map_err(|e| MosesError::Other(format!("Failed to get current executable: {}", e)))?;
    
    // Use Windows shell to request elevation
    let result = Command::new("powershell")
        .args(&[
            "-Command",
            &format!(
                "Start-Process '{}' -ArgumentList '--elevated-operation','{}','--device','{}' -Verb RunAs -Wait",
                current_exe.display(),
                operation,
                device_path
            )
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status()
        .map_err(|e| MosesError::Other(format!("Failed to request elevation: {}", e)))?;
    
    Ok(result.success())
}

/// Show a user-friendly elevation prompt
pub fn show_elevation_prompt(reason: &str) -> Result<bool, MosesError> {
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_YESNO, MB_ICONINFORMATION, IDYES};
    use windows::core::PCWSTR;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    
    let message = format!(
        "Moses Drive Formatter requires administrator privileges to {}.\n\n\
        Click Yes to continue with elevated privileges, or No to cancel.",
        reason
    );
    
    let title = "Administrator Privileges Required";
    
    // Convert strings to wide strings for Windows API
    let message_wide: Vec<u16> = OsStr::new(&message)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let title_wide: Vec<u16> = OsStr::new(title)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    unsafe {
        let result = MessageBoxW(
            None,
            PCWSTR::from_raw(message_wide.as_ptr()),
            PCWSTR::from_raw(title_wide.as_ptr()),
            MB_YESNO | MB_ICONINFORMATION,
        );
        
        Ok(result == IDYES)
    }
}