use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::Result;
use crate::tool::{Tool, ToolRegistry};
use crate::types::ToolResult;

use super::ToolContext;

// ---------------------------------------------------------------------------
// FinishedTool — AgentLoop intercepts this to end the loop
// ---------------------------------------------------------------------------

pub struct FinishedTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for FinishedTool {
    fn name(&self) -> &str {
        "finished"
    }
    fn description(&self) -> &str {
        "Signal that the current task is complete"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "result": {
                    "type": "string",
                    "description": "Summary of what was accomplished"
                }
            },
            "required": ["result"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let result = args["result"].as_str().unwrap_or("task completed");
        Ok(ToolResult {
            text: format!("finished: {result}"),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// CallUserTool — AgentLoop intercepts this to pause and wait for human input
// ---------------------------------------------------------------------------

pub struct CallUserTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for CallUserTool {
    fn name(&self) -> &str {
        "call_user"
    }
    fn description(&self) -> &str {
        "Request human intervention or clarification"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Why human help is needed"
                }
            },
            "required": ["reason"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let reason = args["reason"].as_str().unwrap_or("help needed");
        Ok(ToolResult {
            text: format!("calling user: {reason}"),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(registry: &mut ToolRegistry, ctx: &Arc<ToolContext>) {
    registry.register(Box::new(FinishedTool { _ctx: ctx.clone() }));
    registry.register(Box::new(CallUserTool { _ctx: ctx.clone() }));
}
