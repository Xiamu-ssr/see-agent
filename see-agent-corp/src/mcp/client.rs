use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tracing::debug;

use crate::error::{Result, CorpError};

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    #[allow(dead_code)]
    code: i64,
    message: String,
}

// ---------------------------------------------------------------------------
// MCP tool info (returned by tools/list)
// ---------------------------------------------------------------------------

/// A tool discovered from an MCP server.
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

// ---------------------------------------------------------------------------
// Environment variable expansion
// ---------------------------------------------------------------------------

/// Expand `${VAR}` patterns in a string using the provided env map.
/// Unknown variables are left as-is.
fn expand_env(s: &str, env: &HashMap<String, String>) -> String {
    let mut result = s.to_owned();
    // Find all ${VAR} patterns
    let mut start = 0;
    loop {
        let Some(open) = result[start..].find("${") else {
            break;
        };
        let open = start + open;
        let Some(close) = result[open..].find('}') else {
            break;
        };
        let close = open + close;
        let var_name = &result[open + 2..close];
        let from_env = std::env::var(var_name).ok();
        if let Some(value) = env.get(var_name).or(from_env.as_ref()) {
            result.replace_range(open..=close, value);
            start = open + value.len();
        } else {
            start = close + 1;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// McpClient
// ---------------------------------------------------------------------------

/// Client for a single MCP server using stdio transport.
///
/// Communicates via JSON-RPC 2.0 over the server's stdin/stdout.
pub struct McpClient {
    pub name: String,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    child: Option<Child>,
    next_id: AtomicU64,
}

impl McpClient {
    pub fn new(
        name: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        global_env: &HashMap<String, String>,
    ) -> Self {
        // Expand env vars in command, args, and env values
        let mut merged_env: HashMap<String, String> = std::env::vars().collect();
        merged_env.extend(global_env.clone());
        merged_env.extend(env.clone());

        let command = expand_env(&command, &merged_env);
        let args: Vec<String> = args.iter().map(|a| expand_env(a, &merged_env)).collect();
        let env: HashMap<String, String> = env
            .into_iter()
            .map(|(k, v)| (k, expand_env(&v, &merged_env)))
            .collect();

        Self {
            name,
            command,
            args,
            env,
            child: None,
            next_id: AtomicU64::new(1),
        }
    }

    /// Connect to the MCP server: spawn the process and run the initialize handshake.
    pub async fn connect(&mut self) -> Result<()> {
        debug!(server = %self.name, cmd = %self.command, "connecting to MCP server");

        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        // Merge environment
        for (k, v) in &self.env {
            cmd.env(k, v);
        }

        let child = cmd.spawn().map_err(|e| CorpError::Mcp {
            message: format!("failed to spawn MCP server '{}': {e}", self.name),
        })?;

        self.child = Some(child);

        // Initialize handshake
        let resp = self
            .send_request(
                "initialize",
                Some(json!({
                    "protocolVersion": crate::consts::MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "see-agent-corp",
                        "version": crate::consts::VERSION,
                    }
                })),
            )
            .await?;

        debug!(server = %self.name, ?resp, "MCP initialize response");

        // Send initialized notification (no id, no response expected)
        self.send_notification("notifications/initialized", None)
            .await?;

        Ok(())
    }

    /// Disconnect: kill the child process.
    pub async fn disconnect(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
    }

    /// Discover available tools from the server.
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolInfo>> {
        let resp = self.send_request("tools/list", None).await?;

        let tools = resp
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut result = Vec::new();
        for tool in tools {
            let name = tool
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let description = tool
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let input_schema = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));

            if !name.is_empty() {
                result.push(McpToolInfo {
                    name,
                    description,
                    input_schema,
                });
            }
        }

        debug!(server = %self.name, count = result.len(), "discovered MCP tools");
        Ok(result)
    }

    /// Call a tool on the MCP server.
    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        self.send_request(
            "tools/call",
            Some(json!({
                "name": name,
                "arguments": arguments,
            })),
        )
        .await
    }

    /// Check if the client is connected (child process exists).
    pub fn is_connected(&self) -> bool {
        self.child.is_some()
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    async fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let child = self.child.as_mut().ok_or_else(|| CorpError::Mcp {
            message: format!("MCP server '{}' not connected", self.name),
        })?;

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_owned(),
            params,
        };

        let mut request_bytes = serde_json::to_vec(&request)?;
        request_bytes.push(b'\n');

        // Write to stdin
        let stdin = child.stdin.as_mut().ok_or_else(|| CorpError::Mcp {
            message: "MCP server stdin not available".into(),
        })?;
        stdin.write_all(&request_bytes).await.map_err(|e| CorpError::Mcp {
            message: format!("failed to write to MCP server '{}': {e}", self.name),
        })?;
        stdin.flush().await.map_err(|e| CorpError::Mcp {
            message: format!("failed to flush MCP server '{}': {e}", self.name),
        })?;

        // Read response from stdout
        let stdout = child.stdout.as_mut().ok_or_else(|| CorpError::Mcp {
            message: "MCP server stdout not available".into(),
        })?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        // Read lines until we get a valid JSON-RPC response
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await.map_err(|e| CorpError::Mcp {
                message: format!("failed to read from MCP server '{}': {e}", self.name),
            })?;

            if bytes_read == 0 {
                return Err(CorpError::Mcp {
                    message: format!("MCP server '{}' closed stdout unexpectedly", self.name),
                });
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Try to parse as JSON-RPC response
            if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed) {
                if let Some(err) = resp.error {
                    return Err(CorpError::Mcp {
                        message: format!(
                            "MCP server '{}' error: {}",
                            self.name, err.message
                        ),
                    });
                }
                return Ok(resp.result.unwrap_or(Value::Null));
            }

            // Skip non-response lines (notifications, log output)
            debug!(server = %self.name, line = trimmed, "skipping non-response line");
        }
    }

    async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        let child = self.child.as_mut().ok_or_else(|| CorpError::Mcp {
            message: format!("MCP server '{}' not connected", self.name),
        })?;

        // Notifications have no id
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params.unwrap_or(Value::Null),
        });

        let mut bytes = serde_json::to_vec(&notification)?;
        bytes.push(b'\n');

        let stdin = child.stdin.as_mut().ok_or_else(|| CorpError::Mcp {
            message: "MCP server stdin not available".into(),
        })?;
        stdin.write_all(&bytes).await.map_err(|e| CorpError::Mcp {
            message: format!("failed to write notification to MCP server '{}': {e}", self.name),
        })?;
        stdin.flush().await.map_err(|e| CorpError::Mcp {
            message: format!("failed to flush MCP server '{}': {e}", self.name),
        })?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_env_known_var() {
        let mut env = HashMap::new();
        env.insert("FOO".into(), "bar".into());
        assert_eq!(expand_env("hello ${FOO} world", &env), "hello bar world");
    }

    #[test]
    fn expand_env_unknown_var() {
        let env = HashMap::new();
        // Unknown vars are left as-is
        let result = expand_env("hello ${UNKNOWN_VAR_12345} world", &env);
        assert_eq!(result, "hello ${UNKNOWN_VAR_12345} world");
    }

    #[test]
    fn expand_env_no_vars() {
        let env = HashMap::new();
        assert_eq!(expand_env("no vars here", &env), "no vars here");
    }

    #[test]
    fn expand_env_multiple_vars() {
        let mut env = HashMap::new();
        env.insert("A".into(), "1".into());
        env.insert("B".into(), "2".into());
        assert_eq!(expand_env("${A}+${B}", &env), "1+2");
    }

    #[test]
    fn mcp_tool_name_format() {
        // Convention: mcp__{server}__{tool}
        let name = format!("mcp__{}__{}", "server1", "tool_a");
        assert_eq!(name, "mcp__server1__tool_a");
    }

    #[test]
    fn client_not_connected_by_default() {
        let client = McpClient::new(
            "test".into(),
            "echo".into(),
            vec![],
            HashMap::new(),
            &HashMap::new(),
        );
        assert!(!client.is_connected());
    }
}
