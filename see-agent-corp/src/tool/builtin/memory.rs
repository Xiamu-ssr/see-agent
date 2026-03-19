use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::consts::MEMORY_SEARCH_LIMIT;
use crate::error::Result;
use crate::memory::MarkdownMemory;
use crate::tool::{Tool, ToolRegistry};
use crate::types::ToolResult;

use super::ToolContext;

// ---------------------------------------------------------------------------
// MemorySearchTool
// ---------------------------------------------------------------------------

pub struct MemorySearchTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }
    fn description(&self) -> &str {
        "Search agent memory for relevant information"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let query = args["query"].as_str().unwrap_or("");
        let limit = args["limit"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(MEMORY_SEARCH_LIMIT);

        let mem = MarkdownMemory::new(self.ctx.agent_dir.memory_dir());
        let results = mem.search(query, limit)?;

        if results.is_empty() {
            return Ok(ToolResult::text("no results found"));
        }

        let mut text = format!("found {} results:\n\n", results.len());
        for (i, r) in results.iter().enumerate() {
            text.push_str(&format!(
                "{}. [{}] (score: {:.2})\n{}\n\n",
                i + 1,
                r.file,
                r.score,
                r.snippet
            ));
        }

        Ok(ToolResult::text(text))
    }
}

// ---------------------------------------------------------------------------
// MemoryGetTool
// ---------------------------------------------------------------------------

const DEFAULT_MEMORY_GET_LINES: usize = 20;

pub struct MemoryGetTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for MemoryGetTool {
    fn name(&self) -> &str {
        "memory_get"
    }
    fn description(&self) -> &str {
        "按路径和行号读取 memory 文件片段。配合 memory_search 使用：先搜索找到相关文件和行号，再用此工具精确读取需要的片段，节省上下文。"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Memory file path relative to memory/ directory (e.g. MEMORY.md or notes/plan.md)"
                },
                "from": {
                    "type": "integer",
                    "description": "Starting line number (1-based, default: 1)"
                },
                "lines": {
                    "type": "integer",
                    "description": "Number of lines to read (default: 20)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let path = args["path"].as_str().ok_or_else(|| crate::error::CorpError::Tool {
            tool: "memory_get".to_owned(),
            message: "missing 'path' parameter".to_owned(),
        })?;

        // Security: prevent path traversal
        if path.contains("..") {
            return Ok(ToolResult::text("error: path must not contain '..'"));
        }

        let from = args["from"].as_u64().unwrap_or(1).max(1) as usize;
        let lines = args["lines"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_MEMORY_GET_LINES);

        let file_path = self.ctx.agent_dir.memory_dir().join(path);
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => {
                return Ok(ToolResult::text(format!("file not found: {path}")));
            }
        };

        let all_lines: Vec<&str> = content.lines().collect();
        let total = all_lines.len();
        let start_idx = (from - 1).min(total);
        let end_idx = (start_idx + lines).min(total);
        let selected = &all_lines[start_idx..end_idx];

        let mut text = format!("--- {path} (lines {from}-{}, total {total}) ---\n", start_idx + selected.len());
        for (i, line) in selected.iter().enumerate() {
            text.push_str(&format!("{:4}| {}\n", start_idx + i + 1, line));
        }

        Ok(ToolResult::text(text))
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(registry: &mut ToolRegistry, ctx: &Arc<ToolContext>) {
    registry.register_in_group("memory", Box::new(MemorySearchTool { ctx: ctx.clone() }));
    registry.register_in_group("memory", Box::new(MemoryGetTool { ctx: ctx.clone() }));
}
