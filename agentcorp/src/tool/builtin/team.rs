use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::{Result, AgentCorpError};
use crate::supervisor::inbox::send_to_inbox_with_id;
use crate::team::task_board::TaskBoard;
use crate::tool::{Tool, ToolRegistry};
use crate::types::paths::TeamDir;
use crate::types::{Message, MessagePriority, TaskStatus, ToolResult};

use super::ToolContext;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_team_dir(ctx: &ToolContext) -> Result<&TeamDir> {
    ctx.team_dir.as_ref().ok_or_else(|| AgentCorpError::Tool {
        tool: "team".to_owned(),
        message: "agent is not part of a team".to_owned(),
    })
}

fn parse_priority(s: &str) -> MessagePriority {
    match s.to_lowercase().as_str() {
        "steer" => MessagePriority::Steer,
        _ => MessagePriority::Collect,
    }
}

// ---------------------------------------------------------------------------
// SendMessageTool
// ---------------------------------------------------------------------------

pub struct SendMessageTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        "send_message"
    }
    fn description(&self) -> &str {
        "Send a message to another agent's inbox"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "Target agent ID" },
                "content": { "type": "string", "description": "Message content" },
                "priority": {
                    "type": "string",
                    "enum": ["collect", "steer"],
                    "description": "Message priority (default: collect)"
                }
            },
            "required": ["to", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let to = args["to"].as_str().ok_or_else(|| AgentCorpError::Tool {
            tool: "send_message".to_owned(),
            message: "missing 'to' parameter".to_owned(),
        })?;
        let content = args["content"].as_str().ok_or_else(|| AgentCorpError::Tool {
            tool: "send_message".to_owned(),
            message: "missing 'content' parameter".to_owned(),
        })?;
        let priority = args["priority"]
            .as_str()
            .map(parse_priority)
            .unwrap_or(MessagePriority::Collect);

        let target_dir = self.ctx.workspace.agent(to);
        let inbox_path = target_dir.inbox();

        let message = Message {
            msg_id: None,
            sender: self.ctx.agent_id.clone(),
            content: content.to_owned(),
            priority,
            metadata: std::collections::HashMap::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        send_to_inbox_with_id(&inbox_path, message)?;

        Ok(ToolResult {
            text: format!("sent message to {to}"),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// ListTasksTool
// ---------------------------------------------------------------------------

pub struct ListTasksTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for ListTasksTool {
    fn name(&self) -> &str {
        "list_tasks"
    }
    fn description(&self) -> &str {
        "List tasks on the team task board"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["pending", "claimed", "in_progress", "done", "failed"],
                    "description": "Filter by status (optional)"
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let team_dir = require_team_dir(&self.ctx)?;
        let board = TaskBoard::new(team_dir.clone());

        let status_filter = args["status"].as_str().and_then(parse_task_status);
        let tasks = board.list_tasks(status_filter)?;

        if tasks.is_empty() {
            return Ok(ToolResult {
                text: "no tasks found".to_owned(),
                images: vec![],
            });
        }

        let text = tasks
            .iter()
            .map(|t| {
                format!(
                    "- [{}] {} (status: {:?}, assigned: {})\n  {}",
                    t.id,
                    t.title,
                    t.status,
                    t.assigned_to.as_deref().unwrap_or("none"),
                    t.description,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult {
            text,
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// CreateTaskTool
// ---------------------------------------------------------------------------

pub struct CreateTaskTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for CreateTaskTool {
    fn name(&self) -> &str {
        "create_task"
    }
    fn description(&self) -> &str {
        "Create a new task on the team task board"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Task title" },
                "description": { "type": "string", "description": "Task description" }
            },
            "required": ["title", "description"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let team_dir = require_team_dir(&self.ctx)?;
        let board = TaskBoard::new(team_dir.clone());

        let title = args["title"].as_str().unwrap_or("untitled");
        let description = args["description"].as_str().unwrap_or("");

        let task = board.create_task(title, description, &self.ctx.agent_id)?;

        Ok(ToolResult {
            text: format!("created task '{}' (id: {})", task.title, task.id),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// ClaimTaskTool
// ---------------------------------------------------------------------------

pub struct ClaimTaskTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for ClaimTaskTool {
    fn name(&self) -> &str {
        "claim_task"
    }
    fn description(&self) -> &str {
        "Claim an available task from the task board"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task ID to claim" }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let team_dir = require_team_dir(&self.ctx)?;
        let board = TaskBoard::new(team_dir.clone());

        let task_id = args["task_id"].as_str().ok_or_else(|| AgentCorpError::Tool {
            tool: "claim_task".to_owned(),
            message: "missing 'task_id' parameter".to_owned(),
        })?;

        let task = board.claim_task(task_id, &self.ctx.agent_id)?;

        Ok(ToolResult {
            text: format!("claimed task '{}' (id: {})", task.title, task.id),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// CompleteTaskTool
// ---------------------------------------------------------------------------

pub struct CompleteTaskTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for CompleteTaskTool {
    fn name(&self) -> &str {
        "complete_task"
    }
    fn description(&self) -> &str {
        "Mark a task as completed with a result"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task ID to complete" },
                "result": { "type": "string", "description": "Result description" }
            },
            "required": ["task_id", "result"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let team_dir = require_team_dir(&self.ctx)?;
        let board = TaskBoard::new(team_dir.clone());

        let task_id = args["task_id"].as_str().ok_or_else(|| AgentCorpError::Tool {
            tool: "complete_task".to_owned(),
            message: "missing 'task_id' parameter".to_owned(),
        })?;
        let result_text = args["result"].as_str().unwrap_or("done");

        let task = board.complete_task(task_id, &self.ctx.agent_id, result_text)?;

        Ok(ToolResult {
            text: format!("completed task '{}' (id: {})", task.title, task.id),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// UpdateTaskTool
// ---------------------------------------------------------------------------

pub struct UpdateTaskTool {
    ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for UpdateTaskTool {
    fn name(&self) -> &str {
        "update_task"
    }
    fn description(&self) -> &str {
        "Update fields on a task"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task ID to update" },
                "status": {
                    "type": "string",
                    "enum": ["pending", "claimed", "in_progress", "done", "failed"],
                    "description": "New status"
                },
                "assigned_to": { "type": "string", "description": "Agent to assign to" },
                "result": { "type": "string", "description": "Result text" }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let team_dir = require_team_dir(&self.ctx)?;
        let board = TaskBoard::new(team_dir.clone());

        let task_id = args["task_id"].as_str().ok_or_else(|| AgentCorpError::Tool {
            tool: "update_task".to_owned(),
            message: "missing 'task_id' parameter".to_owned(),
        })?;
        let status = args["status"].as_str().and_then(parse_task_status);
        let assigned_to = args["assigned_to"].as_str();
        let result_text = args["result"].as_str();

        let task = board.update_task(task_id, status, assigned_to, result_text)?;

        Ok(ToolResult {
            text: format!("updated task '{}' (id: {})", task.title, task.id),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// AssignTaskTool
// ---------------------------------------------------------------------------

pub struct AssignTaskTool {
    _ctx: Arc<ToolContext>,
}

#[async_trait]
impl Tool for AssignTaskTool {
    fn name(&self) -> &str {
        "assign_task"
    }
    fn description(&self) -> &str {
        "Assign a task to a specific agent"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task ID" },
                "agent_id": { "type": "string", "description": "Agent to assign to" }
            },
            "required": ["task_id", "agent_id"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let team_dir = require_team_dir(&self._ctx)?;
        let board = TaskBoard::new(team_dir.clone());

        let task_id = args["task_id"].as_str().ok_or_else(|| AgentCorpError::Tool {
            tool: "assign_task".to_owned(),
            message: "missing 'task_id' parameter".to_owned(),
        })?;
        let agent_id = args["agent_id"].as_str().ok_or_else(|| AgentCorpError::Tool {
            tool: "assign_task".to_owned(),
            message: "missing 'agent_id' parameter".to_owned(),
        })?;

        let task = board.assign_task(task_id, agent_id)?;

        Ok(ToolResult {
            text: format!("assigned task '{}' to {}", task.title, agent_id),
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_task_status(s: &str) -> Option<TaskStatus> {
    match s.to_lowercase().as_str() {
        "pending" => Some(TaskStatus::Pending),
        "claimed" => Some(TaskStatus::Claimed),
        "in_progress" => Some(TaskStatus::InProgress),
        "done" => Some(TaskStatus::Done),
        "failed" => Some(TaskStatus::Failed),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(registry: &mut ToolRegistry, ctx: &Arc<ToolContext>) {
    registry.register(Box::new(SendMessageTool { ctx: ctx.clone() }));
    registry.register(Box::new(ListTasksTool { ctx: ctx.clone() }));
    registry.register(Box::new(CreateTaskTool { ctx: ctx.clone() }));
    registry.register(Box::new(ClaimTaskTool { ctx: ctx.clone() }));
    registry.register(Box::new(CompleteTaskTool { ctx: ctx.clone() }));
    registry.register(Box::new(UpdateTaskTool { ctx: ctx.clone() }));
    registry.register(Box::new(AssignTaskTool { _ctx: ctx.clone() }));
}
