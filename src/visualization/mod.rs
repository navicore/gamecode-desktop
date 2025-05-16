mod animations;
mod components;
mod systems;

pub use animations::*;
pub use components::*;
pub use systems::*;

use bevy::prelude::*;

// Resource to track active visualizations
#[derive(Resource)]
pub struct VisualizationState {
    // Manager for animations
    pub animation_manager: animations::AnimationManager,

    // Last position used for spawning a tool
    pub last_position: Vec3,

    // Whether visualization is paused
    pub paused: bool,
}

impl Default for VisualizationState {
    fn default() -> Self {
        Self {
            animation_manager: animations::AnimationManager::new(),
            last_position: Vec3::ZERO,
            paused: false,
        }
    }
}

// Visualization initialization and management
pub fn init() {
    println!("Initializing visualization components...");
}

// Plugin to add all visualization systems to the Bevy app
pub struct VisualizationPlugin;

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        // Ensure our plugin has high priority for rendering
        app.init_resource::<VisualizationState>()
            .add_systems(Startup, setup_visualization_system)
            // Try a different approach - add our systems early in the frame
            .add_systems(
                PreUpdate, // Use PreUpdate instead of Update for higher priority
                (update_visualization_system, update_tool_animations),
            );

        println!("Visualization plugin initialized with high priority");
    }
}

// System to update all tool animations
fn update_tool_animations(
    time: Res<Time>,
    mut vis_state: ResMut<VisualizationState>,
    mut query: Query<(Entity, &mut ToolEntity, &mut Transform)>,
) {
    // Skip if paused
    if vis_state.paused {
        return;
    }

    // Update lifetimes for all tools
    for (_entity, mut tool, _transform) in query.iter_mut() {
        tool.lifetime += time.delta_seconds();
    }
}

// Public function to start a tool visualization
// This function is called when a new tool is executed and needs to be visualized
pub fn start_tool_visualization(
    commands: &mut Commands,
    vis_state: &mut VisualizationState,
    tool_id: &str,
    tool_type: &str,
) {
    // Use sensible defaults for window dimensions if not available in this context
    // In a real application, we would get these from a resource but for this demo we'll estimate
    let window_width = 1280.0; // Default fallback width
    let window_height = 960.0; // Default fallback height

    // Compute a position based on existing tools
    // In Bevy's 2D coordinate system:
    // - Origin (0,0) is at the center of the screen
    // - Positive Y is up, positive X is right
    // - The visualization area is the top 25% of the window

    // Calculate the visualization height (25% of window height)
    let vis_height = window_height * 0.25;

    // We need to ensure tools spread throughout the visualization area
    // Position tools throughout the available visualization area
    // Position tools around the origin (0,0)
    // The camera has been moved to look at the center of the top section,
    // so tools at (0,0) should appear in the center of that section
    // Position tools where we know they'll be at least partially visible
    // We'll try a few different positions by hardcoding them
    let position = if vis_state.last_position == Vec3::ZERO {
        // First tool - we know it's visible at some Y value, so try different positions
        Vec3::new(0.0, 300.0, 0.0)  // High positive Y value
    } else {
        // Subsequent tools in a pattern
        let num_tools = vis_state.animation_manager.active_tools.len() as f32;
        let angle = num_tools * 0.8;

        // Use a small radius for the circular pattern
        let radius = 40.0;
        
        Vec3::new(
            radius * angle.cos(),
            300.0 + (radius * angle.sin()),  // Keep them all in the visible area
            0.0,
        )
    };

    // Store the position
    vis_state.last_position = position;

    // Start the animation
    vis_state
        .animation_manager
        .start_tool_animation(commands, tool_id, tool_type, position);
}

// Public function to update a tool's status
pub fn update_tool_status_public(
    commands: &mut Commands,
    vis_state: &mut VisualizationState,
    tool_id: &str,
    status: ToolStatus,
    tool_query: &mut Query<(&mut ToolEntity, &mut Sprite)>,
) {
    vis_state
        .animation_manager
        .update_tool_status(commands, tool_id, status, tool_query);
}
