use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::io::{append_jsonl, read_jsonl};
use crate::types::Message;

// ---------------------------------------------------------------------------
// Cursor persistence (dual cursor: collect + steer)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct DualCursor {
    collect: usize,
    steer: usize,
}

/// Read the dual cursor values (0-based line offsets into inbox.jsonl).
/// Returns `None` if the cursor file doesn't exist.
/// Migrates old `{"line": N}` format to `{"collect": N, "steer": N}`.
pub fn read_cursors(path: &Path) -> Result<Option<(usize, usize)>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)?;
    let val: serde_json::Value = serde_json::from_str(text.trim())?;

    // New format: {"collect": N, "steer": M}
    if let (Some(c), Some(s)) = (
        val.get("collect").and_then(|v| v.as_u64()),
        val.get("steer").and_then(|v| v.as_u64()),
    ) {
        return Ok(Some((c as usize, s as usize)));
    }

    // Old format: {"line": N} → migrate
    if let Some(line) = val.get("line").and_then(|v| v.as_u64()) {
        let n = line as usize;
        write_cursors(path, n, n)?;
        return Ok(Some((n, n)));
    }

    Ok(Some((0, 0)))
}

/// Write the dual cursor values.
pub fn write_cursors(path: &Path, collect: usize, steer: usize) -> Result<()> {
    let cursor = DualCursor { collect, steer };
    let json = serde_json::to_string(&cursor)?;
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

/// Read all new messages from inbox.jsonl since the collect cursor, advance both cursors to end.
///
/// If the cursor file doesn't exist (old agent or never initialized),
/// skip all existing history and only process future messages.
pub fn drain_inbox(inbox_path: &Path, cursor_path: &Path) -> Result<Vec<Message>> {
    let all: Vec<Message> = read_jsonl(inbox_path)?;

    let collect = match read_cursors(cursor_path)? {
        Some((c, _)) => c,
        None => {
            // No cursor file — skip all history, create cursor at end
            write_cursors(cursor_path, all.len(), all.len())?;
            return Ok(Vec::new());
        }
    };

    if collect >= all.len() {
        return Ok(Vec::new());
    }

    let new_messages: Vec<Message> = all[collect..].to_vec();
    write_cursors(cursor_path, all.len(), all.len())?;
    Ok(new_messages)
}

/// Drain inbox and split by priority, respecting dual cursor.
///
/// Reads from collect cursor. Steer messages already consumed by
/// `drain_steer_only` (index < steer cursor) are NOT returned again.
/// Both cursors advance to the end.
pub fn drain_inbox_split(
    inbox_path: &Path,
    cursor_path: &Path,
) -> Result<(Vec<Message>, Vec<Message>)> {
    let all: Vec<Message> = read_jsonl(inbox_path)?;

    let (collect_cursor, steer_cursor) = match read_cursors(cursor_path)? {
        Some(c) => c,
        None => {
            write_cursors(cursor_path, all.len(), all.len())?;
            return Ok((Vec::new(), Vec::new()));
        }
    };

    if collect_cursor >= all.len() {
        // Ensure steer cursor is also at end
        if steer_cursor < all.len() {
            write_cursors(cursor_path, collect_cursor, all.len())?;
        }
        return Ok((Vec::new(), Vec::new()));
    }

    let mut steer = Vec::new();
    let mut collect = Vec::new();

    for (i, msg) in all.iter().enumerate().skip(collect_cursor) {
        if msg.is_steer() {
            // Only return steer messages not yet consumed by drain_steer_only
            if i >= steer_cursor {
                steer.push(msg.clone());
            }
        } else {
            collect.push(msg.clone());
        }
    }

    let end = all.len();
    write_cursors(cursor_path, end, end)?;
    Ok((steer, collect))
}

/// Drain only steer messages since the steer cursor, advance only the steer cursor.
///
/// This is called inside the reasoning loop (before each LLM call) to inject
/// steer messages in real-time without waiting for the outer drain_inbox_split.
pub fn drain_steer_only(
    inbox_path: &Path,
    cursor_path: &Path,
) -> Result<Vec<Message>> {
    let all: Vec<Message> = read_jsonl(inbox_path)?;

    let (collect_cursor, steer_cursor) = match read_cursors(cursor_path)? {
        Some(c) => c,
        None => {
            return Ok(Vec::new());
        }
    };

    if steer_cursor >= all.len() {
        return Ok(Vec::new());
    }

    let mut steer_msgs = Vec::new();
    let mut new_steer_cursor = steer_cursor;

    for (i, msg) in all.iter().enumerate().skip(steer_cursor) {
        if msg.is_steer() {
            steer_msgs.push(msg.clone());
        }
        new_steer_cursor = i + 1;
    }

    // Only advance steer cursor, keep collect cursor unchanged
    write_cursors(cursor_path, collect_cursor, new_steer_cursor)?;
    Ok(steer_msgs)
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

        assert_eq!(read_cursors(&path).unwrap(), None);
        write_cursors(&path, 0, 0).unwrap();
        assert_eq!(read_cursors(&path).unwrap(), Some((0, 0)));
        write_cursors(&path, 5, 7).unwrap();
        assert_eq!(read_cursors(&path).unwrap(), Some((5, 7)));
    }

    #[test]
    fn cursor_migrates_old_format() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cursor.json");

        // Write old format
        std::fs::write(&path, r#"{"line": 10}"#).unwrap();
        let cursors = read_cursors(&path).unwrap();
        assert_eq!(cursors, Some((10, 10)));

        // File should now be new format
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("collect"));
        assert!(text.contains("steer"));
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

        for i in 0..3 {
            send_to_inbox(&inbox, &make_msg(&format!("msg {i}"), MessagePriority::Collect))
                .unwrap();
        }

        let msgs = drain_inbox(&inbox, &cursor).unwrap();
        assert!(msgs.is_empty());
        assert_eq!(read_cursors(&cursor).unwrap(), Some((3, 3)));
    }

    #[test]
    fn drain_reads_new_messages() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");
        let cursor = tmp.path().join("cursor.json");

        write_cursors(&cursor, 0, 0).unwrap();

        for i in 0..3 {
            send_to_inbox(&inbox, &make_msg(&format!("msg {i}"), MessagePriority::Collect))
                .unwrap();
        }

        let msgs = drain_inbox(&inbox, &cursor).unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(read_cursors(&cursor).unwrap(), Some((3, 3)));

        let msgs = drain_inbox(&inbox, &cursor).unwrap();
        assert!(msgs.is_empty());

        send_to_inbox(&inbox, &make_msg("msg 3", MessagePriority::Steer)).unwrap();
        send_to_inbox(&inbox, &make_msg("msg 4", MessagePriority::Collect)).unwrap();

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

        write_cursors(&cursor, 0, 0).unwrap();

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
    fn drain_steer_only_returns_only_steer() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");
        let cursor = tmp.path().join("cursor.json");

        write_cursors(&cursor, 0, 0).unwrap();

        send_to_inbox(&inbox, &make_msg("a", MessagePriority::Collect)).unwrap();
        send_to_inbox(&inbox, &make_msg("b", MessagePriority::Steer)).unwrap();
        send_to_inbox(&inbox, &make_msg("c", MessagePriority::Collect)).unwrap();
        send_to_inbox(&inbox, &make_msg("d", MessagePriority::Steer)).unwrap();

        let steer = drain_steer_only(&inbox, &cursor).unwrap();
        assert_eq!(steer.len(), 2);
        assert_eq!(steer[0].content, "b");
        assert_eq!(steer[1].content, "d");

        // Steer cursor advanced to end, collect cursor unchanged
        let (c, s) = read_cursors(&cursor).unwrap().unwrap();
        assert_eq!(c, 0); // collect unchanged
        assert_eq!(s, 4); // steer at end
    }

    #[test]
    fn drain_split_skips_already_consumed_steer() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");
        let cursor = tmp.path().join("cursor.json");

        write_cursors(&cursor, 0, 0).unwrap();

        send_to_inbox(&inbox, &make_msg("a", MessagePriority::Collect)).unwrap();
        send_to_inbox(&inbox, &make_msg("b", MessagePriority::Steer)).unwrap();
        send_to_inbox(&inbox, &make_msg("c", MessagePriority::Collect)).unwrap();
        send_to_inbox(&inbox, &make_msg("d", MessagePriority::Steer)).unwrap();

        // First: drain_steer_only consumes all steer messages
        let steer = drain_steer_only(&inbox, &cursor).unwrap();
        assert_eq!(steer.len(), 2);

        // Now drain_inbox_split should NOT return those steer messages again
        let (steer, collect) = drain_inbox_split(&inbox, &cursor).unwrap();
        assert_eq!(steer.len(), 0, "steer already consumed by drain_steer_only");
        assert_eq!(collect.len(), 2);
        assert_eq!(collect[0].content, "a");
        assert_eq!(collect[1].content, "c");
    }

    #[test]
    fn drain_steer_only_after_partial() {
        let tmp = TempDir::new().unwrap();
        let inbox = tmp.path().join("inbox.jsonl");
        let cursor = tmp.path().join("cursor.json");

        write_cursors(&cursor, 0, 0).unwrap();

        // Add 2 messages
        send_to_inbox(&inbox, &make_msg("a", MessagePriority::Steer)).unwrap();
        send_to_inbox(&inbox, &make_msg("b", MessagePriority::Collect)).unwrap();

        // drain_steer_only
        let steer = drain_steer_only(&inbox, &cursor).unwrap();
        assert_eq!(steer.len(), 1);
        assert_eq!(steer[0].content, "a");

        // Add more messages
        send_to_inbox(&inbox, &make_msg("c", MessagePriority::Steer)).unwrap();

        // Second drain_steer_only only gets new steer
        let steer = drain_steer_only(&inbox, &cursor).unwrap();
        assert_eq!(steer.len(), 1);
        assert_eq!(steer[0].content, "c");
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
