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
            return Ok(ToolResult {
                text: "no results found".to_owned(),
                images: vec![],
            });
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

        Ok(ToolResult {
            text,
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// MemoryWriteTool
// ---------------------------------------------------------------------------

pub struct MemoryWriteTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for MemoryWriteTool {
    fn name(&self) -> &str {
        "memory_write"
    }
    fn description(&self) -> &str {
        "Write content to agent memory"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Memory file name (MEMORY.md or YYYY-MM-DD.md)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write"
                }
            },
            "required": ["file", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let file = args["file"].as_str().unwrap_or("MEMORY.md");
        let content = args["content"]
            .as_str()
            .ok_or_else(|| crate::error::AgentCorpError::Tool {
                tool: "memory_write".to_owned(),
                message: "missing 'content' parameter".to_owned(),
            })?;

        let mem = MarkdownMemory::new(self.ctx.agent_dir.memory_dir());
        mem.write(file, content)?;

        Ok(ToolResult {
            text: format!("written to {file}"),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(registry: &mut ToolRegistry, ctx: &Arc<ToolContext>) {
    registry.register(Box::new(MemorySearchTool { ctx: ctx.clone() }));
    registry.register(Box::new(MemoryWriteTool { ctx: ctx.clone() }));
}
