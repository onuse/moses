// Standalone analyzer binary that can be elevated
use std::env;
use moses_core::Device;
use moses_formatters::diagnostics::analyze_unknown_filesystem;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: moses-analyzer <device-json>");
        eprintln!("Error: Invalid arguments");
        std::process::exit(1);
    }
    
    // Parse device from JSON
    let device: Device = match serde_json::from_str(&args[1]) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: Failed to parse device JSON: {}", e);
            std::process::exit(1);
        }
    };
    
    // Run analysis
    match analyze_unknown_filesystem(&device) {
        Ok(report) => {
            println!("{}", report);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error: Analysis failed: {:?}", e);
            std::process::exit(1);
        }
    }
}