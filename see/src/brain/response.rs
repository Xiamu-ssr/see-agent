use serde::{Deserialize, Serialize};

/// A single tool call extracted from the LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Parsed LLM response.
#[derive(Debug, Clone)]
pub struct BrainResponse {
    /// Text content (can be None if only tool calls).
    pub content: Option<String>,
    /// Tool calls requested by the LLM.
    pub tool_calls: Vec<ToolCallInfo>,
    /// Raw API response message (for storing back in conversation history).
    pub raw: serde_json::Value,
}
