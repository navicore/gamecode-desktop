use crate::agent::tools::types::{Tool, ToolArgument, ToolArgumentType};
use async_trait::async_trait;
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::{debug, error};

/// Tool for reading files from the filesystem
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file from the filesystem"
    }

    fn required_args(&self) -> Vec<ToolArgument> {
        vec![ToolArgument {
            name: "path".to_string(),
            description: "Path to the file to read".to_string(),
            required: true,
            arg_type: ToolArgumentType::FilePath,
        }]
    }

    async fn execute(&self, args: &[String], working_dir: &str) -> Result<String, String> {
        if args.is_empty() {
            return Err("No file path provided".to_string());
        }

        let arg = args[0].clone();

        // Check if the argument is in the format "path=value"
        let path_value = if let Some(stripped) = arg.strip_prefix("path=") {
            stripped.to_string()
        } else {
            arg
        };

        let path = if path_value.starts_with('/') {
            // Absolute path
            path_value
        } else {
            // Relative path - prepend working directory
            format!("{}/{}", working_dir.trim_end_matches('/'), path_value)
        };

        // Read the file
        match fs::read_to_string(&path) {
            Ok(content) => Ok(content),
            Err(e) => {
                error!("Error reading file: {}", e);
                Err(format!("Error reading file: {}", e))
            }
        }
    }

    fn visualization_type(&self) -> &'static str {
        "file_read"
    }
}

/// Tool for writing to files in the filesystem
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write content to a file on the filesystem"
    }

    fn required_args(&self) -> Vec<ToolArgument> {
        vec![
            ToolArgument {
                name: "path".to_string(),
                description: "Path to the file to write".to_string(),
                required: true,
                arg_type: ToolArgumentType::FilePath,
            },
            ToolArgument {
                name: "content".to_string(),
                description: "Content to write to the file".to_string(),
                required: true,
                arg_type: ToolArgumentType::String,
            },
        ]
    }

    fn validate_args(&self, args: &[String]) -> Result<(), String> {
        if args.len() < 2 {
            return Err("Both file path and content are required".to_string());
        }
        Ok(())
    }

    async fn execute(&self, args: &[String], working_dir: &str) -> Result<String, String> {
        if args.len() < 2 {
            return Err("Both file path and content are required".to_string());
        }

        // Extract path parameter
        let arg_path = args[0].clone();
        let path_value = if let Some(stripped) = arg_path.strip_prefix("path=") {
            stripped.to_string()
        } else {
            arg_path
        };

        let path = if path_value.starts_with('/') {
            // Absolute path
            path_value
        } else {
            // Relative path - prepend working directory
            format!("{}/{}", working_dir.trim_end_matches('/'), path_value)
        };

        // Extract content parameter
        let arg_content = args[1].clone();
        let content = if let Some(stripped) = arg_content.strip_prefix("content=") {
            stripped.to_string()
        } else {
            arg_content
        };

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(&path).parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    error!("Error creating directories: {}", e);
                    return Err(format!("Error creating directories: {}", e));
                }
            }
        }

        // Write to the file
        match fs::write(&path, content) {
            Ok(_) => Ok(format!("Successfully wrote to file: {}", path)),
            Err(e) => {
                error!("Error writing to file: {}", e);
                Err(format!("Error writing to file: {}", e))
            }
        }
    }

    fn visualization_type(&self) -> &'static str {
        "file_write"
    }
}

/// Tool for listing files in a directory
pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &'static str {
        "list_directory"
    }

    fn description(&self) -> &'static str {
        "List files and directories in a specified path"
    }

    fn required_args(&self) -> Vec<ToolArgument> {
        vec![ToolArgument {
            name: "path".to_string(),
            description: "Path to the directory to list".to_string(),
            required: false, // If not provided, use working directory
            arg_type: ToolArgumentType::DirectoryPath,
        }]
    }

    async fn execute(&self, args: &[String], working_dir: &str) -> Result<String, String> {
        debug!("ListDirectoryTool called with args: {:?}, working_dir: {}", args, working_dir);
        
        // Use provided path or working directory
        let path = if !args.is_empty() {
            let arg = args[0].clone();
            debug!("Raw arg: '{}'", arg);

            // Check if the argument is in the format "path=value"
            let path_value = if let Some(stripped) = arg.strip_prefix("path=") {
                stripped.to_string()
            } else {
                arg
            };
            debug!("After prefix stripping: '{}'", path_value);
            
            // Remove any surrounding quotes (similar to ExecuteCommandTool)
            let path_value = path_value
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('\'')
                .trim_end_matches('\'')
                .to_string();
            debug!("After quote trimming: '{}'", path_value);

            // Check for various scenarios
            if path_value.starts_with('/') {
                // Absolute path
                debug!("Using as absolute path");
                path_value
            } else if path_value == working_dir || path_value == format!("\"{}\"", working_dir) {
                // If it's just a duplicate of working dir, use working dir directly
                debug!("Path is same as working dir, using working_dir directly");
                working_dir.to_string()
            } else if path_value.contains(working_dir) {
                // If it contains the working directory already, try to clean it up
                debug!("Path contains working dir, extracting just the path: '{}'", path_value);
                if path_value.starts_with(&format!("\"{}", working_dir)) && path_value.ends_with('"') {
                    // Handle case where working directory is quoted like: "/path/to/dir"
                    working_dir.to_string()
                } else {
                    path_value
                }
            } else {
                // Relative path - prepend working directory
                debug!("Using as relative path");
                format!("{}/{}", working_dir.trim_end_matches('/'), path_value)
            }
        } else {
            debug!("No args, using working_dir as path");
            working_dir.to_string()
        };

        debug!("Final resolved path: '{}'", path);
        
        // Read the directory
        let path_obj = Path::new(&path);
        if !path_obj.exists() {
            error!("Directory does not exist: '{}'", path);
            return Err(format!("Directory does not exist: {}", path));
        }
        if !path_obj.is_dir() {
            error!("Not a directory: '{}'", path);
            return Err(format!("Not a directory: {}", path));
        }

        // Read directory entries
        match fs::read_dir(&path) {
            Ok(entries) => {
                let mut result = format!("Contents of {}:\n", path);

                // Process entries
                for entry_result in entries {
                    match entry_result {
                        Ok(entry) => {
                            let file_name = entry.file_name();
                            let file_name_str = file_name.to_string_lossy();

                            let file_type = if let Ok(metadata) = entry.metadata() {
                                if metadata.is_dir() {
                                    "dir"
                                } else if metadata.is_file() {
                                    "file"
                                } else {
                                    "other"
                                }
                            } else {
                                "unknown"
                            };

                            result.push_str(&format!("{} ({})\n", file_name_str, file_type));
                        }
                        Err(e) => {
                            error!("Error reading directory entry: {}", e);
                            result.push_str(&format!("Error reading entry: {}\n", e));
                        }
                    }
                }

                Ok(result)
            }
            Err(e) => {
                error!("Error reading directory: {}", e);
                Err(format!("Error reading directory: {}", e))
            }
        }
    }

    fn visualization_type(&self) -> &'static str {
        "file_list"
    }
}

/// Tool for executing shell commands
pub struct ExecuteCommandTool;

impl ExecuteCommandTool {
    /// List of allowed commands for security
    pub fn allowed_commands() -> Vec<&'static str> {
        vec![
            "ls", "dir", "find", "grep", "cat", "head", "tail", "echo", "pwd",
        ]
    }
}

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn name(&self) -> &'static str {
        "execute_command"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command"
    }

    fn required_args(&self) -> Vec<ToolArgument> {
        vec![ToolArgument {
            name: "command".to_string(),
            description: "Command to execute".to_string(),
            required: true,
            arg_type: ToolArgumentType::String,
        }]
    }

    async fn execute(&self, args: &[String], working_dir: &str) -> Result<String, String> {
        if args.is_empty() {
            return Err("No command provided".to_string());
        }

        // Extract command parameter
        let arg_cmd = args[0].clone();
        let command = if arg_cmd.starts_with("command=") {
            let var_name = 8;
            arg_cmd[var_name..].to_string() // Extract the value after "command="
        } else {
            arg_cmd.clone() // Use the raw value
        };

        // Remove any surrounding quotes
        let command = command
            .trim_start_matches('"')
            .trim_end_matches('"')
            .trim_start_matches('\'')
            .trim_end_matches('\'')
            .to_string();

        debug!(
            "Original arg: '{}', parsed command: '{}', in directory: {}",
            arg_cmd, command, working_dir
        );

        // For security, we only support a limited set of commands for now
        // Expand this list based on your needs, but be careful with security implications

        // Use a more robust approach to split the command, preserving quoted parts
        let mut cmd_parts = Vec::new();
        let mut current_part = String::new();
        let mut in_quotes = false;
        let mut quote_char = ' ';

        for c in command.chars() {
            match c {
                '"' | '\'' => {
                    if !in_quotes {
                        // Start of quoted section
                        in_quotes = true;
                        quote_char = c;
                    } else if c == quote_char {
                        // End of quoted section
                        in_quotes = false;
                    } else {
                        // Different quote character inside quotes, treat as normal char
                        current_part.push(c);
                    }
                }
                ' ' | '\t' => {
                    if in_quotes {
                        // Space inside quotes, keep it
                        current_part.push(c);
                    } else if !current_part.is_empty() {
                        // End of part
                        cmd_parts.push(current_part);
                        current_part = String::new();
                    }
                }
                _ => {
                    current_part.push(c);
                }
            }
        }

        // Add the last part if not empty
        if !current_part.is_empty() {
            cmd_parts.push(current_part);
        }

        debug!("Parsed command parts: {:?}", cmd_parts);

        if cmd_parts.is_empty() {
            return Err("Empty command".to_string());
        }

        // Security check for allowed commands
        let base_command = &cmd_parts[0];
        let allowed_commands = Self::allowed_commands();

        if allowed_commands.iter().any(|&cmd| cmd == base_command) {
            // Command is allowed, now check the arguments

            // Additional validation for command arguments
            for arg in &cmd_parts[1..] {
                // Restrict arguments that could be harmful
                if arg.starts_with(";")
                    || arg.starts_with("&&")
                    || arg.starts_with("||")
                    || arg.starts_with("|")
                    || arg.starts_with(">")
                    || arg.starts_with("<")
                    || arg.contains("$(")
                    || arg.contains("`")
                    || arg.contains("${")
                {
                    return Err(format!(
                        "Argument '{}' contains potentially unsafe characters",
                        arg
                    ));
                }
            }
        } else {
            return Err(format!(
                "Command '{}' is not allowed for security reasons. Allowed commands are: {}",
                base_command,
                allowed_commands.join(", ")
            ));
        }

        // Execute the command
        let output = Command::new(&cmd_parts[0])
            .args(&cmd_parts[1..])
            .current_dir(working_dir)
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let mut result = String::new();

        if !stdout.is_empty() {
            result.push_str(&stdout);
        }

        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n\nErrors:\n");
            }
            result.push_str(&stderr);
        }

        if result.is_empty() {
            result = "Command executed successfully with no output".to_string();
        }

        Ok(result)
    }

    fn visualization_type(&self) -> &'static str {
        "command_execution"
    }
}
