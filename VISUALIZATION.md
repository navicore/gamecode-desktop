# Visualization System Documentation

This document explains how the visualization system works, particularly focusing on positioning sprites in the top 25% section of the application.

## Overview

The application is divided into three sections:
- Top 25%: Visualization area (Bevy rendering)
- Middle 50%: Journal/log
- Bottom 25%: Input area

## Key Components

### 1. Camera Setup (app.rs)

```rust
// In setup_system function
let camera = Camera2dBundle::default();
commands.spawn(camera);
```

We use a default Camera2dBundle without any viewport restrictions. This means:
- The camera looks at the origin (0,0) which is the center of the entire window
- Rendering is not clipped to any specific region

### 2. Tool Positioning (visualization/mod.rs)

The critical insight for positioning sprites in the top section is to calculate the correct Y offset:

```rust
// Calculate offset needed to move tools to the visualization area
let window_height = 960.0; // Default height estimate
let vis_height = window_height * 0.25; // Visualization height (25%)

// To move from the center to the visualization area:
// 1. Center of window is at y=0
// 2. Center of visualization area is at y=(window_height*0.5 - vis_height*0.5)
let y_offset = (window_height * 0.5) - (vis_height * 0.5);

// Position first tool at this offset
Vec3::new(0.0, y_offset, 0.0)
```

This formula calculates how far to move objects from the center of the screen to the center of the top 25% section.

### 3. Coordinate System

In Bevy's 2D coordinate system:
- Origin (0,0) is at the center of the window
- Positive Y is up, positive X is right
- For a typical 1280x960 window:
  - X ranges from approximately -640 to 640
  - Y ranges from approximately -480 to 480
  - The top 25% section's center is at approximately Y=360

### 4. Viewport Considerations

While we're currently not using a viewport restriction, it's possible to limit rendering to just the top section:

```rust
// Example viewport configuration for top 25%
camera.viewport = Some(bevy::render::camera::Viewport {
    // In OpenGL/Vulkan coordinates (0,0) is bottom-left
    // To render at top of window, calculate from bottom:
    physical_position: UVec2::new(0, window_physical_height - vis_physical_height),
    physical_size: UVec2::new(window_width, vis_physical_height),
    ..default()
});
```

However, we found that using the default camera without viewport restrictions and positioning sprites with the correct Y offset provides better results.

## Creating Future Game Elements

For future game development with physics:

1. **Positioning Game Elements**:
   - Always use the calculated Y offset to position elements in the top section
   - For horizontal positioning, use the full width of the window

2. **Boundaries**:
   - Top boundary: approximately Y offset + (vis_height/2)
   - Bottom boundary: approximately Y offset - (vis_height/2)
   - Left boundary: approximately -window_width/2
   - Right boundary: approximately window_width/2

3. **Camera Considerations**:
   - If using physics, consider whether you need orthographic or perspective projection
   - For complex scenes, you might need to adjust the camera's Z position

## Working with Tools

When spawning tools or other game elements:

```rust
// Example for random X positioning across the full width
let window_width = 1280.0;
let x_range = window_width * 0.8; // Use 80% of width to keep from edges

// Random X position
let x_pos = (rand::random::<f32>() - 0.5) * x_range;

// Position with calculated Y offset and random X
Vec3::new(x_pos, y_offset, 0.0)
```

This approach ensures elements are positioned in the top visualization area while utilizing the full horizontal space.