use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SessionStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
}

// ---------------------------------------------------------------------------
// SessionMeta (session/meta.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    /// Human-readable task description for this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    pub status: SessionStatus,
    /// ISO-8601 timestamp.
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub total_steps: u32,
    #[serde(default)]
    pub elapsed_seconds: f64,
    #[serde(default)]
    pub summary: String,
    /// Snapshot of the resolved config at session start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_snapshot: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// SessionMessage — the 8 JSONL message types in messages.jsonl
// ---------------------------------------------------------------------------

/// A single entry in `session/messages.jsonl`.
///
/// Unlike the inbox `Message` type (inter-agent communication), this
/// represents conversation messages between the agent and LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub msg_id: u64,
    #[serde(rename = "ts")]
    pub timestamp: String,
    #[serde(rename = "type")]
    pub msg_type: SessionMessageType,
    /// Varies by type — flexible for forward compatibility.
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMessageType {
    System,
    UserTask,
    Assistant,
    ToolResult,
    Screenshot,
    UserReply,
    SystemHint,
    Compact,
    Error,
}
