mod agent;
mod app;
mod core;
mod examples;
mod ui;
mod visualization;

use std::env;

#[tokio::main]
async fn main() {
    // Check command-line arguments
    let args: Vec<String> = env::args().collect();
    
    // Check if debug flag is enabled
    let debug_mode = args.contains(&String::from("--debug"));
    
    if args.len() > 1 && args[1] == "--test-bedrock" {
        // Set up environment variable for logging
        if debug_mode {
            println!("Debug mode enabled");
            // SAFETY: We're just setting log levels which doesn't impact memory safety
            unsafe {
                std::env::set_var("RUST_LOG", "debug,gamecode=debug,aws_config=debug");
            }
        } else {
            // SAFETY: We're just setting log levels which doesn't impact memory safety
            unsafe {
                std::env::set_var("RUST_LOG", "info,gamecode=info,aws_config=info");
            }
        }
        
        println!("Running AWS Bedrock integration test...");
        println!("Use --debug flag for more verbose logging (e.g., cargo run -- --test-bedrock --debug)");
        
        // Run the Bedrock integration test
        if let Err(e) = examples::run_bedrock_example().await {
            eprintln!("Error in Bedrock example: {}", e);
        }
    } else {
        // Run the normal application
        app::run();
    }
}
