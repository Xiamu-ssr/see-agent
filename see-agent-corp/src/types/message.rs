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

    /// Convention: a shutdown message is detected by either:
    /// - metadata containing `"shutdown": "true"` (new format), OR
    /// - content == "shutdown" (backward compat, old format)
    pub fn is_shutdown(&self) -> bool {
        self.metadata.get("shutdown").map(|v| v.as_str()) == Some("true")
            || self.content.trim().eq_ignore_ascii_case("shutdown")
    }

    /// Format as `[sender]` for prompt injection.
    pub fn format_prefix(&self) -> String {
        format!("[{}]", self.sender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(content: &str, metadata: HashMap<String, String>) -> Message {
        Message {
            msg_id: None,
            sender: "test".into(),
            content: content.into(),
            priority: MessagePriority::Collect,
            metadata,
            timestamp: "2025-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn is_shutdown_old_format() {
        let msg = make_msg("shutdown", HashMap::new());
        assert!(msg.is_shutdown());
    }

    #[test]
    fn is_shutdown_old_format_case_insensitive() {
        let msg = make_msg("SHUTDOWN", HashMap::new());
        assert!(msg.is_shutdown());
    }

    #[test]
    fn is_shutdown_old_format_trimmed() {
        let msg = make_msg("  shutdown  ", HashMap::new());
        assert!(msg.is_shutdown());
    }

    #[test]
    fn is_shutdown_new_format_metadata() {
        let mut meta = HashMap::new();
        meta.insert("shutdown".into(), "true".into());
        let msg = make_msg(
            "[system] 系统即将关闭。请立即完成当前工作，保存重要信息到记忆系统，为下次复苏做准备。",
            meta,
        );
        assert!(msg.is_shutdown());
    }

    #[test]
    fn is_shutdown_regular_message_not_shutdown() {
        let msg = make_msg("hello world", HashMap::new());
        assert!(!msg.is_shutdown());
    }

    #[test]
    fn is_shutdown_metadata_false_not_shutdown() {
        let mut meta = HashMap::new();
        meta.insert("shutdown".into(), "false".into());
        let msg = make_msg("hello", meta);
        assert!(!msg.is_shutdown());
    }
}
