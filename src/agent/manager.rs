use crate::agent::backends::{Backend, BackendCore, BedrockBackend};
use crate::agent::context::ContextManager;
use crate::agent::tools::ToolRegistry;

/// Central manager for the AI agent
pub struct AgentManager {
    /// The currently active backend for LLM processing
    backend: BedrockBackend,

    /// Tool registry for managing available tools
    tool_registry: ToolRegistry,

    /// Context manager for maintaining conversation state
    context_manager: ContextManager,

    /// Configuration settings for the agent
    config: AgentConfig,
}

/// Configuration settings for the agent
pub struct AgentConfig {
    /// Whether to use the fast model for context management
    pub use_fast_model_for_context: bool,

    /// Maximum context length to maintain
    pub max_context_length: usize,

    /// Whether to automatically compress older context
    pub auto_compress_context: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            use_fast_model_for_context: true,
            max_context_length: 32000,
            auto_compress_context: true,
        }
    }
}

impl AgentManager {
    /// Create a new agent manager with default settings
    pub fn new() -> Self {
        Self {
            backend: BedrockBackend::new(),
            tool_registry: ToolRegistry::new(),
            context_manager: ContextManager::new(),
            config: AgentConfig::default(),
        }
    }

    /// Create a new agent manager with custom configuration
    pub fn with_config(config: AgentConfig) -> Self {
        Self {
            backend: BedrockBackend::new(),
            tool_registry: ToolRegistry::new(),
            context_manager: ContextManager::new(),
            config,
        }
    }

    /// Process user input and generate a response
    pub async fn process_input(&mut self, input: &str) -> Result<AgentResponse, String> {
        // First, update context with user input
        self.context_manager.add_user_message(input);

        // Prepare context for LLM
        let context = self.context_manager.get_context();

        // Process with LLM
        let backend_response = self
            .backend
            .generate_response(&context)
            .await
            .map_err(|e| format!("Backend error: {}", e))?;

        // Parse LLM response for tool calls
        let tool_calls = self.parse_tool_calls(&backend_response.content);

        // Execute any tool calls
        let tool_results = self.execute_tool_calls(tool_calls).await?;

        // Add assistant response to context
        self.context_manager
            .add_assistant_message(&backend_response.content);

        // Add tool results to context if any
        if !tool_results.is_empty() {
            self.context_manager.add_tool_results(&tool_results);
        }

        // Compress context if needed
        if self.config.auto_compress_context {
            self.maybe_compress_context().await?;
        }

        Ok(AgentResponse {
            content: backend_response.content,
            tool_results,
        })
    }

    /// Parse LLM response to extract tool calls
    fn parse_tool_calls(&self, response: &str) -> Vec<ToolCall> {
        // TODO: Implement parsing of tool calls from LLM response
        // This would look for patterns like:
        // <tool name="tool_name" args={...}> or similar format
        vec![]
    }

    /// Execute any tool calls found in the response
    async fn execute_tool_calls(
        &self,
        tool_calls: Vec<ToolCall>,
    ) -> Result<Vec<ToolResult>, String> {
        let mut results = Vec::new();

        for tool_call in tool_calls {
            let result = self
                .tool_registry
                .execute_tool(&tool_call.name, &tool_call.args)
                .await
                .map_err(|e| format!("Tool execution error: {}", e))?;

            results.push(ToolResult {
                tool_name: tool_call.name.clone(),
                result,
            });
        }

        Ok(results)
    }

    /// Compress context if it gets too large
    async fn maybe_compress_context(&mut self) -> Result<(), String> {
        if self.context_manager.context_length() > self.config.max_context_length {
            // Use the fast model (haiku) for context compression
            if self.config.use_fast_model_for_context {
                // TODO: Switch to fast model temporarily
                // For now, just use current backend
            }

            // Get the current context
            let context = self.context_manager.get_context();

            // Ask LLM to summarize older parts of context
            let summarization_prompt = format!(
                "Please summarize the following conversation concisely while preserving all important information:\n{}\n",
                context
            );

            let summary_response = self
                .backend
                .generate_response(&summarization_prompt)
                .await
                .map_err(|e| format!("Context compression error: {}", e))?;

            // Replace older context with summary
            self.context_manager
                .replace_with_summary(&summary_response.content);
        }

        Ok(())
    }
}

/// Structure representing a tool call extracted from LLM response
pub struct ToolCall {
    pub name: String,
    pub args: Vec<String>,
}

/// Structure representing the result of a tool execution
pub struct ToolResult {
    pub tool_name: String,
    pub result: String,
}

/// Structure representing a complete response from the agent
pub struct AgentResponse {
    pub content: String,
    pub tool_results: Vec<ToolResult>,
}
