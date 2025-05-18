use crate::agent::tools::executor::ToolExecutor;
use crate::agent::tools::types::Tool;
use std::collections::HashMap;

/// Registry for managing available tools
pub struct ToolRegistry {
    /// Map of tool names to their implementations
    tools: HashMap<String, Box<dyn Tool>>,

    /// Tool execution environment
    executor: ToolExecutor,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            executor: ToolExecutor::new(),
        }
    }

    /// Register a new tool
    pub fn register_tool(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Set the working directory for tool execution
    pub fn set_working_directory(&mut self, directory: &str) {
        self.executor.set_working_directory(directory);
    }

    /// Get a list of all available tool names
    pub fn available_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get information about all registered tools
    pub fn tool_descriptions(&self) -> Vec<(String, String)> {
        self.tools
            .iter()
            .map(|(name, tool)| (name.clone(), tool.description().to_string()))
            .collect()
    }

    /// Execute a tool by name with the given arguments
    pub async fn execute_tool(&self, name: &str, args: &[String]) -> Result<String, String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| format!("Tool '{}' not found", name))?;

        // Validate arguments
        tool.validate_args(args)
            .map_err(|e| format!("Invalid arguments for tool '{}': {}", name, e))?;

        // Execute the tool
        self.executor.execute(tool.as_ref(), args).await
    }
}
