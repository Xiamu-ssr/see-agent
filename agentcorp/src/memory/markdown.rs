use std::path::PathBuf;

use crate::error::Result;

use super::bm25::{bm25_search, SearchResult};

/// Markdown-file-backed memory with BM25 search.
///
/// Storage: `*.md` files in a memory directory.
/// Allowed patterns: `MEMORY.md`, `YYYY-MM-DD.md`.
pub struct MarkdownMemory {
    dir: PathBuf,
}

impl MarkdownMemory {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Search memory files for relevant paragraphs.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let docs = self.collect_paragraphs()?;
        Ok(bm25_search(&docs, query, limit))
    }

    /// Write content to a memory file (append with blank line separator).
    ///
    /// `file` must be "MEMORY.md" or "YYYY-MM-DD.md".
    pub fn write(&self, file: &str, content: &str) -> Result<()> {
        if !is_valid_memory_filename(file) {
            return Err(crate::error::AgentCorpError::Agent {
                message: format!("invalid memory filename: {file}"),
            });
        }

        std::fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(file);

        let existing = if path.exists() {
            std::fs::read_to_string(&path)?
        } else {
            String::new()
        };

        let separator = if existing.is_empty() || existing.ends_with("\n\n") {
            ""
        } else if existing.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };

        let new_content = format!("{existing}{separator}{content}\n");
        std::fs::write(&path, new_content)?;
        Ok(())
    }

    /// Read a specific memory file.
    pub fn read(&self, file: &str) -> Result<String> {
        let path = self.dir.join(file);
        if !path.exists() {
            return Ok(String::new());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(content)
    }

    /// Collect all paragraphs from all *.md files as (filename, paragraph) pairs.
    fn collect_paragraphs(&self) -> Result<Vec<(String, String)>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }

        let mut docs = Vec::new();
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
            .collect();
        entries.sort();

        for path in entries {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let content = std::fs::read_to_string(&path)?;
            for para in split_paragraphs(&content) {
                if !para.trim().is_empty() {
                    docs.push((filename.clone(), para));
                }
            }
        }

        Ok(docs)
    }
}

/// Split text into paragraphs by double newlines.
fn split_paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Validate memory filename: MEMORY.md or YYYY-MM-DD.md
fn is_valid_memory_filename(name: &str) -> bool {
    if name == "MEMORY.md" {
        return true;
    }
    // Match YYYY-MM-DD.md
    if name.len() == 13 && name.ends_with(".md") {
        let date_part = &name[..10];
        let parts: Vec<&str> = date_part.split('-').collect();
        return parts.len() == 3
            && parts[0].len() == 4
            && parts[1].len() == 2
            && parts[2].len() == 2
            && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, MarkdownMemory) {
        let tmp = TempDir::new().unwrap();
        let mem = MarkdownMemory::new(tmp.path().join("memory"));
        (tmp, mem)
    }

    #[test]
    fn write_and_read() {
        let (_tmp, mem) = setup();

        mem.write("MEMORY.md", "# Important\nSafari crashes on retina")
            .unwrap();
        let content = mem.read("MEMORY.md").unwrap();
        assert!(content.contains("Safari crashes"));
    }

    #[test]
    fn write_appends() {
        let (_tmp, mem) = setup();

        mem.write("MEMORY.md", "First entry").unwrap();
        mem.write("MEMORY.md", "Second entry").unwrap();

        let content = mem.read("MEMORY.md").unwrap();
        assert!(content.contains("First entry"));
        assert!(content.contains("Second entry"));
    }

    #[test]
    fn write_validates_filename() {
        let (_tmp, mem) = setup();

        assert!(mem.write("MEMORY.md", "ok").is_ok());
        assert!(mem.write("2024-03-15.md", "ok").is_ok());
        assert!(mem.write("evil.md", "nope").is_err());
        assert!(mem.write("../escape.md", "nope").is_err());
    }

    #[test]
    fn search_finds_relevant() {
        let (_tmp, mem) = setup();

        mem.write("MEMORY.md", "# Safari\nSafari 浏览器在 Retina 屏幕上崩溃")
            .unwrap();
        mem.write("MEMORY.md", "# Python\nPython 虚拟环境配置方法")
            .unwrap();

        let results = mem.search("浏览器崩溃", 5).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].snippet.contains("Safari") || results[0].snippet.contains("浏览"));
    }

    #[test]
    fn search_across_files() {
        let (_tmp, mem) = setup();

        mem.write("MEMORY.md", "Long-term memory about config")
            .unwrap();
        mem.write("2024-03-15.md", "Today fixed config loading bug")
            .unwrap();

        let results = mem.search("config", 5).unwrap();
        assert!(results.len() >= 2);
    }

    #[test]
    fn search_empty_returns_empty() {
        let (_tmp, mem) = setup();
        let results = mem.search("anything", 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn read_nonexistent_returns_empty() {
        let (_tmp, mem) = setup();
        let content = mem.read("MEMORY.md").unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn valid_filenames() {
        assert!(is_valid_memory_filename("MEMORY.md"));
        assert!(is_valid_memory_filename("2024-03-15.md"));
        assert!(is_valid_memory_filename("2026-01-01.md"));
        assert!(!is_valid_memory_filename("readme.md"));
        assert!(!is_valid_memory_filename("2024-3-5.md"));
        assert!(!is_valid_memory_filename("../MEMORY.md"));
    }

    #[test]
    fn paragraph_splitting() {
        let text = "First paragraph\n\nSecond paragraph\n\nThird paragraph";
        let paras = split_paragraphs(text);
        assert_eq!(paras.len(), 3);
        assert_eq!(paras[0], "First paragraph");
    }
}
