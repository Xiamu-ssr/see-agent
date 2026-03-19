use serde::{Deserialize, Serialize};

/// Result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub text: String,
    #[serde(default)]
    pub images: Vec<ToolResultImage>,
    /// Optional metadata (e.g. screen dimensions from screenshot tool).
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

impl ToolResult {
    /// Create a text-only result with no images or metadata.
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), images: vec![], metadata: serde_json::Value::Null }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultImage {
    pub base64: String,
    #[serde(default = "default_mime")]
    pub mime_type: String,
    #[serde(default = "default_detail")]
    pub detail: String,
}

fn default_mime() -> String {
    "image/webp".to_string()
}

fn default_detail() -> String {
    "high".to_string()
}

/// Tool call info parsed from LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// OpenAI function-calling tool schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    #[serde(rename = "type")]
    pub schema_type: String, // always "function"
    pub function: FunctionSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
