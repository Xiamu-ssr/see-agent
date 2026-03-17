use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::error::Result;
use crate::tool::Tool;
use crate::types::ToolResult;

use super::client::McpClient;

// ---------------------------------------------------------------------------
// McpToolWrapper
// ---------------------------------------------------------------------------

/// Wraps a single tool discovered from an MCP server as a local `Tool`.
///
/// Tool name format: `mcp__{server_name}__{tool_name}`.
pub struct McpToolWrapper {
    /// Full namespaced name.
    tool_name: String,
    /// Original tool name on the MCP server.
    mcp_tool_name: String,
    description: String,
    input_schema: Value,
    client: Arc<Mutex<McpClient>>,
}

impl McpToolWrapper {
    pub fn new(
        server_name: &str,
        mcp_tool_name: &str,
        description: &str,
        input_schema: Value,
        client: Arc<Mutex<McpClient>>,
    ) -> Self {
        Self {
            tool_name: format!("mcp__{server_name}__{mcp_tool_name}"),
            mcp_tool_name: mcp_tool_name.to_owned(),
            description: description.to_owned(),
            input_schema,
            client,
        }
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let mut client = self.client.lock().await;
        let result = client.call_tool(&self.mcp_tool_name, args).await?;

        // Extract text from result.content
        let text = extract_text_from_content(&result);

        Ok(ToolResult {
            text,
            images: vec![],
        })
    }
}

/// Extract text from MCP tool call result.
///
/// The result typically contains a `content` array of items, each with
/// a `text` field or other content types.
fn extract_text_from_content(result: &Value) -> String {
    let Some(content) = result.get("content").and_then(|v| v.as_array()) else {
        // Fallback: if no content array, stringify the whole result
        return result.to_string();
    };

    let mut parts = Vec::new();
    for item in content {
        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
            parts.push(text.to_owned());
        } else {
            parts.push(item.to_string());
        }
    }

    parts.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_name_format() {
        let client = Arc::new(Mutex::new(McpClient::new(
            "srv".into(),
            "echo".into(),
            vec![],
            Default::default(),
            &Default::default(),
        )));
        let wrapper = McpToolWrapper::new(
            "myserver",
            "search",
            "Search things",
            json!({"type": "object"}),
            client,
        );
        assert_eq!(wrapper.name(), "mcp__myserver__search");
        assert_eq!(wrapper.description(), "Search things");
    }

    #[test]
    fn extract_text_from_content_array() {
        let result = json!({
            "content": [
                {"type": "text", "text": "Hello"},
                {"type": "text", "text": "World"},
            ]
        });
        assert_eq!(extract_text_from_content(&result), "Hello\nWorld");
    }

    #[test]
    fn extract_text_from_empty_content() {
        let result = json!({"content": []});
        assert_eq!(extract_text_from_content(&result), "");
    }

    #[test]
    fn extract_text_fallback_no_content() {
        let result = json!({"data": "raw"});
        assert_eq!(extract_text_from_content(&result), result.to_string());
    }

    #[test]
    fn schema_has_correct_name() {
        let client = Arc::new(Mutex::new(McpClient::new(
            "srv".into(),
            "echo".into(),
            vec![],
            Default::default(),
            &Default::default(),
        )));
        let wrapper = McpToolWrapper::new(
            "srv",
            "tool1",
            "A tool",
            json!({"type": "object", "properties": {"q": {"type": "string"}}}),
            client,
        );
        let schema = Tool::schema(&wrapper);
        assert_eq!(schema.function.name, "mcp__srv__tool1");
    }
}
