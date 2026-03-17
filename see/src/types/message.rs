use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// MessagePriority (MentalModel.md section 2.4)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessagePriority {
    /// Batched into the next loop iteration.
    Collect,
    /// Injected immediately after the current tool completes.
    Steer,
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// A single inbox message (one line in inbox.jsonl).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Monotonically increasing id. None only before persistence assigns one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<u64>,
    pub sender: String,
    pub content: String,
    pub priority: MessagePriority,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// ISO-8601 timestamp string.
    #[serde(rename = "ts")]
    pub timestamp: String,
}

impl Message {
    /// Returns true if the message should be injected immediately.
    pub fn is_steer(&self) -> bool {
        self.priority == MessagePriority::Steer
    }

    /// Convention: a message with content "shutdown" is a graceful stop signal.
    pub fn is_shutdown(&self) -> bool {
        self.content.trim().eq_ignore_ascii_case("shutdown")
    }

    /// Format as `[sender]` for prompt injection.
    pub fn format_prefix(&self) -> String {
        format!("[{}]", self.sender)
    }
}
