use bevy::prelude::*;
use crate::visualization::components::*;

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
    if (time.elapsed_seconds() % 5.0) < 0.01 {
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
                transform.rotate_z(time.delta_seconds() * 2.0);
                
                // Add a simple oscillation to make tools move up and down
                // Keep the movement small to ensure it stays within the viewport
                let oscillation = (time.elapsed_seconds() * 0.8).sin() * 30.0;
                transform.translation.y += oscillation * time.delta_seconds();
            },
            ToolStatus::Completed => {
                // Slow down rotation if completed
                transform.rotate_z(time.delta_seconds() * 0.5);
            },
            ToolStatus::Failed => {
                // Reverse rotation if failed
                transform.rotate_z(-time.delta_seconds() * 1.0);
            },
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
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                // Use extremely bright colors that stand out
                color: match tool.status {
                    ToolStatus::Idle => Color::rgba(0.8, 0.8, 0.8, 1.0),      // Bright white
                    ToolStatus::Running => Color::rgba(1.0, 1.0, 0.0, 1.0),   // Bright yellow
                    ToolStatus::Completed => Color::rgba(0.0, 1.0, 0.0, 1.0), // Bright green
                    ToolStatus::Failed => Color::rgba(1.0, 0.0, 0.0, 1.0),    // Bright red
                },
                // Make it very large to be sure it's visible
                custom_size: Some(Vec2::new(120.0, 120.0)),
                ..default()
            },
            // Ensure Z position is appropriate for visibility (0.0 is default for 2D)
            transform: Transform::from_translation(Vec3::new(position.x, position.y, 0.0)),
            // Explicitly set visibility to be sure
            visibility: Visibility::Visible,
            ..default()
        },
        tool,
    )).id()
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
        sprite.color = match status {
            ToolStatus::Idle => Color::GRAY,
            ToolStatus::Running => Color::YELLOW,
            ToolStatus::Completed => Color::GREEN,
            ToolStatus::Failed => Color::RED,
        };
    }
}
