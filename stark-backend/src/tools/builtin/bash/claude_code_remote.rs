//! Claude Code Remote tool — SSH into a remote machine running Claude Code CLI

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::timeout;

use crate::tools::{
    PropertySchema, Tool, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
    ToolSafetyLevel,
};

const DEFAULT_TIMEOUT_SECS: u64 = 300;
const MAX_TIMEOUT_SECS: u64 = 600;
const DEFAULT_SSH_PORT: &str = "22";

pub struct ClaudeCodeRemoteTool {
    definition: ToolDefinition,
}

#[derive(Debug, Deserialize)]
struct ClaudeCodeRemoteParams {
    prompt: String,
    workdir: Option<String>,
    allowed_tools: Option<Vec<String>>,
    append_system_prompt: Option<String>,
    model: Option<String>,
    timeout: Option<u64>,
    max_turns: Option<u64>,
}

impl ClaudeCodeRemoteTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "prompt".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "The prompt to send to Claude Code on the remote machine".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "workdir".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Working directory on the remote machine to cd into before running claude. Defaults to home directory.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "allowed_tools".to_string(),
            PropertySchema {
                schema_type: "array".to_string(),
                description: "List of tool names to allow Claude Code to use (e.g. [\"Bash\", \"Read\", \"Write\"])".to_string(),
                default: None,
                items: Some(Box::new(PropertySchema {
                    schema_type: "string".to_string(),
                    description: "Tool name".to_string(),
                    default: None,
                    items: None,
                    enum_values: None,
                })),
                enum_values: None,
            },
        );

        properties.insert(
            "append_system_prompt".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Additional system prompt text to append to Claude Code's default system prompt".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "model".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Model to use (e.g. 'claude-sonnet-4-5-20250929'). Defaults to Claude Code's configured default.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "timeout".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: format!("Timeout in seconds for the SSH command. Default: {DEFAULT_TIMEOUT_SECS}, max: {MAX_TIMEOUT_SECS}."),
                default: Some(json!(DEFAULT_TIMEOUT_SECS)),
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "max_turns".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Maximum number of agentic turns Claude Code can take. Maps to --max-turns flag.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        Self {
            definition: ToolDefinition {
                name: "claude_code_remote".to_string(),
                description: "Execute a prompt on a remote machine running Claude Code CLI via SSH. \
                    Returns the structured JSON response from Claude Code including the result text, \
                    cost, and turn count. Requires Claude Code SSH keys to be configured in API Keys settings."
                    .to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["prompt".to_string()],
                },
                group: ToolGroup::Exec,
                hidden: false,
            },
        }
    }

    fn get_ssh_config(&self, context: &ToolContext) -> Result<SshConfig, String> {
        let host = context
            .get_api_key("CLAUDE_CODE_SSH_HOST")
            .filter(|k| !k.is_empty())
            .ok_or("CLAUDE_CODE_SSH_HOST not configured. Set it in Settings > API Keys > Claude Code.")?;

        let user = context
            .get_api_key("CLAUDE_CODE_SSH_USER")
            .filter(|k| !k.is_empty())
            .ok_or("CLAUDE_CODE_SSH_USER not configured. Set it in Settings > API Keys > Claude Code.")?;

        let key_value = context
            .get_api_key("CLAUDE_CODE_SSH_KEY")
            .filter(|k| !k.is_empty())
            .ok_or("CLAUDE_CODE_SSH_KEY not configured. Set it in Settings > API Keys > Claude Code.")?;

        let port = context
            .get_api_key("CLAUDE_CODE_SSH_PORT")
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| DEFAULT_SSH_PORT.to_string());

        // Detect if the value is an inline private key or a file path
        let is_inline_key = key_value.contains("-----BEGIN");

        Ok(SshConfig {
            host,
            user,
            key_value,
            is_inline_key,
            port,
        })
    }
}

struct SshConfig {
    host: String,
    user: String,
    /// Either the raw PEM key content or a filesystem path
    key_value: String,
    /// true if key_value contains inline key material, false if it's a path
    is_inline_key: bool,
    port: String,
}

/// RAII guard that writes an inline SSH key to a temp file and deletes it on drop
struct TempKeyFile {
    path: PathBuf,
}

impl TempKeyFile {
    fn write(key_content: &str) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("starkbot_ssh_{}", std::process::id()));
        // Ensure key ends with newline (SSH is picky about this)
        let content = if key_content.ends_with('\n') {
            key_content.to_string()
        } else {
            format!("{}\n", key_content)
        };
        std::fs::write(&path, &content)
            .map_err(|e| format!("Failed to write temp SSH key: {}", e))?;
        // SSH requires 600 permissions on key files
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("Failed to set permissions on temp SSH key: {}", e))?;
        }
        Ok(Self { path })
    }
}

impl Drop for TempKeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[async_trait]
impl Tool for ClaudeCodeRemoteTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::Standard
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: ClaudeCodeRemoteParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        if params.prompt.trim().is_empty() {
            return ToolResult::error("Prompt cannot be empty");
        }

        // Get SSH configuration from API keys
        let ssh = match self.get_ssh_config(context) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        let timeout_secs = params.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS).min(MAX_TIMEOUT_SECS);

        // Build the remote claude command
        let mut claude_cmd_parts: Vec<String> = vec![
            "claude".to_string(),
            "-p".to_string(),
            // The prompt will be passed via stdin to avoid shell escaping issues
            "--output-format".to_string(),
            "json".to_string(),
        ];

        if let Some(ref tools) = params.allowed_tools {
            for tool in tools {
                claude_cmd_parts.push("--allowedTools".to_string());
                claude_cmd_parts.push(tool.clone());
            }
        }

        if let Some(ref system_prompt) = params.append_system_prompt {
            claude_cmd_parts.push("--append-system-prompt".to_string());
            claude_cmd_parts.push(shell_escape(system_prompt));
        }

        if let Some(ref model) = params.model {
            claude_cmd_parts.push("--model".to_string());
            claude_cmd_parts.push(model.clone());
        }

        if let Some(max_turns) = params.max_turns {
            claude_cmd_parts.push("--max-turns".to_string());
            claude_cmd_parts.push(max_turns.to_string());
        }

        let claude_cmd = claude_cmd_parts.join(" ");

        // Build full remote command — cd to workdir then pipe prompt into claude
        let remote_cmd = if let Some(ref workdir) = params.workdir {
            format!(
                "cd {} && echo {} | {}",
                shell_escape(workdir),
                shell_escape(&params.prompt),
                claude_cmd
            )
        } else {
            format!(
                "echo {} | {}",
                shell_escape(&params.prompt),
                claude_cmd
            )
        };

        log::debug!(
            "claude_code_remote: ssh -p {} {}@{} (inline_key={}) '{}'",
            ssh.port, ssh.user, ssh.host, ssh.is_inline_key, remote_cmd
        );

        // If the key is inline content, write to a temp file (cleaned up on drop)
        let _temp_key = if ssh.is_inline_key {
            Some(match TempKeyFile::write(&ssh.key_value) {
                Ok(t) => t,
                Err(e) => return ToolResult::error(e),
            })
        } else {
            None
        };

        let key_path = if let Some(ref temp) = _temp_key {
            temp.path.to_string_lossy().to_string()
        } else {
            ssh.key_value.clone()
        };

        // Build SSH command
        let mut cmd = Command::new("ssh");
        cmd.args([
            "-o", "BatchMode=yes",
            "-o", "StrictHostKeyChecking=accept-new",
            "-o", "ConnectTimeout=10",
            "-p", &ssh.port,
            "-i", &key_path,
            &format!("{}@{}", ssh.user, ssh.host),
            &remote_cmd,
        ]);

        // Execute with timeout
        let result = match timeout(
            std::time::Duration::from_secs(timeout_secs),
            cmd.output(),
        )
        .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return ToolResult::error(format!("SSH command failed to execute: {}", e));
            }
            Err(_) => {
                return ToolResult::error(format!(
                    "SSH command timed out after {}s. Increase timeout or simplify the prompt.",
                    timeout_secs
                ));
            }
        };

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();

        if !result.status.success() {
            let exit_code = result.status.code().unwrap_or(-1);
            let error_detail = if !stderr.is_empty() {
                format!("SSH exit code {}: {}", exit_code, stderr.trim())
            } else if !stdout.is_empty() {
                format!("SSH exit code {}: {}", exit_code, stdout.trim())
            } else {
                format!("SSH exit code {} (no output)", exit_code)
            };
            return ToolResult::error(error_detail);
        }

        // Parse Claude Code JSON response
        match serde_json::from_str::<Value>(&stdout) {
            Ok(response) => {
                let is_error = response.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
                let result_text = response
                    .get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let cost_usd = response.get("cost_usd").and_then(|v| v.as_f64());
                let num_turns = response.get("num_turns").and_then(|v| v.as_u64());
                let duration_ms = response.get("duration_ms").and_then(|v| v.as_u64());
                let session_id = response.get("session_id").and_then(|v| v.as_str());

                let mut metadata = json!({
                    "source": "claude_code_remote",
                });
                if let Some(cost) = cost_usd {
                    metadata["cost_usd"] = json!(cost);
                }
                if let Some(turns) = num_turns {
                    metadata["num_turns"] = json!(turns);
                }
                if let Some(dur) = duration_ms {
                    metadata["duration_ms"] = json!(dur);
                }
                if let Some(sid) = session_id {
                    metadata["session_id"] = json!(sid);
                }
                if is_error {
                    metadata["is_error"] = json!(true);
                }

                if is_error {
                    log::warn!("claude_code_remote returned error: {}", result_text);
                    ToolResult::error(result_text).with_metadata(metadata)
                } else {
                    ToolResult::success(result_text).with_metadata(metadata)
                }
            }
            Err(e) => {
                // If we can't parse JSON, return raw output
                log::debug!("claude_code_remote: failed to parse JSON response: {}", e);
                if stdout.trim().is_empty() {
                    ToolResult::error(format!(
                        "Claude Code returned no output. stderr: {}",
                        stderr.trim()
                    ))
                } else {
                    ToolResult::success(stdout).with_metadata(json!({
                        "source": "claude_code_remote",
                        "raw_output": true,
                    }))
                }
            }
        }
    }
}

/// Shell-escape a string for safe embedding in a remote command
fn shell_escape(s: &str) -> String {
    // Use single-quote wrapping, escaping any existing single quotes
    format!("'{}'", s.replace('\'', "'\\''"))
}
