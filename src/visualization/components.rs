use bevy::prelude::*;

// Bevy ECS components for visualization

// Tool status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    Idle,
    Running,
    Completed,
    Failed,
}

// Component to represent a tool in the visualization
#[derive(Component)]
pub struct ToolEntity {
    // The type of tool this entity represents
    pub tool_type: String,

    // Current status of the tool
    pub status: ToolStatus,

    // Time this entity has existed
    pub lifetime: f32,

    // Optional parent entity (for hierarchical tools)
    pub parent: Option<Entity>,

    // Any custom data for specific tool types
    pub custom_data: Option<String>,
}

impl ToolEntity {
    pub fn new(tool_type: &str) -> Self {
        Self {
            tool_type: tool_type.to_string(),
            status: ToolStatus::Idle,
            lifetime: 0.0,
            parent: None,
            custom_data: None,
        }
    }

    pub fn with_parent(mut self, parent: Entity) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_custom_data(mut self, data: &str) -> Self {
        self.custom_data = Some(data.to_string());
        self
    }

    pub fn start(&mut self) {
        self.status = ToolStatus::Running;
    }

    pub fn complete(&mut self) {
        self.status = ToolStatus::Completed;
    }

    pub fn fail(&mut self) {
        self.status = ToolStatus::Failed;
    }
}

// Component to represent a tool that follows a path
#[derive(Component)]
pub struct PathFollower {
    // Points in the path
    pub points: Vec<Vec2>,

    // Current point index
    pub current_index: usize,

    // Whether to loop when reaching the end
    pub looping: bool,

    // Movement speed
    pub speed: f32,

    // Whether moving forward or backward
    pub forward: bool,
}

// Component for a pulsing effect
#[derive(Component)]
pub struct Pulse {
    // Base size
    pub base_size: Vec2,

    // Minimum scale factor
    pub min_scale: f32,

    // Maximum scale factor
    pub max_scale: f32,

    // Current time in the pulse cycle
    pub time: f32,

    // Speed of pulsing
    pub speed: f32,
}

// Component for rotation
#[derive(Component)]
pub struct Rotation {
    // Speed of rotation in radians per second
    pub speed: f32,

    // Current rotation amount
    pub current: f32,
}

// Tag component for tools representing file operations
#[derive(Component)]
pub struct FileOperationTag;

// Tag component for tools representing network operations
#[derive(Component)]
pub struct NetworkOperationTag;

// Tag component for tools representing process operations
#[derive(Component)]
pub struct ProcessOperationTag;
