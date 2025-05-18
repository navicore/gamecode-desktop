use crate::agent::backends::{Backend, BackendCore, BedrockBackend, BedrockModel};
use crate::agent::context::ContextManager;
use crate::agent::tools::ToolRegistry;
// Removed regex dependency
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Central manager for the AI agent
pub struct AgentManager {
    /// The currently active backend for LLM processing
    backend: BedrockBackend,

    /// Tool registry for managing available tools
    tool_registry: ToolRegistry,

    /// Context manager for maintaining conversation state
    pub context_manager: ContextManager,

    /// Configuration settings for the agent
    config: AgentConfig,

    /// Whether the backend is initialized
    initialized: bool,
}

/// Configuration settings for the agent
pub struct AgentConfig {
    /// Whether to use the fast model for context management
    pub use_fast_model_for_context: bool,

    /// Maximum context length to maintain
    pub max_context_length: usize,

    /// Whether to automatically compress older context
    pub auto_compress_context: bool,

    /// AWS region to use
    pub aws_region: String,

    /// AWS profile to use
    pub aws_profile: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            use_fast_model_for_context: true,
            max_context_length: 32000,
            auto_compress_context: true,
            aws_region: "us-east-1".to_string(),
            aws_profile: None,
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
            initialized: false,
        }
    }

    /// Create a new agent manager with custom configuration
    pub fn with_config(config: AgentConfig) -> Self {
        Self {
            backend: BedrockBackend::new(),
            tool_registry: ToolRegistry::new(),
            context_manager: ContextManager::new(),
            config,
            initialized: false,
        }
    }

    /// Register a tool with the agent
    pub fn register_tool(&mut self, tool: Box<dyn crate::agent::tools::Tool>) {
        self.tool_registry.register_tool(tool);
    }

    /// Set the working directory for tool execution
    pub fn set_working_directory(&mut self, directory: &str) {
        self.tool_registry.set_working_directory(directory);
    }

    /// Initialize the agent manager
    pub async fn init(&mut self) -> Result<(), String> {
        // Initialize backend with AWS configuration
        let mut backend_config = self.backend.config().clone();
        backend_config.region = self.config.aws_region.clone();
        if let Some(profile) = &self.config.aws_profile {
            backend_config.use_profile = true;
            backend_config.profile_name = Some(profile.clone());
        }

        // Create a new backend with updated config
        self.backend = BedrockBackend::with_config(backend_config);

        // Initialize the backend
        self.backend.init().await?;

        self.initialized = true;
        Ok(())
    }

    /// Process user input and generate a response
    pub async fn process_input(&mut self, input: &str) -> Result<AgentResponse, String> {
        info!("Processing user input: {} chars", input.len());

        // Check if backend is initialized
        if !self.initialized {
            return Err("Backend not initialized. Call init() first.".to_string());
        }

        // First, update context with user input
        self.context_manager.add_user_message(input);
        info!("Context updated with user message");

        // Prepare context for LLM
        let context = self.context_manager.get_context();
        info!("Prepared context for LLM: {} chars", context.len());

        // Process with LLM
        info!("Sending request to LLM backend...");
        let backend_response = self
            .backend
            .generate_response(&context)
            .await
            .map_err(|e| {
                error!("Backend error: {}", e);
                format!("Backend error: {}", e)
            })?;
        info!(
            "Received response from LLM: {} chars",
            backend_response.content.len()
        );

        // Get tool calls directly from the backend response
        info!("Processing tool calls from response");
        // Extract tool calls directly from the structured response
        let tool_calls: Vec<ToolCall> = backend_response
            .tool_calls
            .iter()
            .map(|tc| {
                let args = tc
                    .args
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();

                // Log the tool call ID to track it through the system
                if let Some(id) = &tc.id {
                    debug!("Received tool call with ID '{}' for tool '{}'", id, tc.name);
                } else {
                    warn!("Received tool call without ID for tool '{}'", tc.name);
                }

                ToolCall {
                    name: tc.name.clone(),
                    args,
                    args_json: Some(tc.args.clone()),
                    id: tc.id.clone(),
                }
            })
            .collect();

        info!("Found {} tool calls in backend response", tool_calls.len());
        info!("Processing {} tool calls", tool_calls.len());

        // Execute any tool calls
        let tool_results = if !tool_calls.is_empty() {
            info!("Executing tool calls");
            self.execute_tool_calls(tool_calls).await?
        } else {
            info!("No tool calls to execute");
            Vec::new()
        };

        // Add assistant response to context
        self.context_manager
            .add_assistant_message(&backend_response.content);
        info!("Added assistant response to context");

        // Add tool results to context if any
        if !tool_results.is_empty() {
            info!("Adding {} tool results to context", tool_results.len());
            self.context_manager.add_tool_results(&tool_results);
        }

        // Compress context if needed
        if self.config.auto_compress_context {
            self.maybe_compress_context().await?;
        }

        info!("Processing complete, returning response");
        Ok(AgentResponse {
            content: backend_response.content,
            tool_results,
        })
    }

    // Removed parse_tool_calls - Using structured tool calls directly

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

            // CRITICAL: Make sure we're preserving the original ID from Claude's tool_use block
            // This ID must match EXACTLY for Claude's API validation - even a single character difference will fail
            let tool_call_id = tool_call.id.clone();
            if let Some(id) = &tool_call_id {
                debug!(
                    "USING EXACT Claude-provided tool_use_id: '{}' for result of tool '{}'",
                    id, tool_call.name
                );
                debug!(
                    "ID MUST NOT be modified in any way - even a single character difference will cause validation to fail"
                );
            } else {
                // This should never happen with Claude tool calls, and will cause validation to fail
                warn!(
                    "CRITICAL ERROR: Missing tool ID for tool '{}', Claude will reject the result",
                    tool_call.name
                );
            }

            // Pass the exact same ID to the result
            results.push(ToolResult {
                tool_name: tool_call.name.clone(),
                result,
                tool_call_id: tool_call_id, // This must be passed unmodified to context.rs
            });
        }

        Ok(results)
    }

    /// Compress context if it gets too large
    async fn maybe_compress_context(&mut self) -> Result<(), String> {
        if self.context_manager.context_length() > self.config.max_context_length {
            // Store the original model
            let original_model = self.backend.current_model();

            // Use the fast model (haiku) for context compression
            if self.config.use_fast_model_for_context {
                // Switch to Haiku for summarization
                self.backend.switch_model(BedrockModel::Haiku);
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

            // Switch back to original model if we changed it
            if self.config.use_fast_model_for_context {
                self.backend.switch_model(original_model);
            }
        }

        Ok(())
    }
}

/// Structure representing a tool call extracted from LLM response
pub struct ToolCall {
    /// Name of the tool to call
    pub name: String,

    /// Arguments as strings (for backward compatibility)
    pub args: Vec<String>,

    /// Arguments as JSON (if available)
    pub args_json: Option<HashMap<String, Value>>,

    /// Tool call ID (if available)
    pub id: Option<String>,
}

/// Structure representing the result of a tool execution
pub struct ToolResult {
    /// Name of the tool that was executed
    pub tool_name: String,

    /// Result of the tool execution
    pub result: String,

    /// Tool use ID (if available) - IMPORTANT: This must match exactly the ID from the original tool_use message
    /// Internally we call it tool_call_id but when sending to Claude it must be sent as tool_use_id
    pub tool_call_id: Option<String>,
}

/// Structure representing a complete response from the agent
pub struct AgentResponse {
    pub content: String,
    pub tool_results: Vec<ToolResult>,
}
