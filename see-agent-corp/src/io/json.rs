use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::Result;

/// Read and deserialize a JSON file.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let content = std::fs::read_to_string(path)?;
    let value = serde_json::from_str(&content)?;
    Ok(value)
}

/// Serialize and write a JSON file with pretty printing.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn roundtrip_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");

        let data = TestData {
            name: "hello".into(),
            value: 42,
        };
        write_json(&path, &data).unwrap();
        let loaded: TestData = read_json(&path).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn read_json_not_found() {
        let result: std::result::Result<TestData, _> = read_json(Path::new("/nonexistent.json"));
        assert!(result.is_err());
    }

    #[test]
    fn write_json_creates_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new.json");
        assert!(!path.exists());

        write_json(&path, &serde_json::json!({"a": 1})).unwrap();
        assert!(path.exists());
    }
}
