use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::consts::READ_MAX_FILE_CHARS;
use crate::error::Result;
use crate::tool::{Tool, ToolRegistry};
use crate::types::ToolResult;

use super::ToolContext;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "svg"];

fn is_image_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

fn mime_from_ext(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tiff" => "image/tiff",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
    .to_owned()
}

// ---------------------------------------------------------------------------
// ReadTool
// ---------------------------------------------------------------------------

pub struct ReadTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read a file (text or image). Text files return content; images return base64 for vision."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to agent workspace"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let rel_path = args["path"]
            .as_str()
            .ok_or_else(|| crate::error::CorpError::Tool {
                tool: "read".to_owned(),
                message: "missing 'path' parameter".to_owned(),
            })?;

        let full_path = self.ctx.agent_dir.path().join(rel_path);

        if !full_path.exists() {
            return Ok(ToolResult {
                text: format!("file not found: {rel_path}"),
                images: vec![],
            });
        }

        if is_image_ext(&full_path) {
            read_image(&full_path)
        } else {
            read_text(&full_path)
        }
    }
}

fn read_text(path: &Path) -> Result<ToolResult> {
    let content = std::fs::read_to_string(path).map_err(|e| crate::error::CorpError::Tool {
        tool: "read".to_owned(),
        message: format!("cannot read file: {e}"),
    })?;

    let text = if content.len() > READ_MAX_FILE_CHARS {
        let half = READ_MAX_FILE_CHARS / 2;
        let head = &content[..half];
        let tail = &content[content.len() - half..];
        let omitted = content[half..content.len() - half].lines().count();
        format!("{head}\n[... {omitted} lines omitted ...]\n{tail}")
    } else {
        content
    };

    Ok(ToolResult {
        text,
        images: vec![],
    })
}

fn read_image(path: &Path) -> Result<ToolResult> {
    let bytes = std::fs::read(path).map_err(|e| crate::error::CorpError::Tool {
        tool: "read".to_owned(),
        message: format!("cannot read image: {e}"),
    })?;

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let mime = mime_from_ext(path);
    let size_kb = bytes.len() / 1024;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");

    Ok(ToolResult {
        text: format!("image loaded: {filename} ({size_kb} KB, {mime})"),
        images: vec![crate::types::ToolResultImage {
            base64: b64,
            mime_type: mime,
            detail: "high".to_owned(),
        }],
    })
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(registry: &mut ToolRegistry, ctx: &Arc<ToolContext>) {
    registry.register(Box::new(ReadTool { ctx: ctx.clone() }));
}
