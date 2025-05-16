use crate::visualization::components::*;
use bevy::prelude::*;

// Bevy ECS systems for visualization

// This system runs once at startup to set up the visualization scene
pub fn setup_visualization_system() {
    println!("Setting up visualization system...");

    // This would normally set up the initial scene elements
    // For now it's just a placeholder
}

// This system runs every frame to update the visualization
pub fn update_visualization_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ToolEntity, &mut Transform)>,
    windows: Query<&Window>,
) {
    // Log running status periodically to avoid console spam
    if (time.elapsed_secs_f64() % 5.0) < 0.01 {
        println!("Visualization update running...");
    }

    // Get window dimensions to calculate proper scaling
    let window = windows.get_single().ok();
    let window_height = window.map(|w| w.resolution.height()).unwrap_or(960.0);
    let vis_height = window_height * 0.25; // Visualization area is 25% of window height

    // Example animation: make tools rotate and ensure they use the full visualization area
    for (_entity, tool_entity, mut transform) in query.iter_mut() {
        // Make tool entities rotate
        match tool_entity.status {
            ToolStatus::Running => {
                transform.rotate_z(time.delta_secs() * 2.0);

                // Add a simple oscillation to make tools move up and down
                // Keep the movement small to ensure it stays within the viewport
                // Use f32 for consistency with Transform components
                let oscillation = (time.elapsed_secs_f64() * 0.8).sin() as f32 * 30.0;
                transform.translation.y += oscillation * time.delta_secs();
            }
            ToolStatus::Completed => {
                // Slow down rotation if completed
                transform.rotate_z(time.delta_secs() * 0.5);
            }
            ToolStatus::Failed => {
                // Reverse rotation if failed
                transform.rotate_z(-time.delta_secs() * 1.0);
            }
            _ => {}
        }
    }
}

// This system adds new tool visualization entities
pub fn spawn_tool_visualization(
    commands: &mut Commands,
    tool_type: &str,
    position: Vec3,
) -> Entity {
    // Create a new tool entity
    let tool = ToolEntity::new(tool_type);

    // Use an extremely bright, large sprite that should be clearly visible
    // In Bevy 0.15, we use the Sprite component directly instead of SpriteBundle
    commands
        .spawn((
            // Create a sprite with color based on tool status
            Sprite {
                // Use extremely bright colors that stand out
                color: match tool.status {
                    ToolStatus::Idle => Color::srgba(0.8, 0.8, 0.8, 1.0), // Bright white
                    ToolStatus::Running => Color::srgba(1.0, 1.0, 0.0, 1.0), // Bright yellow
                    ToolStatus::Completed => Color::srgba(0.0, 1.0, 0.0, 1.0), // Bright green
                    ToolStatus::Failed => Color::srgba(1.0, 0.0, 0.0, 1.0), // Bright red
                },
                // Make it very large to be sure it's visible
                custom_size: Some(Vec2::new(120.0, 120.0)),
                ..default()
            },
            // Add the Transform component separately
            Transform::from_translation(position),
            // In Bevy 0.15, Visibility components are added automatically
            // Add our custom tool component
            tool,
        ))
        .id()
}

// Updates a tool entity's status
pub fn update_tool_status(
    commands: &mut Commands,
    entity: Entity,
    status: ToolStatus,
    tool_query: &mut Query<(&mut ToolEntity, &mut Sprite)>,
) {
    if let Ok((mut tool, mut sprite)) = tool_query.get_mut(entity) {
        // Update status
        tool.status = status;

        // Update color based on new status
        // In Bevy 0.15, we need to use srgba instead of color constants
        sprite.color = match status {
            ToolStatus::Idle => Color::srgba(0.5, 0.5, 0.5, 1.0), // Gray
            ToolStatus::Running => Color::srgba(1.0, 1.0, 0.0, 1.0), // Yellow
            ToolStatus::Completed => Color::srgba(0.0, 1.0, 0.0, 1.0), // Green
            ToolStatus::Failed => Color::srgba(1.0, 0.0, 0.0, 1.0), // Red
        };
    }
}
