pub mod backends;
pub mod context;
pub mod manager;
pub mod tools;
pub mod app_recursive_processor;

pub use context::*;
pub use manager::*;
use tracing::trace;

// Agent initialization
pub fn init() {
    // Initialize agent components
    trace!("Initializing agent components...");

    // Initialize tool registry
    tools::init();

    // Initialize backends
    backends::init();
}
