mod bedrock;

pub use bedrock::*;

/// Initialize all available backends
pub fn init() {
    println!("Initializing agent backends...");
}

/// Trait defining a language model backend core functionality
pub trait BackendCore: Send + Sync {
    /// Get the backend's name
    fn name(&self) -> &'static str;
    
    /// Get the backend's context window size
    fn context_window(&self) -> usize;
}

/// Trait defining the async operations for the backend
#[async_trait::async_trait]
pub trait Backend: BackendCore {
    /// Generate a response from the given prompt
    async fn generate_response(&self, prompt: &str) -> Result<BackendResponse, String>;
}

/// Structure containing a response from an LLM backend
pub struct BackendResponse {
    /// The text content of the response
    pub content: String,
    
    /// Model used for generation
    pub model: String,
    
    /// Tokens used in this request and response
    pub tokens_used: Option<usize>,
}
