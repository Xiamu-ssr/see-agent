use std::path::Path;

use crate::error::Result;

/// Write content to a file atomically using write-then-rename.
///
/// Writes to a temporary file in the same directory, then renames.
/// This ensures readers never see a half-written file.
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn atomic_write_creates_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.json");

        atomic_write(&path, b"hello world").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");
    }

    #[test]
    fn atomic_write_no_tmp_left() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.json");

        atomic_write(&path, b"content").unwrap();

        // No .tmp file should remain
        let tmp_path = path.with_extension("tmp");
        assert!(!tmp_path.exists());
    }

    #[test]
    fn atomic_write_overwrites() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.json");

        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
    }
}
