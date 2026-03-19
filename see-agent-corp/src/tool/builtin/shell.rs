use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::consts::{DEFAULT_WAIT_SECS, SHELL_OUTPUT_MAX_CHARS, SHELL_TIMEOUT_SECS};
use crate::error::Result;
use crate::tool::{Tool, ToolRegistry};
use crate::types::ToolResult;

use super::ToolContext;

// ---------------------------------------------------------------------------
// ShellTool
// ---------------------------------------------------------------------------

pub struct ShellTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout/stderr"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| crate::error::CorpError::Tool {
                tool: "shell".to_owned(),
                message: "missing 'command' parameter".to_owned(),
            })?;

        let timeout = Duration::from_secs(SHELL_TIMEOUT_SECS);

        let output = tokio::time::timeout(
            timeout,
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(self.ctx.agent_dir.path())
                .output(),
        )
        .await;

        let output = match output {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                return Ok(ToolResult::text(format!("shell error: {e}")));
            }
            Err(_) => {
                return Ok(ToolResult::text(format!("command timed out after {SHELL_TIMEOUT_SECS}s")));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);

        let mut text = format!("exit code: {code}\n");
        if !stdout.is_empty() {
            text.push_str(&format!("stdout:\n{stdout}\n"));
        }
        if !stderr.is_empty() {
            text.push_str(&format!("stderr:\n{stderr}\n"));
        }

        // Truncate with head+tail preservation to keep both beginning and end visible
        if text.len() > SHELL_OUTPUT_MAX_CHARS {
            let half = SHELL_OUTPUT_MAX_CHARS / 2;
            let head = &text[..half];
            let tail = &text[text.len() - half..];
            let omitted_lines = text[half..text.len() - half].lines().count();
            text = format!("{head}\n[... {omitted_lines} lines omitted ...]\n{tail}");
        }

        Ok(ToolResult::text(text))
    }
}

// ---------------------------------------------------------------------------
// WaitTool
// ---------------------------------------------------------------------------

pub struct WaitTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for WaitTool {
    fn name(&self) -> &str {
        "wait"
    }

    fn description(&self) -> &str {
        "Wait for a specified number of seconds"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "seconds": {
                    "type": "number",
                    "description": "Number of seconds to wait"
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let seconds = args["seconds"].as_f64().unwrap_or(DEFAULT_WAIT_SECS);
        tokio::time::sleep(Duration::from_secs_f64(seconds)).await;
        Ok(ToolResult::text(format!("waited {seconds}s")))
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(registry: &mut ToolRegistry, ctx: &Arc<ToolContext>) {
    registry.register_in_group("core", Box::new(ShellTool { ctx: ctx.clone() }));
    registry.register_in_group("core", Box::new(WaitTool { _ctx: ctx.clone() }));
}
