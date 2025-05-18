use serde_json;
use tracing::debug;

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
        for result in tool_results {
            // Format tool result in jsonrpc format that Claude expects
            // For Claude integration, tool results must provide the tool_call_id
            if let Some(id) = &result.tool_call_id {
                // Format the content properly based on tool type
                let result_content = if result.tool_name == "list_directory" {
                    // Format directory listing as structured objects, not just an array of strings
                    let entries: Vec<&str> = result
                        .result
                        .lines()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();

                    // Create an array of structured objects (avoiding reserved "type" field)
                    let mut file_objects = Vec::new();

                    for (i, entry) in entries.iter().enumerate() {
                        // Skip the first line if it contains directory path
                        if i == 0 && entry.contains("Contents of") {
                            continue;
                        }

                        // Parse file/directory entries
                        if let Some(name_end) = entry.rfind(" (") {
                            let name = entry[..name_end].trim_matches('"');
                            let type_str = entry[name_end + 2..].trim_end_matches(')').trim();

                            // Use is_directory boolean instead of "type" field
                            let is_directory = type_str == "dir" || type_str == "directory";

                            // Create structured object with text field instead of name, keeping other fields
                            let entry_obj = serde_json::json!({
                                "text": name,
                                //"is_directory": is_directory,
                                "type": "text"
                            });

                            file_objects.push(entry_obj);
                        }
                    }

                    // Return the array of file objects as a string
                    serde_json::to_string(&file_objects).unwrap_or_else(|_| {
                        format!("[{{\"error\": \"Failed to format directory entries\"}}]")
                    })
                } else {
                    // For other tools, try first to parse as JSON to see if it's already a valid array
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

                debug!(
                    "Formatting tool result with exact tool_use_id '{}' in pure JSON-RPC format to ensure it matches the original request",
                    id
                );

                // Format as pure JSON-RPC with no additional text
                // This is critical for Claude's API compliance
                // Use the exact ID from the tool call to ensure it matches the original tool_use request
                // DO NOT modify or generate a new ID - it must be exactly the same as received from Claude
                debug!("Using exact tool_use_id: '{}' from tool call", id);
                // CRITICAL: Claude expects "tool_use_id", not "tool_call_id"
                // Even though we're storing it as tool_call_id internally, we need to send it as tool_use_id
                let content = format!(
                    "{{\"type\": \"tool_result\", \"tool_use_id\": \"{}\", \"content\": {}}}",
                    id, result_content
                );

                self.messages.push(Message {
                    role: MessageRole::Tool,
                    content,
                    tool_name: Some(result.tool_name.clone()),
                });

                // Estimate token count (very rough estimate)
                self.token_count += result.result.split_whitespace().count();
            } else {
                debug!("Tool result missing tool_call_id, skipping");
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
                    context.push_str(&format!("<user>\n{}\n</user>\n\n", message.content));
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
