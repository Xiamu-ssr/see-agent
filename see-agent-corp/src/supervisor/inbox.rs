use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::io::{append_jsonl, read_jsonl};
use crate::types::Message;

// ---------------------------------------------------------------------------
// Cursor persistence
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    line: usize,
}

/// Read the cursor value (0-based line offset into inbox.jsonl).
/// Returns `None` if the cursor file doesn't exist.
pub fn read_cursor(path: &Path) -> Result<Option<usize>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)?;
    let cursor: Cursor = serde_json::from_str(text.trim())?;
    Ok(Some(cursor.line))
}

/// Write the cursor value.
pub fn write_cursor(path: &Path, line: usize) -> Result<()> {
    let cursor = Cursor { line };
    let json = serde_json::to_string(&cursor)?;
    // Atomic-ish: write to temp then rename
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Drain
// ---------------------------------------------------------------------------

/// Read all new messages from inbox.jsonl since the cursor, advance the cursor.
///
/// If the cursor file doesn't exist (old agent or never initialized),
/// skip all existing history and only process future messages.
///
/// Returns an empty Vec if there are no new messages.
pub fn drain_inbox(inbox_path: &Path, cursor_path: &Path) -> Result<Vec<Message>> {
    let all: Vec<Message> = read_jsonl(inbox_path)?;

    let cursor = match read_cursor(cursor_path)? {
        Some(c) => c,
        None => {
            // No cursor file — skip all history, create cursor at end
            write_cursor(cursor_path, all.len())?;
            return Ok(Vec::new());
        }
    };

    if cursor >= all.len() {
        return Ok(Vec::new());
    }

    let new_messages: Vec<Message> = all[cursor..].to_vec();
    write_cursor(cursor_path, all.len())?;
    Ok(new_messages)
}

/// Drain inbox but only return steer-priority messages for immediate injection.
///
/// Returns `(steer, collect)` — steer messages are injected immediately,
/// collect messages are batched for the next LLM turn.
pub fn drain_inbox_split(
    inbox_path: &Path,
    cursor_path: &Path,
) -> Result<(Vec<Message>, Vec<Message>)> {
    let messages = drain_inbox(inbox_path, cursor_path)?;
    let mut steer = Vec::new();
    let mut collect = Vec::new();

    for msg in messages {
        if msg.is_steer() {
            steer.push(msg);
        } else {
            collect.push(msg);
        }
    }

    Ok((steer, collect))
}

/// Append a message to an agent's inbox file.
pub fn send_to_inbox(inbox_path: &Path, message: &Message) -> Result<()> {
    append_jsonl(inbox_path, message)
}

/// Assign monotonic IDs to a message before writing.
///
/// Reads the current inbox length to determine the next ID.
pub fn send_to_inbox_with_id(inbox_path: &Path, mut message: Message) -> Result<()> {
    let current: Vec<Message> = read_jsonl(inbox_path).unwrap_or_default();
    message.msg_id = Some(current.len() as u64);
    append_jsonl(inbox_path, &message)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MessagePriority;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_msg(content: &str, priority: MessagePriority) -> Message {
        Message {
            msg_id: None,
            sender: "tester".into(),
            content: content.into(),
            priority,
            metadata: HashMap::new(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn cursor_read_write() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cursor.json");

        assert_eq!(read_cursor(&path).unwrap(), None);
        write_cursor(&path, 0).unwrap();
        assert_eq!(read_cursor(&path).unwrap(), Some(0));
        write_cursor(&path, 5).unwrap();
        assert_eq!(read_cursor(&path).unwrap(), Some(5));
        write_cursor(&path, 10).unwrap();
        assert_eq!(read_cursor(&path).unwrap(), Some(10));
    }

    #[test]
    fn drain_empty_inbox() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");
        let cursor = tmp.path().join("cursor.json");

        let msgs = drain_inbox(&inbox, &cursor).unwrap();
        assert!(msgs.is_empty());
    }

    #[test]
    fn drain_no_cursor_skips_history() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");
        let cursor = tmp.path().join("cursor.json");

        // Write 3 messages before cursor exists
        for i in 0..3 {
            send_to_inbox(&inbox, &make_msg(&format!("msg {i}"), MessagePriority::Collect))
                .unwrap();
        }

        // No cursor file → skip all history
        let msgs = drain_inbox(&inbox, &cursor).unwrap();
        assert!(msgs.is_empty());
        // Cursor now created at end of history
        assert_eq!(read_cursor(&cursor).unwrap(), Some(3));
    }

    #[test]
    fn drain_reads_new_messages() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");
        let cursor = tmp.path().join("cursor.json");

        // Initialize cursor at 0 (like create_agent does)
        write_cursor(&cursor, 0).unwrap();

        // Write 3 messages
        for i in 0..3 {
            send_to_inbox(&inbox, &make_msg(&format!("msg {i}"), MessagePriority::Collect))
                .unwrap();
        }

        // First drain gets all 3
        let msgs = drain_inbox(&inbox, &cursor).unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(read_cursor(&cursor).unwrap(), Some(3));

        // Second drain gets nothing
        let msgs = drain_inbox(&inbox, &cursor).unwrap();
        assert!(msgs.is_empty());

        // Add 2 more
        send_to_inbox(&inbox, &make_msg("msg 3", MessagePriority::Steer)).unwrap();
        send_to_inbox(&inbox, &make_msg("msg 4", MessagePriority::Collect)).unwrap();

        // Third drain gets the 2 new ones
        let msgs = drain_inbox(&inbox, &cursor).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "msg 3");
        assert_eq!(msgs[1].content, "msg 4");
    }

    #[test]
    fn drain_split_separates_priorities() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");
        let cursor = tmp.path().join("cursor.json");

        // Initialize cursor at 0 (like create_agent does)
        write_cursor(&cursor, 0).unwrap();

        send_to_inbox(&inbox, &make_msg("a", MessagePriority::Collect)).unwrap();
        send_to_inbox(&inbox, &make_msg("b", MessagePriority::Steer)).unwrap();
        send_to_inbox(&inbox, &make_msg("c", MessagePriority::Collect)).unwrap();
        send_to_inbox(&inbox, &make_msg("d", MessagePriority::Steer)).unwrap();

        let (steer, collect) = drain_inbox_split(&inbox, &cursor).unwrap();
        assert_eq!(steer.len(), 2);
        assert_eq!(collect.len(), 2);
        assert_eq!(steer[0].content, "b");
        assert_eq!(steer[1].content, "d");
        assert_eq!(collect[0].content, "a");
        assert_eq!(collect[1].content, "c");
    }

    #[test]
    fn send_with_id_assigns_monotonic_ids() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");

        send_to_inbox_with_id(&inbox, make_msg("first", MessagePriority::Collect)).unwrap();
        send_to_inbox_with_id(&inbox, make_msg("second", MessagePriority::Collect)).unwrap();

        let all: Vec<Message> = read_jsonl(&inbox).unwrap();
        assert_eq!(all[0].msg_id, Some(0));
        assert_eq!(all[1].msg_id, Some(1));
    }
}
