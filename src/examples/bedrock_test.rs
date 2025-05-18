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
            info!("Result ID: {:?}", result.tool_call_id);
            info!("Result content: {}", result.result);
        }
    }
    
    // Get the current context for debugging
    let context = agent_manager.context_manager.get_context();
    info!("Current context with tool results:\n{}", context);
    
    // Now we need to send the tool result back to Bedrock and get Claude's response
    info!("Sending follow-up request to Claude with the tool result...");
    
    // Process a follow-up message to continue the conversation with the tool result
    let response2 = agent_manager.process_input("Please continue with the listing and create the hello.txt file as I requested.").await?;
    
    info!("Second response from Claude after receiving tool result:");
    info!("{}", response2.content);
    
    if !response2.tool_results.is_empty() {
        info!("Tool results from second response:");
        for result in &response2.tool_results {
            info!("Tool: {}", result.tool_name);
            info!("Result ID: {:?}", result.tool_call_id);
            info!("Result content: {}", result.result);
        }
    }
    
    // Check if the file was created
    let hello_file_path = Path::new(&current_dir).join("hello.txt");
    if hello_file_path.exists() {
        // Read the file contents to verify
        match std::fs::read_to_string(&hello_file_path) {
            Ok(content) => {
                info!("Successfully created hello.txt with content: {}", content);
            },
            Err(e) => {
                info!("File exists but couldn't read content: {}", e);
            }
        }
    } else {
        info!("File hello.txt was not created yet");
    }
    
    // Display the final context to see the complete conversation flow
    let final_context = agent_manager.context_manager.get_context();
    info!("Final context with complete conversation flow:\n{}", final_context);

    Ok(())
}
