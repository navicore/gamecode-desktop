# GameCode - AI Agent Visualization Interface

# UNDER CONSTRUCTION

# UNDER CONSTRUCTION

# UNDER CONSTRUCTION

## Project Overview
GameCode is an AI agent interface that combines Bevy for game-like visualizations with Lapce-powered text editors. The application features three panes:
1. Top pane: Bevy-rendered visualization of AI agent activities
2. Middle pane: Journal/message log with editor capabilities
3. Bottom pane: Input editor with programmer-friendly keybindings

## Project Structure

```
gamecode/
├── src/
│   ├── main.rs            # Application entry point
│   ├── app.rs             # Main app coordination
│   ├── ui/                # UI components
│   │   ├── mod.rs         # UI module exports
│   │   ├── layout.rs      # Manages the three-pane layout
│   │   └── editor/        # Editor integrations
│   │       ├── mod.rs
│   │       ├── input.rs   # Bottom input editor
│   │       └── journal.rs # Middle message log editor
│   ├── visualization/     # Bevy visualization components
│   │   ├── mod.rs
│   │   ├── systems.rs     # Bevy ECS systems
│   │   ├── components.rs  # Bevy ECS components
│   │   └── animations.rs  # Tool execution animations
│   ├── agent/             # AI agent implementation
│   │   ├── mod.rs         # Agent module exports
│   │   ├── manager.rs     # Central agent manager
│   │   ├── context.rs     # Conversation context management
│   │   ├── backends/      # LLM backend implementations
│   │   │   ├── mod.rs
│   │   │   └── bedrock.rs # AWS Bedrock integration
│   │   └── tools/         # Tool implementations
│   │       ├── mod.rs
│   │       ├── registry.rs # Tool registry
│   │       ├── executor.rs # Tool execution
│   │       └── types.rs    # Tool interfaces and types
│   └── core/              # Core functionality
│       ├── mod.rs
│       ├── state.rs       # Application state
└── Cargo.toml             # Project dependencies
```

## Implementation Plan

1. **Basic Framework Setup**
   - Set up Bevy and window creation
   - Integrate Lapce editor components
   - Establish the three-pane layout

2. **Editor Configuration**
   - Configure Lapce for the input pane with programmer keybindings
   - Set up the journal pane with read-mostly permissions
   - Implement APIs for adding/updating content

3. **Agent System Implementation**
   - Implement AWS Bedrock backend (Sonnet & Haiku models)
   - Build the tool registry and execution system
   - Create context management with compression

4. **Bevy Visualization Layer**
   - Create basic Bevy scene structure
   - Implement entity representation for tools/operations
   - Design animation system for tool execution

5. **Integration Layer**
   - Build communication channels between UI, agent, and visualization
   - Create event system for tool execution
   - Implement state synchronization

6. **Tool Development**
   - Implement core system tools (file, process, network)
   - Add programming-specific tools (code analysis, editing)
   - Develop tool visualization mappings

7. **Polishing**
   - Add keybinding configuration
   - Implement theme support
   - Add persistence for session history

## Usage

(To be added)

## Requirements

- Rust (latest stable version)
- GPU with Vulkan/Metal/DirectX support

## License

(To be determined)
