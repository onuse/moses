fn main() {
    tauri_build::build();
    
    // On Windows, set nice metadata for the formatter executable
    #[cfg(target_os = "windows")]
    {
        // This will make the UAC prompt show a nicer name
        if std::env::var("CARGO_BIN_NAME").unwrap_or_default() == "moses-formatter" {
            println!("cargo:rustc-link-arg=/MANIFESTINPUT:moses-formatter.exe.manifest");
            println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        }
    }
}
