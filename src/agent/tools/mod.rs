mod executor;
mod filesystem;
mod registry;
mod types;

pub use executor::*;
pub use filesystem::*;
pub use registry::*;
pub use types::*;

/// Initialize the tools system
pub fn init() {
    println!("Initializing agent tools...");
}
