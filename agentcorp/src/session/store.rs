use chrono::Utc;

use crate::error::{Result, AgentCorpError};
use crate::io::{append_jsonl, read_jsonl, write_json};
use crate::types::paths::SessionDir;
use crate::types::{SessionMessage, SessionMessageType, SessionMeta, SessionStatus};

/// File-backed session store operating on a single agent's session directory.
pub struct SessionStore {
    dir: SessionDir,
    msg_counter: u64,
}

impl SessionStore {
    /// Open an existing session or prepare for creation.
    pub fn new(dir: SessionDir) -> Self {
        Self {
            dir,
            msg_counter: 0,
        }
    }

    /// Create a new session, writing meta.json and preparing messages.jsonl.
    pub fn create(
        &mut self,
        task: Option<&str>,
        config_snapshot: Option<serde_json::Value>,
    ) -> Result<SessionMeta> {
        let session_path = self.dir.path();
        std::fs::create_dir_all(session_path)?;
        std::fs::create_dir_all(self.dir.screenshots())?;

        let now = Utc::now().to_rfc3339();
        let meta = SessionMeta {
            id: session_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            task: task.map(|s| s.to_owned()),
            status: SessionStatus::Running,
            created_at: now.clone(),
            updated_at: now,
            total_steps: 0,
            elapsed_seconds: 0.0,
            summary: String::new(),
            config_snapshot,
        };

        write_json(&self.dir.meta(), &meta)?;

        // Reset messages.jsonl
        std::fs::write(self.dir.messages(), "")?;
        self.msg_counter = 0;

        Ok(meta)
    }

    /// Load session metadata from disk.
    pub fn load_meta(&self) -> Result<SessionMeta> {
        if !self.dir.meta().exists() {
            return Err(AgentCorpError::Session {
                message: "session meta.json not found".to_owned(),
            });
        }
        crate::io::read_json(&self.dir.meta())
    }

    /// Update session metadata.
    pub fn save_meta(&self, meta: &SessionMeta) -> Result<()> {
        write_json(&self.dir.meta(), meta)
    }

    /// Append a message to messages.jsonl with auto-incrementing msg_id.
    pub fn append_message(
        &mut self,
        msg_type: SessionMessageType,
        data: serde_json::Value,
    ) -> Result<SessionMessage> {
        self.msg_counter += 1;
        let msg = SessionMessage {
            msg_id: self.msg_counter,
            timestamp: Utc::now().to_rfc3339(),
            msg_type,
            data,
        };
        append_jsonl(&self.dir.messages(), &msg)?;
        Ok(msg)
    }

    /// Read all messages from messages.jsonl.
    pub fn read_messages(&mut self) -> Result<Vec<SessionMessage>> {
        let messages: Vec<SessionMessage> = read_jsonl(&self.dir.messages())?;
        // Sync counter to max msg_id
        if let Some(max_id) = messages.iter().map(|m| m.msg_id).max() {
            self.msg_counter = max_id;
        }
        Ok(messages)
    }

    /// Find the last compact marker and return (summary, first_kept_msg_id).
    ///
    /// Returns None if no compaction has occurred.
    pub fn find_last_compact(
        &mut self,
    ) -> Result<Option<(String, u64)>> {
        let messages = self.read_messages()?;

        for msg in messages.iter().rev() {
            if msg.msg_type == SessionMessageType::Compact {
                let summary = msg
                    .data
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let first_kept = msg
                    .data
                    .get("first_kept_msg_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                return Ok(Some((summary, first_kept)));
            }
        }

        Ok(None)
    }

    /// Read messages for restore, skipping those before the last compact marker.
    ///
    /// Returns (compact_summary, kept_messages).
    pub fn read_for_restore(
        &mut self,
    ) -> Result<(Option<String>, Vec<SessionMessage>)> {
        let messages = self.read_messages()?;

        // Find last compact
        let mut compact_summary: Option<String> = None;
        let mut first_kept_id: u64 = 0;

        for msg in messages.iter().rev() {
            if msg.msg_type == SessionMessageType::Compact {
                compact_summary = msg
                    .data
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());
                first_kept_id = msg
                    .data
                    .get("first_kept_msg_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                break;
            }
        }

        // Filter: skip old messages and compact entries
        let kept: Vec<SessionMessage> = messages
            .into_iter()
            .filter(|m| {
                m.msg_id >= first_kept_id
                    && m.msg_type != SessionMessageType::Compact
                    && m.msg_type != SessionMessageType::System
            })
            .collect();

        Ok((compact_summary, kept))
    }

    /// Get the current message counter value.
    pub fn msg_counter(&self) -> u64 {
        self.msg_counter
    }

    /// Get a reference to the session directory.
    pub fn dir(&self) -> &SessionDir {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ensure_workspace;
    use crate::types::paths::WorkspaceDir;
    use serde_json::json;
    use tempfile::TempDir;

    fn setup() -> (TempDir, SessionStore) {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();
        let agent_dir = ws.agent("test");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        let store = SessionStore::new(agent_dir.session());
        (tmp, store)
    }

    #[test]
    fn create_session() {
        let (_tmp, mut store) = setup();
        let meta = store.create(Some("Test task"), None).unwrap();
        assert_eq!(meta.status, SessionStatus::Running);
        assert_eq!(meta.task.as_deref(), Some("Test task"));
        assert!(store.dir().meta().exists());
    }

    #[test]
    fn append_and_read_messages() {
        let (_tmp, mut store) = setup();
        store.create(None, None).unwrap();

        store
            .append_message(SessionMessageType::System, json!({"content": "You are an AI"}))
            .unwrap();
        store
            .append_message(
                SessionMessageType::UserTask,
                json!({"text": "Open Safari", "screenshot": "step_001.webp"}),
            )
            .unwrap();
        store
            .append_message(
                SessionMessageType::Assistant,
                json!({"content": "I'll open Safari", "tool_calls": []}),
            )
            .unwrap();

        let messages = store.read_messages().unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].msg_type, SessionMessageType::System);
        assert_eq!(messages[1].msg_type, SessionMessageType::UserTask);
        assert_eq!(messages[2].msg_type, SessionMessageType::Assistant);
        assert_eq!(store.msg_counter(), 3);
    }

    #[test]
    fn compact_and_restore() {
        let (_tmp, mut store) = setup();
        store.create(None, None).unwrap();

        // Add some messages
        store
            .append_message(SessionMessageType::System, json!({"content": "sys"}))
            .unwrap();
        store
            .append_message(SessionMessageType::UserTask, json!({"text": "old task"}))
            .unwrap();
        store
            .append_message(SessionMessageType::Assistant, json!({"content": "old reply"}))
            .unwrap();
        store
            .append_message(SessionMessageType::UserTask, json!({"text": "new task"}))
            .unwrap();
        store
            .append_message(SessionMessageType::Assistant, json!({"content": "new reply"}))
            .unwrap();

        // Add compact marker — keep messages from id 4 onward
        store
            .append_message(
                SessionMessageType::Compact,
                json!({"summary": "Earlier we discussed old stuff", "first_kept_msg_id": 4}),
            )
            .unwrap();

        // Restore should skip old messages
        let (summary, kept) = store.read_for_restore().unwrap();
        assert_eq!(summary.as_deref(), Some("Earlier we discussed old stuff"));
        // Should keep msg_id 4 and 5 (new task + new reply), skip system/compact/old
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].msg_type, SessionMessageType::UserTask);
        assert_eq!(kept[1].msg_type, SessionMessageType::Assistant);
    }

    #[test]
    fn find_last_compact_none() {
        let (_tmp, mut store) = setup();
        store.create(None, None).unwrap();
        store
            .append_message(SessionMessageType::System, json!({"content": "sys"}))
            .unwrap();

        let result = store.find_last_compact().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn meta_roundtrip() {
        let (_tmp, mut store) = setup();
        let mut meta = store.create(Some("task"), None).unwrap();

        meta.total_steps = 10;
        meta.status = SessionStatus::Completed;
        store.save_meta(&meta).unwrap();

        let loaded = store.load_meta().unwrap();
        assert_eq!(loaded.total_steps, 10);
        assert_eq!(loaded.status, SessionStatus::Completed);
    }
}
