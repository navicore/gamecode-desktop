pub mod backends;
mod context;
mod manager;
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
