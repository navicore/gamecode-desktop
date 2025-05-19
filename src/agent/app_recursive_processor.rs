use crate::agent::backends::Backend;
use crate::agent::manager::{AgentManager, ToolResult};
use tracing::{error, trace};

/// Configuration for tool chain processing
pub struct ToolChainConfig {
    /// Maximum depth of tool chain (how many sequential tool calls allowed)
    pub max_depth: usize,
    
    /// Delay between API calls in milliseconds (to avoid throttling)
    pub delay_ms: u64,
}

impl Default for ToolChainConfig {
    fn default() -> Self {
        Self {
            max_depth: 5,  // Default max depth of 5: Allows for longer tool chains
            delay_ms: 200, // Default delay of 200ms between API calls
        }
    }
}

/// Process a controlled sequence of tool calls with depth limiting
/// This approach allows LLM-driven multi-step tool chains (e.g., Tool A → Tool B → Tool C)
/// while preventing infinite loops by enforcing a maximum chain depth.
///
/// The function will:
/// 1. Execute all tool calls in the current response
/// 2. Get a follow-up response
/// 3. If that response has tool calls, recurse to process them (up to max_depth)
/// 4. Return the combined results and content from the entire chain
async fn process_tool_chain(
    agent_manager: &mut AgentManager,
    current_response: crate::agent::backends::BackendResponse,
    response_tools: &mut Vec<ToolResult>,
    response_content: &mut String,
    current_depth: usize,
    config: &ToolChainConfig,
) {
    // Safety check against infinite loops
    if current_depth >= config.max_depth {
        trace!(
            "CRITICAL: Reached maximum tool chain depth of {}, stopping recursion",
            config.max_depth
        );
        
        // Log what tools are being dropped due to max depth
        if !current_response.tool_calls.is_empty() {
            let tool_names: Vec<String> = current_response.tool_calls
                .iter()
                .map(|call| call.name.clone())
                .collect();
                
            error!(
                "Max tool chain depth of {} reached - DROPPING {} tool calls: {:?}",
                config.max_depth,
                current_response.tool_calls.len(),
                tool_names
            );
        }
        
        return;
    }

    if current_response.tool_calls.is_empty() {
        trace!("No tool calls to process at depth {}", current_depth);
        return;
    }

    trace!(
        "Depth {}/{}: Processing {} tool calls",
        current_depth,
        config.max_depth,
        current_response.tool_calls.len()
    );

    // Execute each tool call from the current response
    let mut tool_results = Vec::new();

    for tool_call in &current_response.tool_calls {
        trace!("Processing tool call: {}", tool_call.name);

        // Extract tool arguments
        let mut args = Vec::new();
        for (key, value) in &tool_call.args {
            if let Some(value_str) = value.as_str() {
                args.push(format!("{}={}", key, value_str));
            } else {
                args.push(format!("{}={}", key, value.to_string()));
            }
        }

        // Execute the tool
        match agent_manager.tool_registry.execute_tool(&tool_call.name, &args).await {
            Ok(result) => {
                // Create a tool result
                tool_results.push(ToolResult {
                    tool_name: tool_call.name.clone(),
                    result: result.clone(),
                    tool_call_id: tool_call.id.clone(),
                });

                trace!("Executed tool {}", tool_call.name);

                // Add this tool result to the original response's tool results
                // Log the tool result being added to the response
                trace!("Adding tool result to response: {} with ID {:?}", tool_call.name, tool_call.id);
                
                response_tools.push(ToolResult {
                    tool_name: tool_call.name.clone(),
                    result: result.clone(),
                    tool_call_id: tool_call.id.clone(),
                });
            }
            Err(e) => {
                error!("Failed to execute tool {}: {}", tool_call.name, e);
            }
        }
    }

    // If we executed any tools, process them to get one more response
    if !tool_results.is_empty() {
        agent_manager.context_manager.add_tool_results(&tool_results);

        // Get a follow-up response
        let context = agent_manager.context_manager.get_context();
        match agent_manager.backend.generate_response(&context).await {
            Ok(next_response) => {
                trace!(
                    "Depth {}: Got response after tools: {} chars, {} new tool calls",
                    current_depth,
                    next_response.content.len(),
                    next_response.tool_calls.len()
                );

                // Add to the combined response
                *response_content = format!("{}\n\n{}", response_content, next_response.content);

                // Add this response to the context
                agent_manager.context_manager.add_assistant_message(&next_response.content);

                // Recursive processing for any new tool calls (with depth incremented)
                if !next_response.tool_calls.is_empty() {
                    trace!(
                        "Depth {}: Response contains {} new tool calls, recursing to depth {}",
                        current_depth,
                        next_response.tool_calls.len(),
                        current_depth + 1
                    );
                    
                    // Add a delay between API calls to avoid throttling
                    if config.delay_ms > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(config.delay_ms)).await;
                    }
                    
                    // Recurse to process the next level of tool calls - using Box::pin to handle recursive async
                    let next_depth = current_depth + 1;
                    let next_response_clone = next_response.clone();
                    let future = Box::pin(async move {
                        process_tool_chain(
                            agent_manager,
                            next_response_clone,
                            response_tools,
                            response_content,
                            next_depth,
                            config,
                        ).await
                    });
                    future.await;
                }
            }
            Err(e) => {
                error!("Failed to get response after tools: {}", e);
            }
        }
    }
}

/// Entry point function for tool chain processing
/// Starts the process with depth 0 and default configuration
pub async fn process_limited_tool_chain(
    agent_manager: &mut AgentManager,
    current_response: crate::agent::backends::BackendResponse,
    response_tools: &mut Vec<ToolResult>,
    response_content: &mut String,
) {
    let config = ToolChainConfig::default();
    process_tool_chain(
        agent_manager, 
        current_response, 
        response_tools, 
        response_content, 
        0,
        &config,
    ).await;
}

/// Entry point function for tool chain processing with custom configuration
pub async fn process_tool_chain_with_config(
    agent_manager: &mut AgentManager,
    current_response: crate::agent::backends::BackendResponse,
    response_tools: &mut Vec<ToolResult>,
    response_content: &mut String,
    config: ToolChainConfig,
) {
    process_tool_chain(
        agent_manager, 
        current_response, 
        response_tools, 
        response_content, 
        0,
        &config,
    ).await;
}

/// Process a single round of tool calls without recursion
/// This is useful when you want to strictly limit to one round of tool execution
pub async fn process_single_tool_round(
    agent_manager: &mut AgentManager,
    current_response: crate::agent::backends::BackendResponse,
    response_tools: &mut Vec<ToolResult>,
    response_content: &mut String,
) {
    // Use the recursive processor with max_depth=1 to process exactly one round
    let config = ToolChainConfig {
        max_depth: 1,
        delay_ms: 0, // No delay needed for a single round
    };
    
    process_tool_chain(
        agent_manager,
        current_response,
        response_tools,
        response_content,
        0,
        &config,
    ).await;
}