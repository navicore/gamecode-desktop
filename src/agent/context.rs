use serde_json;
use tracing::trace;

/// Manager for maintaining conversation context
pub struct ContextManager {
    /// Messages in the current conversation
    messages: Vec<Message>,

    /// Current token count estimate
    token_count: usize,
}

/// Structure representing a message in the conversation
pub struct Message {
    /// Role of the message sender (user, assistant, system, tool)
    pub role: MessageRole,

    /// Content of the message
    pub content: String,

    /// Name of the tool if role is tool
    pub tool_name: Option<String>,
}

/// Enum representing the role of a message sender
#[derive(PartialEq, Clone, Copy)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl ContextManager {
    /// Create a new context manager
    pub fn new() -> Self {
        let mut manager = Self {
            messages: Vec::new(),
            token_count: 0,
        };

        // Add default system message
        manager.add_system_message(
            "You are a helpful assistant with access to tools that can run on the user's computer. \
            Respond to the user's queries directly when possible, \
            and use tools when appropriate to complete tasks.",
        );

        manager
    }

    /// Add a system message to the context
    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::System,
            content: content.to_string(),
            tool_name: None,
        });

        // Estimate token count (very rough estimate)
        self.token_count += content.split_whitespace().count();
    }

    /// Add a user message to the context
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::User,
            content: content.to_string(),
            tool_name: None,
        });

        // Estimate token count (very rough estimate)
        self.token_count += content.split_whitespace().count();
    }

    /// Add an assistant message to the context
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::Assistant,
            content: content.to_string(),
            tool_name: None,
        });

        // Estimate token count (very rough estimate)
        self.token_count += content.split_whitespace().count();
    }

    /// Add tool results to the context
    pub fn add_tool_results(&mut self, tool_results: &[crate::agent::manager::ToolResult]) {
        trace!("Adding {} tool results to context", tool_results.len());
        
        // First, find if the last message contains tool_use blocks
        let last_message_has_tool_use = self.messages.last()
            .map(|m| m.role == MessageRole::Assistant && m.content.contains("<tool name="))
            .unwrap_or(false);
            
        // If the last message has tool use, we need to insert the tool results as a separate user message
        // This follows Claude's expectation that tool_result blocks appear at the beginning
        // of the user message immediately following a message with tool_use blocks
        if last_message_has_tool_use && !tool_results.is_empty() {
            trace!("Last message contains tool_use blocks, creating a special user message for tool results");
            
            // Create a new user message that will ONLY contain tool results
            let mut tool_result_contents = Vec::new();
            
            // Process each tool result to format it properly
            for result in tool_results {
                // Format tool result in jsonrpc format that Claude expects
                // For Claude integration, tool results must provide the tool_call_id
                if let Some(id) = &result.tool_call_id {
                    // Log full details about the tool result
                    trace!("============================================================");
                    trace!("Processing tool result for inclusion in next message:");
                    trace!("Tool: {}", result.tool_name);
                    trace!("Tool ID: {}", id);
                    trace!("Result content: {}", result.result);
                    trace!("============================================================");
                    let result_content = if result.tool_name == "list_directory" {
                        // Format directory listing as structured objects with text fields
                        // This is the format Claude expects: objects with text and type keys
                        let entries: Vec<&str> = result
                            .result
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
                                
                                // Create structured object with text field and type always set to "text"
                                let entry_obj = serde_json::json!({
                                    "text": name,
                                    "type": "text"
                                });

                                file_objects.push(entry_obj);
                            }
                        }

                        // Return the array of file objects as a string
                        serde_json::to_string(&file_objects).unwrap_or_else(|_| {
                            format!("[{{\"error\": \"Failed to format directory entries\"}}]")
                        })
                    } else if result.tool_name == "read_file" {
                        // CRITICAL: Return the raw file content as a single string - no JSON serialization
                        // Just the plain text content exactly as is - Claude expects this specific format
                        trace!("Formatting read_file result as raw string, NOT JSON array");
                        trace!("Content length: {} chars", result.result.len());
                        // The tool_result content field for read_file should be a plain string, NOT a JSON array
                        // Return exactly what we got from the tool without any additional processing
                        result.result.clone()
                    } else {
                        // For other tools, try to parse as JSON first
                        match serde_json::from_str::<serde_json::Value>(&result.result) {
                            Ok(json_val) => {
                                // If it's already an array, use it as is
                                if json_val.is_array() {
                                    serde_json::to_string(&json_val)
                                } else {
                                    // If it's already a proper JSON object, wrap it in an array
                                    let array = vec![json_val];
                                    serde_json::to_string(&array)
                                }
                            }
                            Err(_) => {
                                // If it's not JSON, create a simple array with one item
                                let content_lines: Vec<&str> = result
                                    .result
                                    .lines()
                                    .map(|s| s.trim())
                                    .filter(|s| !s.is_empty())
                                    .collect();

                                // If multiple lines, create an array of lines
                                if content_lines.len() > 1 {
                                    let simple_array: Vec<String> =
                                        content_lines.into_iter().map(|s| s.to_string()).collect();
                                    serde_json::to_string(&simple_array)
                                } else {
                                    // Single item with the content
                                    serde_json::to_string(&vec![result.result.clone()])
                                }
                            }
                        }
                        .unwrap_or_else(|_| {
                            format!(
                                "[\"{}\"]]",
                                result.result.replace("\"", "\\\"").replace("\n", "\\n")
                            )
                        })
                    };

                    trace!(
                        "Formatting tool result with exact tool_use_id '{}' in expected format",
                        id
                    );

                    // Format as pure JSON-RPC - CRITICAL: Use exactly the same tool_use_id
                    let content = if result.tool_name == "read_file" {
                        trace!("CRITICAL: Formatting read_file result with special handling");
                        // For read_file, the content must be a JSON string, not an array
                        // Quote and escape the content string properly for JSON
                        let escaped_content = serde_json::to_string(&result.result).unwrap_or_default();
                        
                        // Log details about the transformation
                        trace!("Original read_file content length: {}", result.result.len());
                        trace!("Escaped JSON string format: {}", escaped_content);
                        trace!("First 100 chars of escaped format: {}", if escaped_content.len() > 100 {
                            &escaped_content[..100]
                        } else {
                            &escaped_content
                        });
                        
                        format!(
                            "{{\"type\": \"tool_result\", \"tool_use_id\": \"{}\", \"content\": {}}}",
                            id, escaped_content
                        )
                    } else {
                        format!(
                            "{{\"type\": \"tool_result\", \"tool_use_id\": \"{}\", \"content\": {}}}",
                            id, result_content
                        )
                    };
                    
                    // Add this content to our tool results collection
                    tool_result_contents.push(content);
                    
                    // Estimate token count
                    self.token_count += result.result.split_whitespace().count();
                } else {
                    trace!("Tool result missing tool_call_id, skipping");
                }
            }
            
            // If we have tool results, create a special message with ONLY tool results
            if !tool_result_contents.is_empty() {
                // Join all tool results together
                let combined_content = tool_result_contents.join("\n");
                
                // Create a user message with tool results
                let tool_result_message = Message {
                    role: MessageRole::User,
                    content: combined_content,
                    tool_name: None,
                };
                
                // Add this message to the context
                trace!("Adding user message with {} tool results", tool_result_contents.len());
                self.messages.push(tool_result_message);
            }
        } else {
            // Legacy approach - add tool results as Tool messages
            trace!("Adding tool results as individual Tool messages");
            for result in tool_results {
                if let Some(id) = &result.tool_call_id {
                    // Log full details about the tool result
                    trace!("============================================================");
                    trace!("Processing tool result for inclusion in next message:");
                    trace!("Tool: {}", result.tool_name);
                    trace!("Tool ID: {}", id);
                    trace!("Result content: {}", result.result);
                    trace!("============================================================");
                    let result_content = if result.tool_name == "list_directory" {
                        // Format directory listing as structured objects with text fields
                        // This is the format Claude expects: objects with text and type keys
                        let entries: Vec<&str> = result
                            .result
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
                                
                                // Create structured object with text field and type always set to "text"
                                let entry_obj = serde_json::json!({
                                    "text": name,
                                    "type": "text"
                                });

                                file_objects.push(entry_obj);
                            }
                        }

                        // Return the array of file objects as a string
                        serde_json::to_string(&file_objects).unwrap_or_else(|_| {
                            format!("[{{\"error\": \"Failed to format directory entries\"}}]")
                        })
                    } else if result.tool_name == "read_file" {
                        // CRITICAL: Return the raw file content as a single string - no JSON serialization
                        // Just the plain text content exactly as is - Claude expects this specific format
                        trace!("Formatting read_file result as raw string, NOT JSON array");
                        trace!("Content length: {} chars", result.result.len());
                        // The tool_result content field for read_file should be a plain string, NOT a JSON array
                        // Return exactly what we got from the tool without any additional processing
                        result.result.clone()
                    } else {
                        // For other tools, format appropriately
                        match serde_json::from_str::<serde_json::Value>(&result.result) {
                            Ok(json_val) => {
                                if json_val.is_array() {
                                    serde_json::to_string(&json_val)
                                } else {
                                    let array = vec![json_val];
                                    serde_json::to_string(&array)
                                }
                            }
                            Err(_) => {
                                let content_lines: Vec<&str> = result
                                    .result
                                    .lines()
                                    .map(|s| s.trim())
                                    .filter(|s| !s.is_empty())
                                    .collect();

                                if content_lines.len() > 1 {
                                    let simple_array: Vec<String> =
                                        content_lines.into_iter().map(|s| s.to_string()).collect();
                                    serde_json::to_string(&simple_array)
                                } else {
                                    serde_json::to_string(&vec![result.result.clone()])
                                }
                            }
                        }
                        .unwrap_or_else(|_| {
                            format!(
                                "[\"{}\"]]",
                                result.result.replace("\"", "\\\"").replace("\n", "\\n")
                            )
                        })
                    };

                    let content = if result.tool_name == "read_file" {
                        trace!("CRITICAL: Formatting read_file result with special handling");
                        // For read_file, the content must be a JSON string, not an array or broken into lines
                        // Quote and escape the content string properly for JSON
                        let escaped_content = serde_json::to_string(&result.result).unwrap_or_default();
                        
                        // Log details about the transformation
                        trace!("Original read_file content length: {}", result.result.len());
                        trace!("Escaped JSON string format: {}", escaped_content);
                        trace!("First 100 chars of escaped format: {}", if escaped_content.len() > 100 {
                            &escaped_content[..100]
                        } else {
                            &escaped_content
                        });
                        
                        format!(
                            "{{\"type\": \"tool_result\", \"tool_use_id\": \"{}\", \"content\": {}}}",
                            id, escaped_content
                        )
                    } else {
                        format!(
                            "{{\"type\": \"tool_result\", \"tool_use_id\": \"{}\", \"content\": {}}}",
                            id, result_content
                        )
                    };

                    self.messages.push(Message {
                        role: MessageRole::Tool,
                        content,
                        tool_name: Some(result.tool_name.clone()),
                    });

                    self.token_count += result.result.split_whitespace().count();
                } else {
                    trace!("Tool result missing tool_call_id, skipping");
                }
            }
        }
    }

    /// Get the current context as a formatted string
    pub fn get_context(&self) -> String {
        let mut context = String::new();

        for message in &self.messages {
            match message.role {
                MessageRole::System => {
                    context.push_str(&format!("<system>\n{}\n</system>\n\n", message.content));
                }
                MessageRole::User => {
                    // Special case for user messages containing tool results
                    if message.content.starts_with("{\"type\": \"tool_result\"") || 
                       message.content.starts_with("{\"type\":\"tool_result\"") {
                        // Tool results should be included directly without <user> tags
                        // This is critical for Claude's API to recognize the proper format
                        context.push_str(&format!("{}\n\n", message.content));
                        trace!("Including tool result user message directly without tags");
                    } else {
                        // Normal user message
                        context.push_str(&format!("<user>\n{}\n</user>\n\n", message.content));
                    }
                }
                MessageRole::Assistant => {
                    context.push_str(&format!(
                        "<assistant>\n{}\n</assistant>\n\n",
                        message.content
                    ));
                }
                MessageRole::Tool => {
                    // Direct inclusion of tool results in jsonrpc format expected by Claude
                    context.push_str(&format!("{}\n\n", message.content));
                }
            }
        }

        context
    }

    /// Get the current context length (rough token estimate)
    pub fn context_length(&self) -> usize {
        self.token_count
    }

    /// Replace older messages with a summary
    pub fn replace_with_summary(&mut self, summary: &str) {
        // Keep the system message and last few exchanges
        let system_messages: Vec<Message> = self
            .messages
            .iter()
            .filter(|m| m.role == MessageRole::System)
            .cloned()
            .collect();

        // Keep the last 4 messages (2 exchanges) - adjust as needed
        let recent_messages: Vec<Message> = self.messages.iter().rev().take(4).cloned().collect();

        // Create a new summary message
        let summary_message = Message {
            role: MessageRole::System,
            content: format!("Summary of previous conversation:\n{}\n", summary),
            tool_name: None,
        };

        // Reset messages with system + summary + recent
        self.messages = system_messages;
        self.messages.push(summary_message);
        self.messages.extend(recent_messages.into_iter().rev());

        // Recalculate token count
        self.token_count = self
            .messages
            .iter()
            .map(|m| m.content.split_whitespace().count())
            .sum();
    }
}

impl Clone for Message {
    fn clone(&self) -> Self {
        Self {
            role: self.role,
            content: self.content.clone(),
            tool_name: self.tool_name.clone(),
        }
    }
}
