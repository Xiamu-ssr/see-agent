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
    /// Snapshot of the resolved config at session start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_snapshot: Option<serde_json::Value>,
}
