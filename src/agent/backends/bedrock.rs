use crate::agent::backends::{Backend, BackendCore, BackendResponse};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::{error::SdkError, operation::invoke_model::InvokeModelError, Client};
use aws_smithy_types::Blob;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use crate::agent::tools::ExecuteCommandTool;

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

    /// Anthropic API version
    anthropic_version: String,
}

/// Message structure for Claude API
#[derive(Serialize, Debug)]
struct ClaudeMessage {
    /// Role (user or assistant)
    role: String,

    /// Content blocks for the message
    content: Vec<ClaudeContentBlock>,
}

/// Content block for Claude API
#[derive(Serialize, Debug)]
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
        tool_use_id: String,
        content: String,
    },
}

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
    id: String,

    /// Content blocks
    content: Vec<ClaudeResponseContent>,

    /// Model used
    model: String,

    /// Usage information
    usage: ClaudeUsage,
}

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

        debug!("AWS Bedrock client initialized successfully");
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
                let msg = match err {
                    InvokeModelError::AccessDeniedException(e) => {
                        format!("Access denied: {}", e.to_string())
                    }
                    InvokeModelError::InternalServerException(e) => {
                        format!("Internal server error: {}", e.to_string())
                    }
                    InvokeModelError::ModelNotReadyException(e) => {
                        format!("Model not ready: {}", e.to_string())
                    }
                    InvokeModelError::ModelTimeoutException(e) => {
                        format!("Model timeout: {}", e.to_string())
                    }
                    InvokeModelError::ResourceNotFoundException(e) => {
                        format!("Resource not found: {}", e.to_string())
                    }
                    InvokeModelError::ServiceQuotaExceededException(e) => {
                        format!("Service quota exceeded: {}", e.to_string())
                    }
                    InvokeModelError::ThrottlingException(e) => {
                        format!("Throttling error: {}", e.to_string())
                    }
                    InvokeModelError::ValidationException(e) => {
                        format!("Validation error: {}", e.to_string())
                    }
                    _ => format!("Unknown service error: {:?}", err),
                };
                msg
            }
            SdkError::ConstructionFailure(err) => format!("Construction failure: {:?}", err),
            SdkError::DispatchFailure(err) => format!("Dispatch failure: {:?}", err),
            SdkError::ResponseError(err) => format!("Response error: {:?}", err),
            SdkError::TimeoutError(err) => format!("Timeout error: {:?}", err),
            _ => format!("Unknown error: {:?}", err),
        }
    }

    /// Construct a Claude API request from a prompt
    fn construct_claude_request(&self, prompt: &str) -> Result<ClaudeRequest, String> {
        // Parse the prompt to extract system, user, and assistant messages
        // In this implementation, we just use the formatted context from the ContextManager
        // which already has the messages properly formatted with <user>, <assistant>, etc. tags

        // Convert a simple text prompt to a Claude message
        let message = ClaudeMessage {
            role: "user".to_string(),
            content: vec![ClaudeContentBlock::Text {
                content_type: "text".to_string(),
                text: prompt.to_string(),
            }],
        };

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
                let description = format!("Execute a shell command (limited to safe commands: {})", allowed_cmd_list);
                
                // Create the schema with dynamic command description
                let mut schema_properties = serde_json::Map::new();
                let mut command_property = serde_json::Map::new();
                
                command_property.insert(
                    "type".to_string(), 
                    serde_json::Value::String("string".to_string())
                );
                
                command_property.insert(
                    "description".to_string(),
                    serde_json::Value::String(format!(
                        "Command to execute with arguments. Only these commands are allowed: {}", 
                        allowed_cmd_list
                    ))
                );
                
                schema_properties.insert("command".to_string(), serde_json::Value::Object(command_property));
                
                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), serde_json::Value::String("object".to_string()));
                schema.insert("properties".to_string(), serde_json::Value::Object(schema_properties));
                schema.insert(
                    "required".to_string(), 
                    serde_json::Value::Array(vec![serde_json::Value::String("command".to_string())])
                );
                
                ClaudeTool {
                    name: "execute_command".to_string(),
                    description,
                    input_schema: serde_json::Value::Object(schema),
                }
            },
        ]);

        // Create a more appropriate system prompt with security considerations
        let system_prompt = "You are a helpful AI assistant who has access to the user's computer through tools. \
        When using tools, prefer relative paths rather than absolute paths for security. \
        Whenever possible, use the current working directory rather than specifying absolute paths. \
        Answer questions and help with tasks efficiently and securely.";
        
        Ok(ClaudeRequest {
            messages: vec![message],
            system: Some(system_prompt.to_string()),
            max_tokens: self.config.max_tokens,
            temperature: self.current_model_temperature(),
            tools,
            tool_choice: Some(serde_json::json!({ "type": "auto" })),
            anthropic_version: "bedrock-2023-05-31".to_string(),
        })
    }
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
        debug!("Generating response with model: {:?}", self.current_model);

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
        info!("REQUEST JSON:\n{}", pretty_request);

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
            debug!(
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
            debug!("API call took {:?}", elapsed);

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
                        Ok(pretty_json) => info!("RESPONSE JSON:\n{}", pretty_json),
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
                                if let (Some(_id), Some(name), Some(input)) =
                                    (&block.id, &block.name, &block.input)
                                {
                                    tool_calls.push(ToolUse {
                                        name: name.clone(),
                                        args: input.clone(),
                                    });
                                }
                            }
                            _ => {
                                // Ignore other content types
                                debug!("Ignoring content block with type: {}", block.content_type);
                            }
                        }
                    }

                    // Add text representation of tool calls for backward compatibility
                    // This will be removed in a future version once transition is complete
                    for tool_call in tool_calls.iter() {
                        let tool_json = self
                            .pretty_print_json(&tool_call.args)
                            .unwrap_or_else(|_| "{}".to_string());

                        let formatted_tool_call =
                            format!("<tool name=\"{}\">\n{}\n</tool>", tool_call.name, tool_json);

                        content.push_str(&formatted_tool_call);
                        content.push('\n');
                    }

                    // Log minimal info about processed results
                    debug!(
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
