use crate::agent::backends::{Backend, BackendCore, BackendResponse};
use crate::agent::tools::ExecuteCommandTool;
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::{error::SdkError, operation::invoke_model::InvokeModelError, Client};
use aws_smithy_types::Blob;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, trace, warn};
use uuid;

/// AWS Bedrock implementation of the Backend trait
pub struct BedrockBackend {
    /// Configuration for the Bedrock backend
    config: BedrockConfig,

    /// Currently selected model
    current_model: BedrockModel,

    /// Bedrock client
    client: Option<Arc<Client>>,
}

/// Available Bedrock models
#[derive(Clone, Copy, Debug)]
pub enum BedrockModel {
    /// Claude 3.7 Sonnet - for primary interactions
    Sonnet,

    /// Claude 3.5 Haiku - for context management and summarization
    Haiku,
}

/// Configuration for the Bedrock backend
#[derive(Clone)]
pub struct BedrockConfig {
    /// AWS region to use
    pub region: String,

    /// Maximum token limit for each model
    pub sonnet_token_limit: usize,
    pub haiku_token_limit: usize,

    /// Temperature setting for each model
    pub sonnet_temperature: f32,
    pub haiku_temperature: f32,

    /// Maximum tokens to generate in a response
    pub max_tokens: usize,

    /// Whether to use AWS profile for authentication
    pub use_profile: bool,

    /// AWS profile name to use
    pub profile_name: Option<String>,

    /// Number of retries for API calls
    pub max_retries: usize,
}

impl Default for BedrockConfig {
    fn default() -> Self {
        Self {
            //region: "us-east-1".to_string(),
            region: "us-west-2".to_string(),
            sonnet_token_limit: 28000,
            haiku_token_limit: 28000,
            sonnet_temperature: 0.7,
            haiku_temperature: 0.3,
            max_tokens: 4096,
            use_profile: true,
            profile_name: None,
            max_retries: 3,
        }
    }
}

/// Request structure for Claude API
#[derive(Serialize, Debug)]
struct ClaudeRequest {
    /// Holds an array of Message objects
    messages: Vec<ClaudeMessage>,

    /// System prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,

    /// Max tokens to generate
    max_tokens: usize,

    /// Temperature (0-1)
    temperature: f32,

    /// Tool definitions available to the model
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ClaudeTool>>,

    /// Control how the model uses tools
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,

    // Tool results are now embedded directly in messages as content blocks
    /// Anthropic API version
    anthropic_version: String,
}

/// Message structure for Claude API
#[derive(Serialize, Debug, Clone)]
struct ClaudeMessage {
    /// Role (user or assistant)
    role: String,

    /// Content blocks for the message
    content: Vec<ClaudeContentBlock>,
}

/// Content block for Claude API
#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
enum ClaudeContentBlock {
    /// Text content
    Text {
        #[serde(rename = "type")]
        content_type: String,
        text: String,
    },

    /// Tool use content
    ToolUse {
        #[serde(rename = "type")]
        content_type: String,
        id: String,
        name: String,
        input: HashMap<String, Value>,
    },

    /// Tool result content
    ToolResult {
        #[serde(rename = "type")]
        content_type: String,

        #[serde(rename = "tool_use_id")]
        tool_use_id: String,
        content: Value,
    },
}

// Tool results are now embedded directly in messages as content blocks
// The ToolResultBlock struct is no longer needed

/// Tool definition for Claude API
#[derive(Serialize, Debug)]
struct ClaudeTool {
    /// Tool name
    name: String,

    /// Tool description
    description: String,

    /// Tool input schema
    input_schema: Value,
}

/// Claude API response
#[derive(Deserialize, Debug)]
struct ClaudeResponse {
    /// Response ID
    //id: String,

    /// Content blocks
    content: Vec<ClaudeResponseContent>,

    /// Model used
    model: String,

    /// Usage information
    usage: ClaudeUsage,
}

impl ClaudeResponse {}

/// Content block in Claude response
#[derive(Deserialize, Debug)]
struct ClaudeResponseContent {
    /// Type of content
    #[serde(rename = "type")]
    content_type: String,

    /// Text content (if type is text)
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,

    /// Tool use (if type is tool_use)
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,

    /// Tool name (if type is tool_use)
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    /// Tool input (if type is tool_use)
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<HashMap<String, Value>>,
}

/// Usage information in Claude response
#[derive(Deserialize, Debug)]
struct ClaudeUsage {
    /// Input tokens
    input_tokens: usize,

    /// Output tokens
    output_tokens: usize,
}

/// Tool use structure representing a tool call from the LLM
#[derive(Debug, Clone)]
pub struct ToolUse {
    /// Tool name
    pub name: String,

    /// Tool arguments as JSON
    pub args: HashMap<String, Value>,

    /// Tool call ID (from Claude response)
    pub id: Option<String>,
}

impl BedrockBackend {
    /// Create a new Bedrock backend with default settings
    pub fn new() -> Self {
        Self {
            config: BedrockConfig::default(),
            current_model: BedrockModel::Sonnet,
            client: None,
        }
    }

    /// Create a new Bedrock backend with custom configuration
    pub fn with_config(config: BedrockConfig) -> Self {
        Self {
            config,
            current_model: BedrockModel::Sonnet,
            client: None,
        }
    }

    /// Get a reference to the current configuration
    pub fn config(&self) -> &BedrockConfig {
        &self.config
    }

    /// Get the current model
    pub fn current_model(&self) -> BedrockModel {
        self.current_model
    }

    /// Initialize the Bedrock client
    pub async fn init(&mut self) -> Result<(), String> {
        // Single initialization log with key details
        let profile_info = if let Some(profile) = &self.config.profile_name {
            format!("profile '{}' in region '{}'", profile, self.config.region)
        } else {
            format!("default profile in region '{}'", self.config.region)
        };

        info!("Initializing AWS Bedrock client with {}", profile_info);

        // Configure AWS client
        let aws_config = if self.config.use_profile {
            let mut builder = aws_config::defaults(BehaviorVersion::latest());

            if let Some(profile) = &self.config.profile_name {
                builder = builder.profile_name(profile);
            }

            builder = builder.region(aws_config::Region::new(self.config.region.clone()));
            builder.load().await
        } else {
            aws_config::defaults(BehaviorVersion::latest())
                .region(aws_config::Region::new(self.config.region.clone()))
                .load()
                .await
        };

        // Create and store client
        let client = aws_sdk_bedrockruntime::Client::new(&aws_config);
        self.client = Some(Arc::new(client));

        trace!("AWS Bedrock client initialized successfully");
        Ok(())
    }

    /// Switch to a different model
    pub fn switch_model(&mut self, model: BedrockModel) {
        self.current_model = model;
    }

    /// Get the current model's token limit
    pub fn current_model_token_limit(&self) -> usize {
        match self.current_model {
            BedrockModel::Sonnet => self.config.sonnet_token_limit,
            BedrockModel::Haiku => self.config.haiku_token_limit,
        }
    }

    /// Get the current model's temperature
    pub fn current_model_temperature(&self) -> f32 {
        match self.current_model {
            BedrockModel::Sonnet => self.config.sonnet_temperature,
            BedrockModel::Haiku => self.config.haiku_temperature,
        }
    }

    /// Get the current model's name as a string
    pub fn current_model_name(&self) -> &'static str {
        match self.current_model {
            BedrockModel::Sonnet => "us.anthropic.claude-3-7-sonnet-20250219-v1:0",
            BedrockModel::Haiku => "anthropic.claude-3-5-haiku-20240307-v1:0",
        }
    }

    /// Pretty print a serializable value as JSON
    fn pretty_print_json<T: Serialize>(&self, value: &T) -> Result<String, String> {
        match serde_json::to_string_pretty(value) {
            Ok(json) => Ok(json),
            Err(e) => Err(format!("Failed to pretty-print JSON: {}", e)),
        }
    }

    /// Parse error from AWS Bedrock API
    fn parse_error(&self, err: SdkError<InvokeModelError>) -> String {
        match err {
            SdkError::ServiceError(context) => {
                let err = context.err();

                match err {
                    InvokeModelError::AccessDeniedException(e) => {
                        format!("Access denied: {}", e)
                    }
                    InvokeModelError::InternalServerException(e) => {
                        format!("Internal server error: {}", e)
                    }
                    InvokeModelError::ModelNotReadyException(e) => {
                        format!("Model not ready: {}", e)
                    }
                    InvokeModelError::ModelTimeoutException(e) => {
                        format!("Model timeout: {}", e)
                    }
                    InvokeModelError::ResourceNotFoundException(e) => {
                        format!("Resource not found: {}", e)
                    }
                    InvokeModelError::ServiceQuotaExceededException(e) => {
                        format!("Service quota exceeded: {}", e)
                    }
                    InvokeModelError::ThrottlingException(e) => {
                        format!("Throttling error: {}", e)
                    }
                    InvokeModelError::ValidationException(e) => {
                        format!("Validation error: {}", e)
                    }
                    _ => format!("Unknown service error: {:?}", err),
                }
            }
            SdkError::ConstructionFailure(err) => format!("Construction failure: {:?}", err),
            SdkError::DispatchFailure(err) => format!("Dispatch failure: {:?}", err),
            SdkError::ResponseError(err) => format!("Response error: {:?}", err),
            SdkError::TimeoutError(err) => format!("Timeout error: {:?}", err),
            _ => format!("Unknown error: {:?}", err),
        }
    }

    /// Construct a Claude API request from a prompt and optional tool results
    fn construct_claude_request(&self, prompt: &str) -> Result<ClaudeRequest, String> {
        // Parse the conversation history from the prompt
        // The prompt comes from the ContextManager as a formatted string that includes:
        // - System messages (<s>...</s>)
        // - User messages (<user>...</user>)
        // - Assistant messages (<assistant>...</assistant>)
        // - Tool results in JSON format ({"type": "tool_result", ...})

        // Parse conversation history and extract tool results
        let (mut messages, tool_results) = self.parse_conversation_history(prompt)?;

        trace!("Created Claude request with {} messages", messages.len());
        for (i, msg) in messages.iter().enumerate() {
            let content_types: Vec<&str> = msg
                .content
                .iter()
                .map(|c| match c {
                    ClaudeContentBlock::Text { .. } => "text",
                    ClaudeContentBlock::ToolUse { .. } => "tool_use",
                    ClaudeContentBlock::ToolResult { .. } => "tool_result",
                })
                .collect();
            trace!(
                "Message {}: role={}, content_types={:?}",
                i,
                msg.role,
                content_types
            );
        }

        // Create tool schemas for the available tools
        let tools = Some(vec![
            ClaudeTool {
                name: "read_file".to_string(),
                description: "Read the contents of a file from the filesystem".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ClaudeTool {
                name: "write_file".to_string(),
                description: "Write content to a file on the filesystem".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
            ClaudeTool {
                name: "list_directory".to_string(),
                description: "List files and directories in a specified path".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the directory to list (optional, uses working directory if not specified)"
                        }
                    }
                }),
            },
            {
                // Create the execute_command tool with dynamic description based on allowed commands
                let allowed_cmd_list = ExecuteCommandTool::allowed_commands().join(", ");
                let description = format!(
                    "Execute a shell command (limited to safe commands: {})",
                    allowed_cmd_list
                );

                // Create the schema with dynamic command description
                let mut schema_properties = serde_json::Map::new();
                let mut command_property = serde_json::Map::new();

                command_property.insert(
                    "type".to_string(),
                    serde_json::Value::String("string".to_string()),
                );

                command_property.insert(
                    "description".to_string(),
                    serde_json::Value::String(format!(
                        "Command to execute with arguments. Only these commands are allowed: {}",
                        allowed_cmd_list
                    )),
                );

                schema_properties.insert(
                    "command".to_string(),
                    serde_json::Value::Object(command_property),
                );

                let mut schema = serde_json::Map::new();
                schema.insert(
                    "type".to_string(),
                    serde_json::Value::String("object".to_string()),
                );
                schema.insert(
                    "properties".to_string(),
                    serde_json::Value::Object(schema_properties),
                );
                schema.insert(
                    "required".to_string(),
                    serde_json::Value::Array(vec![serde_json::Value::String(
                        "command".to_string(),
                    )]),
                );

                ClaudeTool {
                    name: "execute_command".to_string(),
                    description,
                    input_schema: serde_json::Value::Object(schema),
                }
            },
        ]);

        // Security-focused system prompt
        let system_prompt = "You are a helpful AI assistant who has access to the user's computer through tools. \
        When using tools, prefer relative paths rather than absolute paths for security. \
        Whenever possible, use the current working directory rather than specifying absolute paths. \
        Answer questions and help with tasks efficiently and securely.";

        // Organize messages to maintain the conversation flow with tool results

        // Collect messages that have tool use blocks, so we can make sure they're followed by
        // appropriate tool result messages
        let mut tool_use_blocks = Vec::new();
        let mut has_tool_result_blocks = false;

        // First, identify tool use blocks and whether we already have tool results
        for message in &messages {
            for block in &message.content {
                if let ClaudeContentBlock::ToolUse { id, .. } = block {
                    tool_use_blocks.push(id.clone());
                } else if let ClaudeContentBlock::ToolResult { .. } = block {
                    has_tool_result_blocks = true;
                }
            }
        }

        trace!(
            "Found {} tool use blocks and {} tool result blocks",
            tool_use_blocks.len(),
            if has_tool_result_blocks { "some" } else { "no" }
        );

        // Process any tool results we collected during parsing
        let has_collected_tool_results = !tool_results.is_empty();

        // If we have tool use blocks but no tool result blocks, we need to add them
        if !tool_use_blocks.is_empty() && !has_tool_result_blocks && has_collected_tool_results {
            trace!(
                "Found {} tool_use blocks and {} collected tool results - inserting tool results",
                tool_use_blocks.len(),
                tool_results.len()
            );

            // Now, restructure the messages to include tool results properly
            let mut final_messages = Vec::new();

            // Completely rebuild the message sequence to ensure each assistant message with tool_use
            // is immediately followed by a user message with matching tool_result
            
            // First pass: collect each message and check for tool_use blocks
            let mut i = 0;
            while i < messages.len() {
                let message = messages[i].clone();
                
                // Check if this is an assistant message with tool_use blocks
                let tool_use_ids: Vec<String> = if message.role == "assistant" {
                    message.content.iter()
                        .filter_map(|block| {
                            if let ClaudeContentBlock::ToolUse { id, .. } = block {
                                Some(id.clone())
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                
                // Add the original message
                final_messages.push(message);
                
                // If this message had tool_use blocks, we need to add a user message with tool_results
                if !tool_use_ids.is_empty() {
                    trace!("Message {} contains {} tool_use blocks, adding tool results", i, tool_use_ids.len());
                    
                    let mut tool_result_blocks = Vec::new();
                    
                    // Log all available tool results for debugging
                    trace!("Available tool results:");
                    for (result_id, content) in &tool_results {
                        trace!("  Tool ID: {}", result_id);
                        trace!("  Content: {}", content);
                    }
                    
                    // For each tool_use block, find its corresponding tool_result
                    for id in &tool_use_ids {
                        // Find matching tool result in collected results
                        if let Some((_, content)) = tool_results.iter().find(|(result_id, _)| result_id == id) {
                            trace!("Creating tool_result block with EXACT tool_use_id: '{}'", id);
                            trace!("Tool result content: {}", content);
                            
                            tool_result_blocks.push(ClaudeContentBlock::ToolResult {
                                content_type: "tool_result".to_string(),
                                tool_use_id: id.clone(),
                                content: content.clone(),
                            });
                        } else {
                            trace!("WARNING: No tool result found for tool_use_id: '{}'", id);
                            trace!("Available tool result IDs: {:?}", 
                                  tool_results.iter().map(|(id, _)| id).collect::<Vec<&String>>());
                        }
                    }
                    
                    // Add a user message with just the tool results immediately after the assistant message
                    if !tool_result_blocks.is_empty() {
                        trace!("Adding user message with {} tool result blocks after message {}", 
                               tool_result_blocks.len(), i);
                               
                        final_messages.push(ClaudeMessage {
                            role: "user".to_string(),
                            content: tool_result_blocks,
                        });
                    }
                }
                
                i += 1;
            }

            // Our new approach handles all cases within the main iteration loop
            // by adding tool results immediately after each message with tool use blocks

            messages = final_messages;

            // Log the final message sequence
            trace!("Restructured messages sequence:");
            for (i, msg) in messages.iter().enumerate() {
                let content_types: Vec<&str> = msg
                    .content
                    .iter()
                    .map(|c| match c {
                        ClaudeContentBlock::Text { .. } => "text",
                        ClaudeContentBlock::ToolUse { .. } => "tool_use",
                        ClaudeContentBlock::ToolResult { .. } => "tool_result",
                    })
                    .collect();
                trace!(
                    "  Message {}: role={}, content_types={:?}",
                    i,
                    msg.role,
                    content_types
                );
            }
        }

        // Log final message structure
        trace!("Final message structure:");
        for (i, msg) in messages.iter().enumerate() {
            let content_types: Vec<&str> = msg
                .content
                .iter()
                .map(|c| match c {
                    ClaudeContentBlock::Text { .. } => "text",
                    ClaudeContentBlock::ToolUse { .. } => "tool_use",
                    ClaudeContentBlock::ToolResult { .. } => "tool_result",
                })
                .collect();
            trace!(
                "  Message {}: role={}, content_types={:?}",
                i,
                msg.role,
                content_types
            );
        }

        // Ensure proper ordering: after each message with tool_use, the next message should start with tool_result
        // This is a final validation step to enforce Claude's API requirements
        let mut has_tool_use = false;
        for (i, msg) in messages.iter().enumerate() {
            let has_tool_use_block = msg
                .content
                .iter()
                .any(|c| matches!(c, ClaudeContentBlock::ToolUse { .. }));

            if has_tool_use_block {
                has_tool_use = true;
            } else if has_tool_use && i > 0 {
                // Check if this message starts with tool_result blocks
                let starts_with_tool_result = matches!(
                    msg.content.first(),
                    Some(ClaudeContentBlock::ToolResult { .. })
                );

                if !starts_with_tool_result {
                    trace!("Warning: Message following tool_use doesn't start with tool_result!");
                }

                // Reset flag after checking
                has_tool_use = false;
            }
        }

        Ok(ClaudeRequest {
            messages,
            system: Some(system_prompt.to_string()),
            max_tokens: self.config.max_tokens,
            temperature: self.current_model_temperature(),
            tools,
            tool_choice: Some(serde_json::json!({ "type": "auto" })),
            anthropic_version: "bedrock-2023-05-31".to_string(),
        })
    }

    /// Parse the conversation history to extract all messages (user, assistant, system, tool) properly formatted
    /// Returns a tuple of (messages, tool_results) where tool_results is a collection of (id, content) pairs
    #[allow(clippy::type_complexity)]
    fn parse_conversation_history(
        &self,
        prompt: &str,
    ) -> Result<(Vec<ClaudeMessage>, Vec<(String, Value)>), String> {
        let mut messages = Vec::new();
        let mut current_role = None;
        let mut in_tag = false;
        let mut tag_lines = Vec::new();
        let mut tool_results = Vec::new();

        // Split the prompt into lines for processing
        let lines: Vec<&str> = prompt.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Check for opening tags
            if line == "<s>" {
                // System message start
                current_role = Some("system");
                in_tag = true;
                tag_lines = Vec::new();
            } else if line == "<user>" {
                // User message start
                current_role = Some("user");
                in_tag = true;
                tag_lines = Vec::new();
            } else if line == "<assistant>" {
                // Assistant message start
                current_role = Some("assistant");
                in_tag = true;
                tag_lines = Vec::new();
            } else if line.starts_with("{\"type\": \"tool_result\"")
                || line.starts_with("{\"type\":\"tool_result\"")
            {
                // Tool result - parse JSON and collect for later processing
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    // Get tool_use_id (or fall back to tool_call_id) and content
                    if let (Some(id), Some(content)) = (
                        json.get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .or_else(|| json.get("tool_call_id").and_then(|v| v.as_str())),
                        json.get("content"),
                    ) {
                        // Parse the content into appropriate format for Claude based on tool type
                        trace!("Processing tool result with id: {}, content: {}", id, content);
                        
                        let parsed_content = if id.contains("read_file") {
                            // For read_file, just pass through the raw content as a single string
                            // No JSON parsing, no line splitting - just the exact file content
                            // IMPORTANT: Claude expects a raw text string for file contents, not a JSON string or array
                            if content.is_string() {
                                let content_str = content.as_str().unwrap_or("");
                                trace!("Read file result, preserving as raw string: {} chars", content_str.len());
                                trace!("Raw content: {}", content_str);
                                // The key fix: Return content as a JSON string but NOT wrapped in quotes or array brackets
                                // Using serde_json::Value::String ensures proper escaping without wrapping in array
                                Value::String(content_str.to_string())
                            } else {
                                // This should not happen with read_file
                                trace!("Warning: read_file result not a string, converting");
                                Value::String(content.to_string())
                            }
                        } else if id.contains("list_directory") || (content.is_string() && 
                                content.as_str().unwrap_or("").contains("Contents of")) {
                            // For directory listings, format as objects with text and type fields
                            let content_str = content.as_str().unwrap_or("");
                            trace!("Directory listing result: {} chars", content_str.len());
                            
                            let entries: Vec<&str> = content_str
                                .lines()
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .collect();

                            // Create an array of structured objects
                            let mut file_objects = Vec::new();

                            for (i, entry) in entries.iter().enumerate() {
                                // Skip the first line if it contains directory path
                                if i == 0 && entry.contains("Contents of") {
                                    continue;
                                }

                                // Parse file/directory entries
                                if let Some(name_end) = entry.rfind(" (") {
                                    let name = entry[..name_end].trim_matches('"');
                                    
                                    // Create structured object with text field and type=text
                                    let mut obj = serde_json::Map::new();
                                    obj.insert(
                                        "text".to_string(),
                                        Value::String(name.to_string()),
                                    );
                                    obj.insert(
                                        "type".to_string(),
                                        Value::String("text".to_string()),
                                    );

                                    file_objects.push(Value::Object(obj));
                                }
                            }

                            // Return the array of file objects
                            Value::Array(file_objects)
                        } else if content.is_string() {
                            // For other tools with string content
                            let content_str = content.as_str().unwrap_or("");
                            trace!("Other tool result: {} chars", content_str.len());
                            
                            // Try parsing as JSON first
                            match serde_json::from_str::<Value>(content_str) {
                                Ok(json_val) => {
                                    // If already JSON, use it
                                    json_val
                                }
                                Err(_) => {
                                    // If not JSON, use as string
                                    Value::String(content_str.to_string())
                                }
                            }
                        } else {
                            // If it's already a complex JSON value, use as is
                            content.clone()
                        };

                        // Store tool result for later use
                        trace!(
                            "Collected tool result with exact tool_use_id '{}' for later processing",
                            id
                        );
                        // Ensure we're storing the exact id string without any modification
                        tool_results.push((id.to_string(), parsed_content));
                    }
                }
            } else if line == "</s>" || line == "</user>" || line == "</assistant>" {
                // Closing tag - finalize current message if we have a role
                if let Some(role) = current_role.take() {
                    if !tag_lines.is_empty() {
                        let content_text = tag_lines.join("\n");

                        // Check for tool calls in assistant messages
                        if role == "assistant" && content_text.contains("<tool name=") {
                            // Process assistant message with potential tool calls
                            let (text_content, tool_calls) =
                                self.extract_tool_calls_from_text(&content_text);

                            // Create content blocks - first text, then tool uses
                            let mut content_blocks = Vec::new();

                            // Add text content if not empty
                            if !text_content.trim().is_empty() {
                                content_blocks.push(ClaudeContentBlock::Text {
                                    content_type: "text".to_string(),
                                    text: text_content,
                                });
                            }

                            // Add any tool call blocks
                            for tool_call in tool_calls {
                                // Check if we have the original ID - critical for response validation
                                let id = if let Some(original_id) = &tool_call.id {
                                    trace!(
                                        "Using original tool_use_id '{}' from conversation history",
                                        original_id
                                    );
                                    original_id.clone()
                                } else {
                                    // If no original ID, generate one, but this may cause validation issues
                                    let generated_id = format!("tool-{}", uuid::Uuid::new_v4());
                                    warn!(
                                        "No original tool_use_id found, generating new ID '{}' which may cause validation failures",
                                        generated_id
                                    );
                                    generated_id
                                };

                                content_blocks.push(ClaudeContentBlock::ToolUse {
                                    content_type: "tool_use".to_string(),
                                    id,
                                    name: tool_call.name,
                                    input: tool_call.args,
                                });
                            }

                            // Add message with all content blocks
                            if !content_blocks.is_empty() {
                                messages.push(ClaudeMessage {
                                    role: role.to_string(),
                                    content: content_blocks,
                                });
                            }
                        } else {
                            // Regular text message
                            let content_blocks = vec![ClaudeContentBlock::Text {
                                content_type: "text".to_string(),
                                text: content_text,
                            }];

                            messages.push(ClaudeMessage {
                                role: role.to_string(),
                                content: content_blocks,
                            });
                        }
                    }

                    in_tag = false;
                    tag_lines = Vec::new();
                }
            } else if in_tag {
                // Inside a tag, collect content
                tag_lines.push(line);
            }

            i += 1;
        }

        // If empty (no messages found), create a default user message
        if messages.is_empty() {
            let content_blocks = vec![ClaudeContentBlock::Text {
                content_type: "text".to_string(),
                text: prompt.to_string(),
            }];

            messages.push(ClaudeMessage {
                role: "user".to_string(),
                content: content_blocks,
            });
        }

        trace!(
            "Parsed conversation history: {} messages, {} tool results",
            messages.len(),
            tool_results.len()
        );

        Ok((messages, tool_results))
    }

    /// Extract tool calls from formatted assistant text
    fn extract_tool_calls_from_text(&self, text: &str) -> (String, Vec<ToolUse>) {
        let mut tool_calls = Vec::new();
        let mut text_content = String::new();

        // Extract tool calls using a simple regex-like approach
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            if line.trim().starts_with("<tool name=") {
                // Extract tool name from the line
                if let Some(name_start) = line.find("name=\"") {
                    if let Some(name_end) = line[name_start + 6..].find("\"") {
                        let name = &line[name_start + 6..name_start + 6 + name_end];

                        // Extract the original tool ID if provided (important for response validation)
                        let original_id = if let Some(id_start) = line.find("id=\"") {
                            line[id_start + 4..]
                                .find("\"")
                                .map(|id_end| line[id_start + 4..id_start + 4 + id_end].to_string())
                        } else {
                            None
                        };

                        // Log if we found or couldn't find an original ID
                        if let Some(id) = &original_id {
                            trace!("Found original tool_use_id '{}' in message text", id);
                        } else {
                            warn!(
                                "No original tool_use_id found in message text, response validation may fail"
                            );
                        }

                        // Extract tool args (everything until </tool>)
                        let mut arg_json = String::new();
                        for arg_line in lines.by_ref() {
                            if arg_line.trim() == "</tool>" {
                                break;
                            }
                            arg_json.push_str(arg_line);
                            arg_json.push('\n');
                        }

                        // Parse JSON arguments if possible
                        match serde_json::from_str::<HashMap<String, Value>>(&arg_json) {
                            Ok(args) => {
                                tool_calls.push(ToolUse {
                                    name: name.to_string(),
                                    args,
                                    id: original_id, // Use original ID if found
                                });
                            }
                            Err(_) => {
                                // If JSON parsing fails, attempt to parse as key=value pairs
                                warn!("Failed to parse tool args as JSON: {}", arg_json);

                                // For legacy support - try extracting key=value pairs
                                let mut args = HashMap::new();
                                for line in arg_json.lines() {
                                    if let Some(equals_idx) = line.find('=') {
                                        let key = line[..equals_idx].trim();
                                        let value = line[equals_idx + 1..].trim();
                                        args.insert(
                                            key.to_string(),
                                            Value::String(value.to_string()),
                                        );
                                    }
                                }

                                if !args.is_empty() {
                                    tool_calls.push(ToolUse {
                                        name: name.to_string(),
                                        args,
                                        id: original_id, // Use original ID if found
                                    });
                                }
                            }
                        }
                    }
                }
            } else {
                // Add this line to text content
                text_content.push_str(line);
                text_content.push('\n');
            }
        }

        (text_content, tool_calls)
    }

    // The extract_tool_results function has been replaced by parse_conversation_history
}

impl BackendCore for BedrockBackend {
    fn name(&self) -> &'static str {
        "AWS Bedrock"
    }

    fn context_window(&self) -> usize {
        self.current_model_token_limit()
    }
}

#[async_trait]
impl Backend for BedrockBackend {
    async fn generate_response(&self, prompt: &str) -> Result<BackendResponse, String> {
        trace!("Generating response with model: {:?}", self.current_model);

        // If client is not initialized, return error
        let client = match &self.client {
            Some(client) => client.clone(),
            None => {
                error!("Bedrock client not initialized");
                return Err("Bedrock client not initialized. Call init() first.".to_string());
            }
        };

        // Construct Claude request
        let request = self.construct_claude_request(prompt)?;

        // Serialize to pretty-printed JSON for logging
        let pretty_request = match self.pretty_print_json(&request) {
            Ok(json) => json,
            Err(e) => {
                error!("{}", e);
                return Err(format!("Failed to serialize request: {}", e));
            }
        };
        debug!("REQUEST JSON:\n{}", pretty_request);

        // Serialize to compact JSON for API call
        let request_json = match serde_json::to_string(&request) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize request: {}", e);
                return Err(format!("Failed to serialize request: {}", e));
            }
        };

        // Set up retry for API calls
        let mut retries = 0;
        let mut last_error = None;

        while retries <= self.config.max_retries {
            if retries > 0 {
                // Exponential backoff
                let backoff_ms = 100 * (2u64.pow(retries as u32));
                warn!(
                    "Retrying API call ({}/{}) after error. Waiting {}ms before retry.",
                    retries, self.config.max_retries, backoff_ms
                );
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            }

            // Call Bedrock API
            trace!(
                "Calling AWS Bedrock API with model: {}",
                self.current_model_name()
            );
            let start_time = std::time::Instant::now();
            let result = client
                .invoke_model()
                .model_id(self.current_model_name())
                .content_type("application/json")
                .accept("application/json")
                .body(Blob::new(request_json.clone().into_bytes()))
                .send()
                .await;
            let elapsed = start_time.elapsed();
            trace!("API call took {:?}", elapsed);

            match result {
                Ok(response) => {
                    // Parse response body
                    let response_body = response.body.clone();
                    let response_str = match String::from_utf8(response_body.as_ref().to_vec()) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to parse response body: {}", e);
                            return Err(format!("Failed to parse response body: {}", e));
                        }
                    };

                    // Parse as JSON value first for pretty printing
                    let json_value = match serde_json::from_str::<serde_json::Value>(&response_str)
                    {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Failed to parse response as JSON: {}", e);
                            return Err(format!("Failed to parse response as JSON: {}", e));
                        }
                    };

                    // Print pretty JSON for logging
                    match self.pretty_print_json(&json_value) {
                        Ok(pretty_json) => debug!("RESPONSE JSON:\n{}", pretty_json),
                        Err(e) => {
                            error!("{}", e);
                            // Still continue processing since we have the original response
                        }
                    };

                    // Deserialize response
                    let claude_response: ClaudeResponse =
                        match serde_json::from_str::<ClaudeResponse>(&response_str) {
                            Ok(r) => r,
                            Err(e) => {
                                error!("Failed to deserialize response: {}", e);
                                return Err(format!("Failed to deserialize response: {}", e));
                            }
                        };

                    // Extract text content and tool calls from JSON
                    let mut content = String::new();
                    let mut tool_calls = Vec::new();

                    // Process each content block from Claude response
                    for block in claude_response.content.iter() {
                        match block.content_type.as_str() {
                            "text" => {
                                if let Some(text) = &block.text {
                                    content.push_str(text);
                                    content.push('\n');
                                }
                            }
                            "tool_use" => {
                                // Extract tool call directly from JSON
                                if let (Some(id), Some(name), Some(input)) =
                                    (&block.id, &block.name, &block.input)
                                {
                                    // Log the exact Claude-provided tool_use ID for tracking
                                    trace!("Received tool_use with ID '{}' from Claude API", id);

                                    tool_calls.push(ToolUse {
                                        name: name.clone(),
                                        args: input.clone(),
                                        id: Some(id.clone()), // Store exactly as received - must not be modified
                                    });
                                }
                            }
                            _ => {
                                // Ignore other content types
                                warn!("Ignoring content block with type: {}", block.content_type);
                            }
                        }
                    }

                    // Add text representation of tool calls for backward compatibility
                    // This will be removed in a future version once transition is complete
                    for tool_call in tool_calls.iter() {
                        let tool_json = self
                            .pretty_print_json(&tool_call.args)
                            .unwrap_or_else(|_| "{}".to_string());

                        // Include the original tool_use_id in the formatted tool call
                        let formatted_tool_call = if let Some(id) = &tool_call.id {
                            trace!(
                                "Including original tool_use_id '{}' in formatted tool call",
                                id
                            );
                            format!(
                                "<tool name=\"{}\" id=\"{}\">\n{}\n</tool>",
                                tool_call.name,
                                id, // Include the exact original ID
                                tool_json
                            )
                        } else {
                            warn!("No ID available for tool call, response validation may fail");
                            format!("<tool name=\"{}\">\n{}\n</tool>", tool_call.name, tool_json)
                        };

                        content.push_str(&formatted_tool_call);
                        content.push('\n');
                    }

                    // Log minimal info about processed results
                    trace!(
                        "Processed {} content blocks with {} tool calls",
                        claude_response.content.len(),
                        tool_calls.len()
                    );

                    // Build response with tool calls directly included
                    return Ok(BackendResponse {
                        content,
                        model: claude_response.model,
                        tokens_used: Some(
                            claude_response.usage.input_tokens
                                + claude_response.usage.output_tokens,
                        ),
                        tool_calls,
                    });
                }
                Err(err) => {
                    let error_msg = self.parse_error(err);
                    error!("API call failed: {}", error_msg);
                    last_error = Some(error_msg);
                    retries += 1;
                }
            }
        }

        // If we get here, all retries failed
        let error_msg =
            last_error.unwrap_or_else(|| "Unknown error calling Bedrock API".to_string());
        error!(
            "Failed to call Bedrock API after {} retries: {}",
            self.config.max_retries, error_msg
        );
        Err(error_msg)
    }
}
