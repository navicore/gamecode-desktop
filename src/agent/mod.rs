pub mod backends;
pub mod context;
pub mod manager;
pub mod tools;

pub use context::*;
pub use manager::*;

// Agent initialization
pub fn init() {
    // Initialize agent components
    println!("Initializing agent components...");

    // Initialize tool registry
    tools::init();

    // Initialize backends
    backends::init();
}
