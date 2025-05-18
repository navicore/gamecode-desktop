# AWS Bedrock Integration with Claude Models

This document explains how to test the AWS Bedrock integration with Claude models in this application.

## Prerequisites

1. An AWS account with access to AWS Bedrock
2. Claude models enabled in AWS Bedrock (Claude 3.7 Sonnet and Claude 3.5 Haiku)
3. AWS credentials configured on your machine

## Testing the Integration

To test the AWS Bedrock integration, follow these steps:

1. Make sure your AWS credentials are configured. You can use either environment variables or the AWS credentials file:

   - Environment variables:
     ```
     export AWS_ACCESS_KEY_ID=your_access_key
     export AWS_SECRET_ACCESS_KEY=your_secret_key
     export AWS_REGION=us-east-1
     ```

   - Or, AWS credentials file (typically at `~/.aws/credentials`):
     ```
     [default]
     aws_access_key_id = your_access_key
     aws_secret_access_key = your_secret_key
     aws_region = us-east-1
     ```

2. Run the application with the `--test-bedrock` flag:
   ```
   cargo run -- --test-bedrock
   ```

   This will:
   - Initialize the AWS Bedrock client
   - Register file system tools (read_file, write_file, list_directory)
   - Send a test query to Claude asking it to list files and create a test file
   - Display the response from Claude and any tool results

## Code Structure

The AWS Bedrock integration consists of the following components:

1. **BedrockBackend**: Implements the backend for AWS Bedrock with Claude models
   - Located at `src/agent/backends/bedrock.rs`
   - Handles API requests and responses
   - Supports model switching between Sonnet and Haiku
   - Includes error handling and retries

2. **AgentManager**: Manages the agent including backend, tools, and context
   - Located at `src/agent/manager.rs`
   - Initializes the backend
   - Processes user input
   - Parses tool calls from LLM responses
   - Executes tools and manages context

3. **Tools**: Filesystem tools for the agent to use
   - Located at `src/agent/tools/filesystem.rs`
   - ReadFileTool: Reads contents of files
   - WriteFileTool: Writes content to files
   - ListDirectoryTool: Lists contents of directories

## Configuration

You can configure the AWS Bedrock integration by modifying:

1. **BedrockConfig**: Configure AWS region, model settings, and more
   - Token limits for each model
   - Temperature settings
   - AWS profile settings
   - Retry settings

2. **AgentConfig**: Configure agent behavior
   - Fast model for context compression
   - Max context length
   - AWS region and profile

## Known Limitations

1. The current implementation uses a basic regex-based tool call parser, which may not handle all edge cases
2. Error handling could be improved with more detailed error messages
3. The current implementation does not include robust token counting
4. The API is subject to change as the implementation evolves