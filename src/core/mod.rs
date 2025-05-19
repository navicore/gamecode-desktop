mod agent;
mod state;
mod tools;

pub use agent::*;
pub use state::*;
pub use tools::*;
use tracing::trace;

// Core functionality initialization
pub fn init() {
    // TODO: Initialize core systems
    trace!("Initializing core systems...");
}
