use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::egui::{Align, Frame, Layout};

use crate::agent;
use crate::agent::backends::Backend; // Import the Backend trait
use crate::agent::app_recursive_processor::{
    process_single_tool_round, 
    process_limited_tool_chain, 
    process_tool_chain_with_config,
    ToolChainConfig
};
use crate::agent::manager::{AgentConfig, AgentManager, AgentResponse};
use crate::core;
use crate::ui;
use crate::visualization::{self, ToolStatus, VisualizationPlugin, VisualizationState};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, trace};

// Define resources for our application
#[derive(Resource)]
pub struct AppState {
    // Input and journal state
    input_text: String,
    journal_messages: Vec<JournalMessage>,
    tool_id_counter: usize,

    // UI state
    show_settings: bool,
    dark_mode: bool,

    // Agent state
    agent_manager: Option<Arc<Mutex<AgentManager>>>,
    agent_initialized: bool,
    processing_input: bool,
}

// A message in the journal with styling information
pub struct JournalMessage {
    content: String,
    sender: MessageSender,
    timestamp: f64,
}

// Who sent the message
pub enum MessageSender {
    User,
    Assistant,
    System,
    Tool(String), // Tool type
}

pub fn run() {
    // Initialize core systems
    core::init();

    // Initialize UI components
    ui::init();

    // Initialize visualization
    visualization::init();

    // Initialize agent and tools
    agent::init();

    // Create Bevy app
    App::new()
        // Add default Bevy plugins without the LogPlugin
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::log::LogPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "GameCode - AI Agent Visualization".to_string(),
                        resolution: (1280.0, 960.0).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        // Add egui for UI components
        // In Bevy 0.15
        .add_plugins(bevy_egui::EguiPlugin)
        // Add visualization plugin
        .add_plugins(VisualizationPlugin)
        // Add app resources
        .init_resource::<AppState>()
        .init_resource::<AgentTask>()
        // Add our systems
        .add_systems(Startup, setup_system)
        // In Bevy 0.15, we need to chain system configurations
        .add_systems(Update, ui_system)
        .add_systems(Update, initialize_agent_system) // Initialize the agent on startup
        .add_systems(Update, poll_agent_task) // Poll agent tasks
        .add_systems(Update, update_camera_viewport) // Update camera viewport to match UI layout
        .run();
}

// Initialize resources
impl Default for AppState {
    fn default() -> Self {
        Self {
            input_text: String::new(),
            journal_messages: vec![
                JournalMessage {
                    content: "Welcome to GameCode!".to_string(),
                    sender: MessageSender::System,
                    timestamp: 0.0,
                },
                JournalMessage {
                    content: "Type in the input box below to interact with the AI agent."
                        .to_string(),
                    sender: MessageSender::System,
                    timestamp: 0.0,
                },
            ],
            tool_id_counter: 0,
            show_settings: false,
            dark_mode: true,
            agent_manager: None,
            agent_initialized: false,
            processing_input: false,
        }
    }
}

// Setup function runs once at startup
fn setup_system(mut commands: Commands, windows: Query<&Window>) {
    // Get window dimensions
    let window = windows.single();
    let window_width = window.resolution.width();
    let window_height = window.resolution.height();

    // Calculate visualization area dimensions (25% of window height)
    let vis_height = window_height * 0.25;

    // CRITICAL INSIGHT: The camera needs to be positioned to look at the center of the top 25% section
    // not at the center of the entire window

    // Calculate viewport position for the top 25% of the window
    let width = window_width as u32;
    let height = vis_height as u32;
    let window_physical_height = window_height as u32;
    let viewport_bottom_left_y = window_physical_height - height;

    // Calculate the position offset needed to center the camera in the top 25% section
    // The amount to shift up from the center of the window to the center of the top section
    // This is: (half window height - half visualization height) = 3/8 of window height
    let y_offset = (window_height * 0.5) - (vis_height * 0.5);

    // In Bevy 0.15, we use Camera2d marker component which auto-inserts required components
    commands.spawn(Camera2d);
}

// Generate a unique tool ID
fn generate_tool_id(app_state: &mut AppState) -> String {
    let id = format!("tool_{}", app_state.tool_id_counter);
    app_state.tool_id_counter += 1;
    id
}

// Tracking struct for demo tools
#[derive(Clone)]
struct DemoTool {
    id: String,
    tool_type: String,
    start_time: f64,
    completed: bool,
}

// System to initialize the agent
fn initialize_agent_system(mut app_state: ResMut<AppState>) {
    // Only initialize once
    if app_state.agent_initialized {
        return;
    }

    // Create agent manager if it doesn't exist
    if app_state.agent_manager.is_none() {
        // Create config
        let config = AgentConfig {
            use_fast_model_for_context: true,
            max_context_length: 32000,
            auto_compress_context: true,
            aws_region: "us-west-2".to_string(),
            aws_profile: None,
        };

        // Create agent manager with config
        let agent_manager = AgentManager::with_config(config);
        app_state.agent_manager = Some(Arc::new(Mutex::new(agent_manager)));

        // Add a system message to the journal
        app_state.journal_messages.push(JournalMessage {
            content: "AI Assistant initialized and ready".to_string(),
            sender: MessageSender::System,
            timestamp: 0.0,
        });
    }

    // Mark as initialized - we will do the actual backend initialization in the
    // first message since it's async and needs to be handled in a task
    app_state.agent_initialized = true;
}

// Process agent response and update UI
fn process_agent_response(
    commands: &mut Commands,
    app_state: &mut AppState,
    vis_state: &mut VisualizationState,
    tool_query: &mut Query<(&mut visualization::ToolEntity, &mut Sprite)>,
    response: AgentResponse,
    current_time: f64,
) {
    // Add assistant response to journal
    if !response.content.is_empty() {
        app_state.journal_messages.push(JournalMessage {
            content: response.content,
            sender: MessageSender::Assistant,
            timestamp: current_time,
        });
    }

    // Process any tool results
    for tool_result in &response.tool_results {
        // Generate a tool ID if needed
        let tool_id = generate_tool_id(app_state);

        // Map tool name to tool type for visualization (simple mapping for now)
        let tool_type = match tool_result.tool_name.as_str() {
            "read_file" => "file",
            "write_file" => "file",
            "list_directory" => "file",
            "execute_command" => "process",
            _ => "process", // Default
        };

        // Start a new tool visualization
        visualization::start_tool_visualization(commands, vis_state, &tool_id, tool_type);

        // Update the status to running
        visualization::update_tool_status_public(
            commands,
            vis_state,
            &tool_id,
            ToolStatus::Running,
            tool_query,
        );

        // Add a journal message for the tool
        app_state.journal_messages.push(JournalMessage {
            content: format!("Started {} tool (ID: {})", tool_type, tool_id),
            sender: MessageSender::Tool(tool_type.to_string()),
            timestamp: current_time,
        });

        // Don't show the raw tool result in the journal, as it will be processed
        // and displayed in a more user-friendly way in the LLM's follow-up response

        // Instead, add a hidden system message to track the tool execution
        // Adding a special marker that can be filtered out in the journal display
        app_state.journal_messages.push(JournalMessage {
            content: format!("<!-- TOOL_TRACKER: {} -->", tool_id),
            sender: MessageSender::System,
            timestamp: current_time,
        });

        // Update the status to completed
        visualization::update_tool_status_public(
            commands,
            vis_state,
            &tool_id,
            ToolStatus::Completed,
            tool_query,
        );

        // Add a journal message for the completion
        app_state.journal_messages.push(JournalMessage {
            content: format!("Completed {} tool (ID: {})", tool_type, tool_id),
            sender: MessageSender::Tool(tool_type.to_string()),
            timestamp: current_time,
        });
    }

    // Reset processing flag
    app_state.processing_input = false;
}

// System to update the camera viewport to match the visualization area
// Back to the basic approach that makes tools visible
fn update_camera_viewport(windows: Query<&Window>, mut cameras: Query<&mut Camera>) {
    // Clear any viewport restrictions to ensure tools are visible
    for mut camera in cameras.iter_mut() {
        camera.viewport = None;
    }
}

// Task structure to handle async agent requests
#[derive(Resource)]
pub struct AgentTask {
    // Task status
    pub processing: bool,
    // Input that was processed
    pub input: String,
    // Channel for receiving responses from the async task
    pub receiver: Option<tokio::sync::mpsc::Receiver<AgentResponse>>,
}

impl Default for AgentTask {
    fn default() -> Self {
        Self {
            processing: false,
            input: String::new(),
            receiver: None,
        }
    }
}

// System to process agent tasks
// Checks the channel for responses from the async task
fn poll_agent_task(
    mut commands: Commands,
    mut app_state: ResMut<AppState>,
    mut agent_task: ResMut<AgentTask>,
    mut vis_state: ResMut<VisualizationState>,
    mut tool_query: Query<(&mut visualization::ToolEntity, &mut Sprite)>,
    time: Res<Time>,
) {
    // If we're not processing or don't have a receiver, nothing to do
    if !agent_task.processing || agent_task.receiver.is_none() {
        return;
    }

    let current_time = time.elapsed_secs_f64();

    // Try to get a response from the channel without blocking
    if let Some(receiver) = &mut agent_task.receiver {
        // Use try_recv to not block the game loop
        match receiver.try_recv() {
            Ok(response) => {
                trace!("Received response from async task");

                // Process the response
                process_agent_response(
                    &mut commands,
                    &mut app_state,
                    &mut vis_state,
                    &mut tool_query,
                    response,
                    current_time,
                );

                // Reset state
                agent_task.processing = false;
                agent_task.receiver = None;
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                // No response yet, that's OK
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                // Channel disconnected, reset state
                trace!("Channel disconnected, resetting agent task state");
                agent_task.processing = false;
                agent_task.receiver = None;

                // Add error message to journal
                app_state.journal_messages.push(JournalMessage {
                    content: "Lost connection to AI assistant. Please try again.".to_string(),
                    sender: MessageSender::System,
                    timestamp: current_time,
                });
            }
        }
    }
}

// UI system runs every frame
fn ui_system(
    mut contexts: bevy_egui::EguiContexts,
    mut app_state: ResMut<AppState>,
    time: Res<Time>,
    mut commands: Commands,
    mut vis_state: ResMut<VisualizationState>,
    mut tool_query: Query<(&mut visualization::ToolEntity, &mut Sprite)>,
    mut agent_task: ResMut<AgentTask>,
) {
    let ctx = contexts.ctx_mut();
    let current_time = time.elapsed_secs_f64();

    // Apply theme
    if app_state.dark_mode {
        let mut visuals = ctx.style().visuals.clone();
        visuals.dark_mode = true;
        ctx.set_visuals(visuals);
    }

    // Calculate screen divisions (25% for visualization, 50% for journal, 25% for input)
    let available_rect = ctx.screen_rect();
    let visualization_height = available_rect.height() * 0.25;
    let journal_height = available_rect.height() * 0.5;
    let input_height = available_rect.height() * 0.25;

    // Top pane - Visualization (handled by Bevy rendering)
    // Use the simplest approach - just a Window with an empty frame
    egui::Window::new("visualization_window")
        .frame(Frame::NONE)
        .title_bar(false)
        .resizable(false)
        .fixed_rect(egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(available_rect.width(), visualization_height),
        ))
        .show(ctx, |_ui| {
            // Intentionally leave empty
        });

    // Settings button (top-right corner)
    egui::Window::new("Settings Button")
        .frame(Frame::NONE)
        .title_bar(false)
        .resizable(false)
        .fixed_rect(egui::Rect::from_min_max(
            egui::pos2(available_rect.width() - 50.0, 10.0),
            egui::pos2(available_rect.width() - 10.0, 50.0),
        ))
        .show(ctx, |ui| {
            if ui.button("⚙").clicked() {
                app_state.show_settings = !app_state.show_settings;
            }
        });

    // Settings panel if shown
    if app_state.show_settings {
        egui::Window::new("Settings")
            .resizable(true)
            .default_size([300.0, 200.0])
            .show(ctx, |ui| {
                ui.heading("Display Settings");
                ui.checkbox(&mut app_state.dark_mode, "Dark Mode");

                ui.separator();
                ui.heading("Tool Visualization");
                if ui.button("Test Agent").clicked() {
                    // Add a test message
                    app_state.journal_messages.push(JournalMessage {
                        content: "Test agent functionality".to_string(),
                        sender: MessageSender::System,
                        timestamp: time.elapsed_secs_f64(),
                    });
                }

                ui.separator();
                if ui.button("Close").clicked() {
                    app_state.show_settings = false;
                }
            });
    }

    // Middle pane - Journal
    egui::Window::new("Journal")
        .frame(Frame::NONE.stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(80))))
        .title_bar(false) // Remove title bar for consistency with input pane
        .resizable(false)
        .fixed_rect(egui::Rect::from_min_max(
            egui::pos2(0.0, visualization_height),
            egui::pos2(
                available_rect.width(),
                visualization_height + journal_height,
            ),
        ))
        .show(ctx, |ui| {
            // Calculate exact dimensions we want for the journal
            let journal_width = available_rect.width() - 20.0; // Full window width minus small margin

            // Use a vertical layout for the journal section
            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                // Add heading at the top
                ui.heading("Journal");
                ui.add_space(4.0);

                // Scrollable journal area with explicit height limit
                // Add more padding (60.0 instead of 40.0) to prevent overlap with input area
                let available_height = ui.available_height() - 60.0; // Reserve more space for the heading and padding

                // Set a fixed width for all content to ensure scrollbar position is consistent
                ui.set_min_width(journal_width);

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(available_height)
                    .auto_shrink([false, false]) // Prevent auto-shrinking in both directions
                    .show(ui, |ui| {
                        // Set fixed width again for the scroll area content
                        ui.set_min_width(journal_width - 20.0);
                        // Add bottom margin to the scroll area
                        egui::Frame::NONE
                            .inner_margin(egui::Margin {
                                left: 0,
                                right: 0,
                                top: 0,
                                bottom: 10,
                            })
                            .show(ui, |ui| {
                                ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                    for message in &app_state.journal_messages {
                                        // Skip hidden system messages (tool trackers)
                                        if let MessageSender::System = &message.sender {
                                            if message.content.contains("<!-- TOOL_TRACKER:") {
                                                continue;
                                            }
                                        }

                                        // Style based on sender
                                        let (text_color, prefix) = match &message.sender {
                                            MessageSender::User => (egui::Color32::WHITE, "> "),
                                            MessageSender::Assistant => {
                                                (egui::Color32::from_rgb(100, 200, 255), "🤖 ")
                                            }
                                            MessageSender::System => (egui::Color32::GRAY, "📋 "),
                                            MessageSender::Tool(tool_type) => {
                                                let color = match tool_type.as_str() {
                                                    "file" => {
                                                        egui::Color32::from_rgb(100, 255, 100)
                                                    }
                                                    "network" => {
                                                        egui::Color32::from_rgb(100, 200, 255)
                                                    }
                                                    "process" => {
                                                        egui::Color32::from_rgb(255, 255, 100)
                                                    }
                                                    "database" => {
                                                        egui::Color32::from_rgb(255, 100, 100)
                                                    }
                                                    _ => egui::Color32::LIGHT_GRAY,
                                                };
                                                (color, &*format!("🔧 [{}] ", tool_type))
                                            }
                                        };

                                        // Draw the message with styling
                                        ui.horizontal(|ui| {
                                            let formatted_text =
                                                format!("{}{}", prefix, message.content);
                                            ui.colored_label(text_color, formatted_text);
                                        });
                                        // Add some space between messages instead of a separator
                                        ui.add_space(4.0);
                                    }
                                });
                            });
                    });
            });
        });

    // Bottom pane - Input
    egui::Window::new("Input")
        .frame(Frame::NONE.stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(80))))
        .title_bar(false) // Remove the title bar
        .resizable(false)
        .fixed_rect(egui::Rect::from_min_max(
            egui::pos2(0.0, visualization_height + journal_height),
            egui::pos2(
                available_rect.width(),
                visualization_height + journal_height + input_height,
            ),
        ))
        .show(ctx, |ui| {
            // Use a vertical top-down layout for the entire input section
            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                // Add heading at the top
                ui.heading("Input");
                ui.add_space(4.0);

                // Main input area
                // Input area without submit button - full width
                ui.horizontal(|ui| {
                    // Calculate available width - use full width
                    let available_width = ui.available_width();

                    // Create a styled text editor with dark background
                    let text_edit = egui::TextEdit::multiline(&mut app_state.input_text)
                        .desired_width(available_width)
                        .desired_rows(14) // Fill the input space
                        .hint_text(
                            "Type your command here... (Enter to submit, Shift+Enter for new line)",
                        )
                        .frame(true)
                        .margin(egui::vec2(8.0, 8.0))
                        // Set custom styling for the text editor
                        .text_color_opt(Some(egui::Color32::WHITE))
                        .font(egui::FontId::monospace(14.0));

                    // Add with custom background color frame
                    let frame = egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(30, 30, 30)) // Dark background
                        .inner_margin(egui::Margin::same(8))
                        .corner_radius(egui::CornerRadius::same(4));

                    // Use the frame to render the text editor
                    let response = frame.show(ui, |ui| ui.add(text_edit)).inner;

                    // Focus the text input when application starts
                    if ui.input(|i| i.time <= 0.1) {
                        response.request_focus();
                    }

                    // Handle key events:
                    // - Enter: Submit the message
                    // - Shift+Enter: Add a new line (handled automatically by TextEdit)
                    if response.has_focus()
                        && ui.input(|i| {
                            i.key_pressed(egui::Key::Enter) && !i.modifiers.shift
                            // Not holding shift
                        })
                    {
                        let input_text = app_state.input_text.clone();
                        if !input_text.is_empty() {
                            // Add user input to journal
                            app_state.journal_messages.push(JournalMessage {
                                content: input_text.clone(),
                                sender: MessageSender::User,
                                timestamp: current_time,
                            });

                            // If already processing input, don't process again
                            if app_state.processing_input {
                                // Add a notice that we're already processing
                                app_state.journal_messages.push(JournalMessage {
                                    content: "Already processing previous request, please wait...".to_string(),
                                    sender: MessageSender::System,
                                    timestamp: current_time,
                                });
                                return;
                            }

                            // Mark that we're processing input
                            app_state.processing_input = true;

                            // Make sure we have an agent manager
                            if let Some(agent_manager) = app_state.agent_manager.clone() {
                                // Create a clone of the input for the async task
                                let input_clone = input_text.clone();

                                // Add a "processing" message
                                app_state.journal_messages.push(JournalMessage {
                                    content: "Processing your request...".to_string(),
                                    sender: MessageSender::System,
                                    timestamp: current_time,
                                });

                                // Set agent task state
                                agent_task.processing = true;
                                agent_task.input = input_text.clone();

                                // Create a channel for communication
                                let (sender, receiver) = tokio::sync::mpsc::channel(1);
                                agent_task.receiver = Some(receiver);

                                // Clone what we need for the tokio task
                                let agent_manager_clone = agent_manager.clone();

                                // Create a tokio runtime for this task
                                let runtime = match tokio::runtime::Builder::new_current_thread()
                                    .enable_all()
                                    .build() {
                                        Ok(rt) => rt,
                                        Err(e) => {
                                            error!("Failed to create tokio runtime: {}", e);
                                            return;
                                        }
                                    };

                                // Use the runtime to spawn the task
                                std::thread::spawn(move || {
                                    // Block on the async task within the runtime
                                    runtime.block_on(async {
                                        // Get a lock on the agent manager
                                        let mut agent_manager = agent_manager_clone.lock().await;

                                        // Initialize if not already done
                                        if !agent_manager.is_initialized() {
                                            trace!("Agent manager not initialized, initializing...");

                                            // Register tools before initializing
                                            // File system tools
                                            agent_manager.register_tool(Box::new(crate::agent::tools::ReadFileTool));
                                            agent_manager.register_tool(Box::new(crate::agent::tools::WriteFileTool));
                                            agent_manager.register_tool(Box::new(crate::agent::tools::ListDirectoryTool));
                                            agent_manager.register_tool(Box::new(crate::agent::tools::ExecuteCommandTool));

                                            // Set working directory
                                            let current_dir = std::env::current_dir()
                                                .map(|p| p.to_string_lossy().to_string())
                                                .unwrap_or_else(|_| ".".to_string());
                                            agent_manager.set_working_directory(&current_dir);

                                            // Now initialize the backend
                                            if let Err(e) = agent_manager.init().await {
                                                error!("Failed to initialize agent: {}", e);
                                                return;
                                            }
                                            trace!("Agent manager initialized successfully");
                                        }

                                        // Process the input
                                        match agent_manager.process_input(&input_clone).await {
                                            Ok(mut response) => {
                                                // Log the initial response
                                                trace!("Initial response: got {} chars of response and {} tool results",
                                                      response.content.len(), response.tool_results.len());

                                                // If there are tool results, we need to continue the conversation
                                                if !response.tool_results.is_empty() {
                                                    trace!("Tool results present - continuing conversation");

                                                    // We need to continue the conversation but avoid adding an empty user message
                                                    // This requires more direct access to create a proper agent response

                                                    // We don't need to add the assistant message to the context again
                                                    // It was already added in process_input() before we get here
                                                    // Skipping: agent_manager.context_manager.add_assistant_message(&response.content);

                                                    // Generate a second response using the backend directly
                                                    let context = agent_manager.context_manager.get_context();
                                                    match agent_manager.backend.generate_response(&context).await {
                                                        Ok(mut backend_response) => {
                                                            trace!("Follow-up response after tools: {} chars", backend_response.content.len());

                                                            // Add the follow-up content to the original response
                                                            response.content = format!("{}\n\n{}",
                                                                                      response.content,
                                                                                      backend_response.content);

                                                            // Process any additional tool calls in the follow-up response with configurable chain depth
                                                            if !backend_response.tool_calls.is_empty() {
                                                                trace!("Follow-up contains {} more tool calls, processing with limited chain", 
                                                                      backend_response.tool_calls.len());
                                                                      
                                                                // Configure the depth of tool chaining
                                                                // Get max_depth from environment variable if present, or use default value
                                                                let max_depth = std::env::var("TOOL_CHAIN_MAX_DEPTH")
                                                                    .ok()
                                                                    .and_then(|v| v.parse::<usize>().ok())
                                                                    .unwrap_or(5);  // Increased default to 5
                                                                    
                                                                let config = ToolChainConfig {
                                                                    max_depth,  // Allow up to configured levels of tool chaining
                                                                    delay_ms: 200, // Small delay between API calls to avoid throttling
                                                                };
                                                                
                                                                trace!("Tool chain processing configured with max_depth={}, delay_ms={}", 
                                                                       config.max_depth, config.delay_ms);
                                                                
                                                                // Use our tool chain processor with:
                                                                // 1. The agent manager
                                                                // 2. The backend_response which contains the tool calls
                                                                // 3. Mutable reference to response.tool_results to capture any new results
                                                                // 4. Mutable reference to response.content to append new content
                                                                // 5. Custom configuration for tool chain depth and delay
                                                                // Log the tool results before processing
                                                                trace!("Tool results BEFORE processing chain: {} results", response.tool_results.len());
                                                                for (i, res) in response.tool_results.iter().enumerate() {
                                                                    trace!("  Result {}: Tool={}, ID={:?}", 
                                                                          i, 
                                                                          res.tool_name, 
                                                                          res.tool_call_id);
                                                                }
                                                                
                                                                // Process the tool chain
                                                                process_tool_chain_with_config(
                                                                    &mut agent_manager,
                                                                    backend_response.clone(), // Clone so we can still access the original below
                                                                    &mut response.tool_results,
                                                                    &mut response.content,
                                                                    config
                                                                ).await;
                                                                
                                                                // Log the tool results after processing
                                                                trace!("Tool results AFTER processing chain: {} results", response.tool_results.len());
                                                                for (i, res) in response.tool_results.iter().enumerate() {
                                                                    trace!("  Result {}: Tool={}, ID={:?}", 
                                                                          i, 
                                                                          res.tool_name, 
                                                                          res.tool_call_id);
                                                                }
                                                            } else {
                                                                trace!("Follow-up response contained no additional tool calls");
                                                            }
                                                            
                                                            // Add the assistant's follow-up response to the context for future messages
                                                            agent_manager.context_manager.add_assistant_message(&backend_response.content);
                                                        }
                                                        Err(e) => {
                                                            error!("Failed to get follow-up after tools: {}", e);
                                                        }
                                                    }
                                                }

                                                // Send the combined response to the main thread
                                                trace!("Sending final response: {} chars", response.content.len());
                                                if let Err(e) = sender.try_send(response) {
                                                    error!("Failed to send response to main thread: {}", e);
                                                }
                                            }
                                            Err(e) => {
                                                error!("Error processing input: {}", e);
                                            }
                                        }
                                    });
                                });
                            }

                            // Clear input box
                            app_state.input_text.clear();

                            // Request focus back to input
                            response.request_focus();
                        }
                    }
                });

                // Simple hint text at the bottom
                ui.with_layout(Layout::right_to_left(Align::RIGHT), |ui| {
                    ui.small("Press Enter to submit, Shift+Enter for new line");
                });
            });
        });
}
