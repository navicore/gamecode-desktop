use async_trait::async_trait;

/// Trait defining a tool that can be executed by the agent
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool's name
    fn name(&self) -> &'static str;
    
    /// Get the tool's description
    fn description(&self) -> &'static str;
    
    /// Get the tool's required arguments
    fn required_args(&self) -> Vec<ToolArgument>;
    
    /// Validate that the provided arguments are correct
    fn validate_args(&self, args: &[String]) -> Result<(), String> {
        let required = self.required_args();
        
        // Check if we have at least the required number of arguments
        if args.len() < required.iter().filter(|arg| arg.required).count() {
            return Err("Not enough arguments provided".to_string());
        }
        
        // TODO: Add more sophisticated validation based on argument types
        
        Ok(())
    }
    
    /// Execute the tool with the given arguments
    async fn execute(&self, args: &[String], working_dir: &str) -> Result<String, String>;
    
    /// Get visualization details for this tool
    fn visualization_type(&self) -> &'static str {
        "default"
    }
}

/// Structure describing a tool argument
pub struct ToolArgument {
    /// Name of the argument
    pub name: String,
    
    /// Description of the argument
    pub description: String,
    
    /// Whether the argument is required
    pub required: bool,
    
    /// Type of the argument
    pub arg_type: ToolArgumentType,
}

/// Enum describing the type of a tool argument
pub enum ToolArgumentType {
    /// String argument
    String,
    
    /// Integer argument
    Integer,
    
    /// Float argument
    Float,
    
    /// Boolean argument
    Boolean,
    
    /// File path argument
    FilePath,
    
    /// Directory path argument
    DirectoryPath,
}

/// An example implementation of a simple tool
pub struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }
    
    fn description(&self) -> &'static str {
        "Echoes back the input text"
    }
    
    fn required_args(&self) -> Vec<ToolArgument> {
        vec![ToolArgument {
            name: "text".to_string(),
            description: "The text to echo back".to_string(),
            required: true,
            arg_type: ToolArgumentType::String,
        }]
    }
    
    async fn execute(&self, args: &[String], _working_dir: &str) -> Result<String, String> {
        if args.is_empty() {
            return Err("No text provided to echo".to_string());
        }
        
        Ok(args.join(" "))
    }
    
    fn visualization_type(&self) -> &'static str {
        "echo"
    }
}
