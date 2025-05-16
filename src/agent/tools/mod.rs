mod registry;
mod executor;
mod types;

pub use registry::*;
pub use executor::*;
pub use types::*;

/// Initialize the tools system
pub fn init() {
    println!("Initializing agent tools...");
}
