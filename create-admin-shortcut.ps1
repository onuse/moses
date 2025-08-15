# PowerShell script to create a shortcut that runs Moses with admin privileges

$scriptPath = Split-Path -Parent $MyInvocation.MyCommand.Path
$exePath = Join-Path $scriptPath "src-tauri\target\release\moses.exe"
$shortcutPath = Join-Path $scriptPath "Moses Drive Formatter (Admin).lnk"

# Check if exe exists
if (-not (Test-Path $exePath)) {
    Write-Host "Moses.exe not found at: $exePath" -ForegroundColor Red
    Write-Host "Please build the project first with: cargo build --release" -ForegroundColor Yellow
    exit 1
}

# Create shortcut
$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut($shortcutPath)
$Shortcut.TargetPath = "powershell.exe"
$Shortcut.Arguments = "-WindowStyle Hidden -Command `"Start-Process '$exePath' -Verb RunAs`""
$Shortcut.WorkingDirectory = $scriptPath
$Shortcut.IconLocation = $exePath
$Shortcut.Description = "Moses Drive Formatter - Run as Administrator"
$Shortcut.Save()

Write-Host "Created admin shortcut: $shortcutPath" -ForegroundColor Green
Write-Host "You can now double-click the shortcut to run Moses with admin privileges." -ForegroundColor Cyan