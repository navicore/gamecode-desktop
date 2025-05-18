use crate::agent::backends::BedrockConfig;
use crate::agent::manager::{AgentConfig, AgentManager};
use crate::agent::tools::{ExecuteCommandTool, ListDirectoryTool, ReadFileTool, WriteFileTool};
use std::env;
use std::path::Path;
use tracing::info;

/// Example showing AWS Bedrock integration with Claude models
pub async fn run_bedrock_example() -> Result<(), String> {
    // Initialize tracing with a more verbose configuration
    tracing_subscriber::fmt()
        .with_env_filter("info,gamecode=debug")
        .with_target(true)
        .init();

    info!("Starting Bedrock integration example");

    // Create agent configuration
    let agent_config = AgentConfig {
        use_fast_model_for_context: true,
        max_context_length: 32000,
        auto_compress_context: true,
        aws_region: "us-east-1".to_string(),
        aws_profile: Some("default".to_string()), // Make sure this profile exists in your ~/.aws/credentials
    };

    // Create and initialize agent manager
    let mut agent_manager = AgentManager::with_config(agent_config);

    // Register tools
    agent_manager.register_tool(Box::new(ReadFileTool));
    agent_manager.register_tool(Box::new(WriteFileTool));
    agent_manager.register_tool(Box::new(ListDirectoryTool));
    agent_manager.register_tool(Box::new(ExecuteCommandTool));

    info!("Initializing agent manager");
    agent_manager.init().await?;

    // Get current directory for context
    let current_dir = env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::from("/"));

    info!("Current working directory: {}", current_dir);

    // Set the working directory for the tool executor
    agent_manager.set_working_directory(&current_dir);

    // Process a simple input
    let input = "Hello! Can you list the files in the current directory? The best way is to use the list_directory tool \
        with no parameters to list the current directory. \
        After that, can you create a simple test file called 'hello.txt' with the content 'Hello from AWS Bedrock and Claude!'?";

    info!("Sending input: {}", input);

    let response = agent_manager.process_input(&input).await?;

    info!("Response from Claude:");
    info!("{}", response.content);

    if !response.tool_results.is_empty() {
        info!("Tool results:");
        for result in &response.tool_results {
            info!("Tool: {}", result.tool_name);
            info!("Result: {}", result.result);
        }
    }

    // Check if the file was created
    let hello_file_path = Path::new(&current_dir).join("hello.txt");
    if hello_file_path.exists() {
        info!("Successfully created hello.txt!");
    }

    Ok(())
}
