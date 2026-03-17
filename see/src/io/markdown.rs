use std::path::Path;

use crate::error::Result;

/// Read a text/markdown file. Returns empty string if file does not exist.
pub fn read_text(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(content)
}

/// Write text content to a file. Creates parent directories if needed.
pub fn write_text(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_nonexistent_returns_empty() {
        let result = read_text(Path::new("/nonexistent/MEMORY.md")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn roundtrip_text() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("note.md");

        write_text(&path, "# Hello\nWorld").unwrap();
        let content = read_text(&path).unwrap();
        assert_eq!(content, "# Hello\nWorld");
    }

    #[test]
    fn write_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("deep").join("nested").join("file.md");

        write_text(&path, "content").unwrap();
        assert!(path.exists());
    }
}
