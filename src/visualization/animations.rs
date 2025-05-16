use crate::visualization::components::ToolStatus;
use crate::visualization::systems::{spawn_tool_visualization, update_tool_status};
use bevy::prelude::*;
use std::collections::HashMap;
use std::time::Duration;

// Animations for tool execution visualization
pub struct AnimationManager {
    // Map of active tool animations with their type and entity ID
    pub active_tools: HashMap<String, Entity>,

    // Animation settings
    animation_speed: f32,
}

impl AnimationManager {
    pub fn new() -> Self {
        Self {
            active_tools: HashMap::new(),
            animation_speed: 1.0,
        }
    }

    // Start animating a tool execution
    pub fn start_tool_animation(
        &mut self,
        commands: &mut Commands,
        tool_id: &str,
        tool_type: &str,
        position: Vec3,
    ) {
        println!(
            "Starting animation for {} tool (ID: {})",
            tool_type, tool_id
        );

        // Create a new tool entity in the visualization
        let entity = spawn_tool_visualization(commands, tool_type, position);

        // Store the entity for later reference
        self.active_tools.insert(tool_id.to_string(), entity);
    }

    // Update a tool's animation status
    pub fn update_tool_status(
        &mut self,
        commands: &mut Commands,
        tool_id: &str,
        status: ToolStatus,
        tool_query: &mut Query<(
            &mut crate::visualization::components::ToolEntity,
            &mut Sprite,
        )>,
    ) {
        if let Some(&entity) = self.active_tools.get(tool_id) {
            update_tool_status(commands, entity, status, tool_query);
        }
    }

    // Complete a tool's animation (success or failure)
    pub fn complete_tool_animation(
        &mut self,
        commands: &mut Commands,
        tool_id: &str,
        success: bool,
        tool_query: &mut Query<(
            &mut crate::visualization::components::ToolEntity,
            &mut Sprite,
        )>,
    ) {
        let status = if success {
            ToolStatus::Completed
        } else {
            ToolStatus::Failed
        };

        if let Some(&entity) = self.active_tools.get(tool_id) {
            update_tool_status(commands, entity, status, tool_query);

            let result = if success { "successful" } else { "failed" };
            println!(
                "Completing animation with {} result for tool ID: {}",
                result, tool_id
            );

            // In a real implementation, we might schedule the entity for removal
            // after some delay so the user can see the success/failure state
        }
    }

    // Set animation speed (1.0 is normal speed)
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.animation_speed = speed.max(0.1).min(3.0);
    }

    // Get current animation speed
    pub fn animation_speed(&self) -> f32 {
        self.animation_speed
    }
}

// Different animation patterns for different tool types
pub enum AnimationPattern {
    // A simple rotation animation
    Rotate {
        speed: f32,
    },

    // A scaling animation (growing and shrinking)
    Scale {
        min_scale: f32,
        max_scale: f32,
        speed: f32,
    },

    // A blinking animation (changing opacity)
    Blink {
        min_alpha: f32,
        max_alpha: f32,
        speed: f32,
    },

    // A moving animation (following a path)
    Path {
        points: Vec<Vec2>,
        loop_animation: bool,
        speed: f32,
    },
}

// Get the appropriate animation pattern for a tool type
pub fn get_animation_for_tool(tool_type: &str) -> AnimationPattern {
    match tool_type {
        "file" => AnimationPattern::Scale {
            min_scale: 0.8,
            max_scale: 1.2,
            speed: 1.0,
        },
        "process" => AnimationPattern::Rotate { speed: 2.0 },
        "network" => AnimationPattern::Path {
            points: vec![Vec2::new(-50.0, 0.0), Vec2::new(50.0, 0.0)],
            loop_animation: true,
            speed: 1.5,
        },
        _ => AnimationPattern::Blink {
            min_alpha: 0.5,
            max_alpha: 1.0,
            speed: 1.0,
        },
    }
}
