use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// TaskStatus (for team tasklist.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Claimed,
    InProgress,
    Done,
    Failed,
}

// ---------------------------------------------------------------------------
// TaskItem
// ---------------------------------------------------------------------------

/// A single entry in teams/{team_id}/tasklist.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: TaskStatus,
    /// Agent id that claimed / is working on this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    /// Task ids that must complete before this one can start.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Outcome or output once finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Agent id that created this task.
    pub created_by: String,
    /// ISO-8601 timestamp.
    pub created_at: String,
    /// ISO-8601 timestamp.
    pub updated_at: String,
}
