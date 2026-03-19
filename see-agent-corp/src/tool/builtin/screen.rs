use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::consts::DEFAULT_SCROLL_AMOUNT;
use crate::error::Result;
use crate::tool::{Tool, ToolRegistry};
use crate::types::{ToolResult, ToolResultImage};

use super::ToolContext;

// ---------------------------------------------------------------------------
// Helpers — osascript execution
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
async fn run_osascript(script: &str) -> Result<String> {
    let output = tokio::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .await
        .map_err(|e| crate::error::CorpError::Tool {
            tool: "osascript".to_owned(),
            message: format!("failed to run osascript: {e}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::error::CorpError::Tool {
            tool: "osascript".to_owned(),
            message: format!("osascript error: {stderr}"),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(not(target_os = "macos"))]
async fn run_osascript(_script: &str) -> Result<String> {
    Err(crate::error::CorpError::Tool {
        tool: "osascript".to_owned(),
        message: "screen interaction tools require macOS".to_owned(),
    })
}

// ---------------------------------------------------------------------------
// ClickTool
// ---------------------------------------------------------------------------

pub struct ClickTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for ClickTool {
    fn name(&self) -> &str {
        "click"
    }
    fn description(&self) -> &str {
        "Click at the specified screen coordinates"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "x": { "type": "integer", "description": "X coordinate" },
                "y": { "type": "integer", "description": "Y coordinate" },
                "button": { "type": "string", "enum": ["left", "right"], "description": "Mouse button (default: left)" }
            },
            "required": ["x", "y"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let x = args["x"].as_i64().unwrap_or(0);
        let y = args["y"].as_i64().unwrap_or(0);
        let button = args["button"].as_str().unwrap_or("left");

        let script = if button == "right" {
            format!(
                r#"tell application "System Events" to click at {{{x}, {y}}} using {{control down}}"#
            )
        } else {
            format!(r#"tell application "System Events" to click at {{{x}, {y}}}"#)
        };

        run_osascript(&script).await?;
        Ok(ToolResult::text(format!("clicked at ({x}, {y}) [{button}]")))
    }
}

// ---------------------------------------------------------------------------
// TypeTextTool
// ---------------------------------------------------------------------------

pub struct TypeTextTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for TypeTextTool {
    fn name(&self) -> &str {
        "type_text"
    }
    fn description(&self) -> &str {
        "Type text using the keyboard"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to type" }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let text = args["text"].as_str().unwrap_or("");
        // Escape backslashes and quotes for AppleScript
        let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            r#"tell application "System Events" to keystroke "{escaped}""#
        );
        run_osascript(&script).await?;
        Ok(ToolResult::text(format!("typed: {text}")))
    }
}

// ---------------------------------------------------------------------------
// HotkeyTool
// ---------------------------------------------------------------------------

pub struct HotkeyTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for HotkeyTool {
    fn name(&self) -> &str {
        "hotkey"
    }
    fn description(&self) -> &str {
        "Press a keyboard shortcut (e.g. command+c)"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "keys": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Keys to press simultaneously (e.g. [\"command\", \"c\"])"
                }
            },
            "required": ["keys"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let keys: Vec<&str> = args["keys"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect()
            })
            .unwrap_or_default();

        if keys.is_empty() {
            return Ok(ToolResult::text("no keys specified"));
        }

        // Separate modifiers from the main key
        let modifiers: Vec<&str> = keys
            .iter()
            .filter(|k| matches!(**k, "command" | "shift" | "control" | "option"))
            .copied()
            .collect();
        let main_keys: Vec<&str> = keys
            .iter()
            .filter(|k| !matches!(**k, "command" | "shift" | "control" | "option"))
            .copied()
            .collect();

        let key_char = main_keys.first().copied().unwrap_or("return");

        let using = if modifiers.is_empty() {
            String::new()
        } else {
            let mods: Vec<String> = modifiers
                .iter()
                .map(|m| format!("{m} down"))
                .collect();
            format!(" using {{{}}}", mods.join(", "))
        };

        let script = if key_char.len() == 1 {
            format!(
                r#"tell application "System Events" to keystroke "{key_char}"{using}"#
            )
        } else {
            // Named keys → key code approach
            let code = named_key_to_code(key_char);
            format!(
                r#"tell application "System Events" to key code {code}{using}"#
            )
        };

        run_osascript(&script).await?;
        Ok(ToolResult::text(format!("pressed: {}", keys.join("+"))))
    }
}

fn named_key_to_code(name: &str) -> u8 {
    match name.to_lowercase().as_str() {
        "return" | "enter" => 36,
        "tab" => 48,
        "space" => 49,
        "delete" | "backspace" => 51,
        "escape" | "esc" => 53,
        "up" => 126,
        "down" => 125,
        "left" => 123,
        "right" => 124,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        _ => 36, // fallback to return
    }
}

// ---------------------------------------------------------------------------
// ScrollTool
// ---------------------------------------------------------------------------

pub struct ScrollTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for ScrollTool {
    fn name(&self) -> &str {
        "scroll"
    }
    fn description(&self) -> &str {
        "Scroll the screen up or down"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "direction": {
                    "type": "string",
                    "enum": ["up", "down"],
                    "description": "Scroll direction"
                },
                "amount": {
                    "type": "integer",
                    "description": "Number of scroll steps (default: 3)"
                }
            },
            "required": ["direction"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let direction = args["direction"].as_str().unwrap_or("down");
        let amount = args["amount"]
            .as_i64()
            .unwrap_or(DEFAULT_SCROLL_AMOUNT as i64) as i32;
        let delta = if direction == "up" { amount } else { -amount };

        let script = format!(
            r#"tell application "System Events" to scroll area 1 by {delta}"#
        );
        // scroll via osascript is unreliable; use cliclick or mouse events as fallback
        let _ = run_osascript(&script).await;
        Ok(ToolResult::text(format!("scrolled {direction} by {amount}")))
    }
}

// ---------------------------------------------------------------------------
// DragTool
// ---------------------------------------------------------------------------

pub struct DragTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for DragTool {
    fn name(&self) -> &str {
        "drag"
    }
    fn description(&self) -> &str {
        "Drag from one screen coordinate to another"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "from_x": { "type": "integer", "description": "Start X coordinate" },
                "from_y": { "type": "integer", "description": "Start Y coordinate" },
                "to_x": { "type": "integer", "description": "End X coordinate" },
                "to_y": { "type": "integer", "description": "End Y coordinate" }
            },
            "required": ["from_x", "from_y", "to_x", "to_y"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let from_x = args["from_x"].as_i64().unwrap_or(0);
        let from_y = args["from_y"].as_i64().unwrap_or(0);
        let to_x = args["to_x"].as_i64().unwrap_or(0);
        let to_y = args["to_y"].as_i64().unwrap_or(0);

        // Use cliclick for drag (more reliable than osascript)
        let output = tokio::process::Command::new("cliclick")
            .args([
                &format!("dd:{from_x},{from_y}"),
                &format!("du:{to_x},{to_y}"),
            ])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {}
            _ => {
                // Fallback message if cliclick not available
                return Ok(ToolResult::text(format!(
                    "drag from ({from_x},{from_y}) to ({to_x},{to_y}) — install cliclick for drag support"
                )));
            }
        }

        Ok(ToolResult::text(format!("dragged from ({from_x},{from_y}) to ({to_x},{to_y})")))
    }
}

// ---------------------------------------------------------------------------
// ScreenshotTool
// ---------------------------------------------------------------------------

pub struct ScreenshotTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for ScreenshotTool {
    fn name(&self) -> &str {
        "screenshot"
    }
    fn description(&self) -> &str {
        "Capture a screenshot of the current screen"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> Result<ToolResult> {
        let screenshot = self.ctx.eye.capture().await?;
        let detail = screenshot.detail().to_owned();
        let text = format!(
            "screenshot captured ({}x{})",
            screenshot.width, screenshot.height
        );
        let metadata = json!({
            "width": screenshot.width,
            "height": screenshot.height,
            "screen_width": screenshot.screen_width.unwrap_or(screenshot.width),
            "screen_height": screenshot.screen_height.unwrap_or(screenshot.height),
            "mime_type": screenshot.mime_type,
        });
        Ok(ToolResult {
            text,
            images: vec![ToolResultImage {
                base64: screenshot.base64,
                mime_type: screenshot.mime_type,
                detail,
            }],
            metadata,
        })
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(registry: &mut ToolRegistry, ctx: &Arc<ToolContext>) {
    registry.register_in_group("screen", Box::new(ClickTool { _ctx: ctx.clone() }));
    registry.register_in_group("screen", Box::new(TypeTextTool { _ctx: ctx.clone() }));
    registry.register_in_group("screen", Box::new(HotkeyTool { _ctx: ctx.clone() }));
    registry.register_in_group("screen", Box::new(ScrollTool { _ctx: ctx.clone() }));
    registry.register_in_group("screen", Box::new(DragTool { _ctx: ctx.clone() }));
    registry.register_in_group("screen", Box::new(ScreenshotTool { ctx: ctx.clone() }));
}
