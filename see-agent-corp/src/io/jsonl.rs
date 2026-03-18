use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::Result;

/// Append a single record to a JSONL file.
///
/// Creates the file if it does not exist.
pub fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let line = serde_json::to_string(value)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// Read all records from a JSONL file.
///
/// Returns an empty Vec if the file does not exist.
/// Skips empty lines.
pub fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let mut items = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let item: T = serde_json::from_str(trimmed)?;
        items.push(item);
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Record {
        id: u32,
        text: String,
    }

    #[test]
    fn append_and_read_jsonl() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.jsonl");

        append_jsonl(
            &path,
            &Record {
                id: 1,
                text: "first".into(),
            },
        )
        .unwrap();
        append_jsonl(
            &path,
            &Record {
                id: 2,
                text: "second".into(),
            },
        )
        .unwrap();

        let records: Vec<Record> = read_jsonl(&path).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, 1);
        assert_eq!(records[1].text, "second");
    }

    #[test]
    fn read_nonexistent_returns_empty() {
        let records: Vec<Record> = read_jsonl(Path::new("/nonexistent.jsonl")).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn read_handles_empty_lines() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.jsonl");

        std::fs::write(
            &path,
            "{\"id\":1,\"text\":\"a\"}\n\n{\"id\":2,\"text\":\"b\"}\n\n",
        )
        .unwrap();

        let records: Vec<Record> = read_jsonl(&path).unwrap();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn append_creates_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new.jsonl");
        assert!(!path.exists());

        append_jsonl(
            &path,
            &Record {
                id: 1,
                text: "hello".into(),
            },
        )
        .unwrap();
        assert!(path.exists());
    }
}
