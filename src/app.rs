use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::egui::{Align, Frame, Layout};

use crate::agent;
use crate::core;
use crate::ui;
use crate::visualization::{self, ToolStatus, VisualizationPlugin, VisualizationState};

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

    // Demo tool state
    last_demo_tool_time: f32,
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
    println!("Application starting...");

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
        // Add default Bevy plugins
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "GameCode - AI Agent Visualization".to_string(),
                resolution: (1280.0, 960.0).into(),
                ..default()
            }),
            ..default()
        }))
        // Add egui for UI components
        // In Bevy 0.15
        .add_plugins(bevy_egui::EguiPlugin)
        // Add visualization plugin
        .add_plugins(VisualizationPlugin)
        // Add app resources
        .init_resource::<AppState>()
        // Add our systems
        .add_systems(Startup, setup_system)
        // In Bevy 0.15, we need to chain system configurations
        .add_systems(Update, ui_system)
        .add_systems(Update, demo_tool_system) // This will create demo tools for testing
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
            last_demo_tool_time: 0.0,
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

    println!(
        "Camera positioned at Y={} with viewport at {},{} size {}x{}",
        y_offset, 0, viewport_bottom_left_y, width, height
    );
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

// Demo system to create example tools for testing visualization
fn demo_tool_system(
    mut commands: Commands,
    time: Res<Time>,
    mut app_state: ResMut<AppState>,
    mut vis_state: ResMut<VisualizationState>,
    mut tool_query: Query<(&mut visualization::ToolEntity, &mut Sprite)>,
) {
    // Store the current time
    let current_time = time.elapsed_secs_f64();

    // Create a new tool every 5 seconds for demo purposes
    if time.elapsed_secs() - app_state.last_demo_tool_time > 5.0 {
        let tool_id = generate_tool_id(&mut app_state);

        // Random tool type for demo
        let tool_types = ["file", "network", "process", "database"];
        let tool_type = tool_types[app_state.tool_id_counter % tool_types.len()];

        // Start a new tool visualization
        visualization::start_tool_visualization(&mut commands, &mut vis_state, &tool_id, tool_type);

        // Update the status to running
        visualization::update_tool_status_public(
            &mut commands,
            &mut vis_state,
            &tool_id,
            ToolStatus::Running,
            &mut tool_query,
        );

        // Add a journal message for the tool
        app_state.journal_messages.push(JournalMessage {
            content: format!("Started {} tool (ID: {})", tool_type, tool_id),
            sender: MessageSender::Tool(tool_type.to_string()),
            timestamp: current_time,
        });

        // Update the last tool time
        app_state.last_demo_tool_time = time.elapsed_secs();

        // In a real application, we'd use a proper task scheduler
        // For this demo, we'll create a simple tracking mechanism

        // Create a custom tag in the journal for tracking
        app_state.journal_messages.push(JournalMessage {
            content: format!(
                "<!-- TOOL_TRACKER:{}:{}:{} -->",
                tool_id, tool_type, current_time
            ),
            sender: MessageSender::System,
            timestamp: current_time,
        });
    }

    // This is a hack to track tools in this demo
    // In a real application, we'd have a proper tracking system
    // Scan the messages for tracker tags
    let mut tools_to_complete = Vec::new();

    for message in &app_state.journal_messages {
        if let MessageSender::System = message.sender {
            if message.content.contains("<!-- TOOL_TRACKER:") {
                // Parse the tracker
                let parts: Vec<&str> = message.content.split(':').collect();
                if parts.len() >= 4 {
                    let tool_id = parts[1];
                    let tool_type = parts[2];
                    let start_time: f64 = parts[3].trim_end_matches(" -->").parse().unwrap_or(0.0);

                    // Check if it's time to complete this tool
                    if current_time - start_time > 3.0 {
                        // Check if it's already been completed
                        let already_completed = app_state.journal_messages.iter().any(|m| {
                            if let MessageSender::Tool(_) = m.sender {
                                m.content.contains(&format!(
                                    "Completed {} tool (ID: {})",
                                    tool_type, tool_id
                                )) || m.content.contains(&format!(
                                    "Failed {} tool (ID: {})",
                                    tool_type, tool_id
                                ))
                            } else {
                                false
                            }
                        });

                        if !already_completed {
                            tools_to_complete.push((tool_id.to_string(), tool_type.to_string()));
                        }
                    }
                }
            }
        }
    }

    // Now complete any tools that need completion
    for (tool_id, tool_type) in tools_to_complete {
        // Randomly succeed or fail
        let success = rand::random::<bool>();

        // Update the status
        visualization::update_tool_status_public(
            &mut commands,
            &mut vis_state,
            &tool_id,
            if success {
                ToolStatus::Completed
            } else {
                ToolStatus::Failed
            },
            &mut tool_query,
        );

        // Add a journal message for the completion
        app_state.journal_messages.push(JournalMessage {
            content: format!(
                "{} {} tool (ID: {})",
                if success { "Completed" } else { "Failed" },
                tool_type,
                tool_id
            ),
            sender: MessageSender::Tool(tool_type),
            timestamp: current_time,
        });
    }
}

// System to update the camera viewport to match the visualization area
// Back to the basic approach that makes tools visible
fn update_camera_viewport(windows: Query<&Window>, mut cameras: Query<&mut Camera>) {
    // Clear any viewport restrictions to ensure tools are visible
    for mut camera in cameras.iter_mut() {
        camera.viewport = None;
    }

    println!("Viewport cleared - rendering to full window");
}

// UI system runs every frame
fn ui_system(
    mut contexts: bevy_egui::EguiContexts,
    mut app_state: ResMut<AppState>,
    time: Res<Time>,
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
                if ui.button("Add Demo Tool").clicked() {
                    // Force a demo tool to be created immediately
                    app_state.last_demo_tool_time = 0.0;
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

                            // Add a mock assistant response
                            // In a real app, this would go through the agent
                            app_state.journal_messages.push(JournalMessage {
                                content: format!("I received your message: '{}'", input_text),
                                sender: MessageSender::Assistant,
                                timestamp: current_time,
                            });

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
