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
            and use tools when appropriate to complete tasks."
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
            self.messages.push(Message {
                role: MessageRole::Tool,
                content: result.result.clone(),
                tool_name: Some(result.tool_name.clone()),
            });
            
            // Estimate token count (very rough estimate)
            self.token_count += result.result.split_whitespace().count();
        }
    }
    
    /// Get the current context as a formatted string
    pub fn get_context(&self) -> String {
        let mut context = String::new();
        
        for message in &self.messages {
            match message.role {
                MessageRole::System => {
                    context.push_str(&format!("<system>\n{}\n</system>\n\n", message.content));
                },
                MessageRole::User => {
                    context.push_str(&format!("<user>\n{}\n</user>\n\n", message.content));
                },
                MessageRole::Assistant => {
                    context.push_str(&format!("<assistant>\n{}\n</assistant>\n\n", message.content));
                },
                MessageRole::Tool => {
                    if let Some(tool_name) = &message.tool_name {
                        context.push_str(&format!("<tool name=\"{}\">\n{}\n</tool>\n\n", tool_name, message.content));
                    }
                },
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
        let system_messages: Vec<Message> = self.messages.iter()
            .filter(|m| m.role == MessageRole::System)
            .cloned()
            .collect();
            
        // Keep the last 4 messages (2 exchanges) - adjust as needed
        let recent_messages: Vec<Message> = self.messages.iter()
            .rev()
            .take(4)
            .cloned()
            .collect();
            
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
        self.token_count = self.messages.iter()
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
