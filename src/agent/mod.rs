mod manager;
mod context;
pub mod backends;
pub mod tools;

pub use manager::*;
pub use context::*;

// Agent initialization
pub fn init() {
    // Initialize agent components
    println!("Initializing agent components...");
    
    // Initialize tool registry
    tools::init();
    
    // Initialize backends
    backends::init();
}
