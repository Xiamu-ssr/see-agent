use async_trait::async_trait;
use serde_json::json;

use crate::error::Result;
use crate::types::ToolResult;

use super::registry::{Tool, ToolRegistry};

// ---------------------------------------------------------------------------
// Macro for simple tool definition
// ---------------------------------------------------------------------------

macro_rules! define_tool {
    ($struct_name:ident, $name:expr, $desc:expr, $params:expr) => {
        pub struct $struct_name;

        #[async_trait]
        impl Tool for $struct_name {
            fn name(&self) -> &str {
                $name
            }
            fn description(&self) -> &str {
                $desc
            }
            fn parameters(&self) -> serde_json::Value {
                $params
            }
            async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
                // Stub — actual implementation injected by agent runtime
                Ok(ToolResult {
                    text: format!("[{}] stub: {:?}", $name, args),
                    images: vec![],
                })
            }
        }
    };
}

// ---------------------------------------------------------------------------
// GUI Input Tools (stubs — real impl depends on Phase 5 screen capture)
// ---------------------------------------------------------------------------

define_tool!(
    ClickTool,
    "click",
    "点击屏幕上的指定坐标。坐标是逻辑像素，左上角为 (0,0)。",
    json!({
        "type": "object",
        "properties": {
            "x": {"type": "integer", "description": "横坐标（逻辑像素）"},
            "y": {"type": "integer", "description": "纵坐标（逻辑像素）"},
            "button": {"type": "string", "enum": ["left", "right", "middle"], "default": "left"},
            "double": {"type": "boolean", "default": false}
        },
        "required": ["x", "y"]
    })
);

define_tool!(
    TypeTextTool,
    "type_text",
    "在当前焦点位置输入文字。中文通过剪贴板粘贴实现。如需按回车提交，在 text 末尾加 \\n。",
    json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "要输入的文字"}
        },
        "required": ["text"]
    })
);

define_tool!(
    HotkeyTool,
    "hotkey",
    "按下快捷键组合。例如 ['command','c'] 表示 Cmd+C。",
    json!({
        "type": "object",
        "properties": {
            "keys": {
                "type": "array",
                "items": {"type": "string"},
                "description": "按键列表: command, ctrl, alt, shift, return, escape, tab, space, delete 等"
            }
        },
        "required": ["keys"]
    })
);

define_tool!(
    ScrollTool,
    "scroll",
    "在指定位置滚动。",
    json!({
        "type": "object",
        "properties": {
            "x": {"type": "integer", "description": "滚动位置横坐标"},
            "y": {"type": "integer", "description": "纵坐标"},
            "direction": {"type": "string", "enum": ["up", "down", "left", "right"]},
            "amount": {"type": "integer", "default": 3, "description": "滚动格数"}
        },
        "required": ["x", "y", "direction"]
    })
);

define_tool!(
    DragTool,
    "drag",
    "从一个坐标拖拽到另一个坐标。",
    json!({
        "type": "object",
        "properties": {
            "start_x": {"type": "integer"},
            "start_y": {"type": "integer"},
            "end_x": {"type": "integer"},
            "end_y": {"type": "integer"}
        },
        "required": ["start_x", "start_y", "end_x", "end_y"]
    })
);

define_tool!(
    ScreenshotTool,
    "screenshot",
    "截取当前屏幕截图，用于观察当前界面状态。在不确定当前状态时使用。",
    json!({
        "type": "object",
        "properties": {},
        "required": []
    })
);

// ---------------------------------------------------------------------------
// System Tools
// ---------------------------------------------------------------------------

define_tool!(
    ShellTool,
    "shell",
    "执行终端命令。打开应用优先用 shell('open -a AppName')，比视觉找图标更快更准。",
    json!({
        "type": "object",
        "properties": {
            "command": {"type": "string", "description": "要执行的 shell 命令"}
        },
        "required": ["command"]
    })
);

define_tool!(
    WaitTool,
    "wait",
    "等待指定秒数，用于等待页面加载或动画完成。",
    json!({
        "type": "object",
        "properties": {
            "seconds": {"type": "number", "default": 2}
        }
    })
);

// ---------------------------------------------------------------------------
// Control Flow Tools
// ---------------------------------------------------------------------------

define_tool!(
    FinishedTool,
    "finished",
    "任务完成。必须调用此工具表示任务结束。",
    json!({
        "type": "object",
        "properties": {
            "summary": {"type": "string", "description": "任务完成的总结"}
        },
        "required": ["summary"]
    })
);

define_tool!(
    CallUserTool,
    "call_user",
    "遇到无法解决的问题（需要密码、验证码等），请求用户帮助。",
    json!({
        "type": "object",
        "properties": {
            "question": {"type": "string", "description": "需要用户回答的问题"}
        },
        "required": ["question"]
    })
);

// ---------------------------------------------------------------------------
// Memory Tools
// ---------------------------------------------------------------------------

define_tool!(
    MemorySearchTool,
    "memory_search",
    "Search memory for relevant past experiences and knowledge.",
    json!({
        "type": "object",
        "properties": {
            "query": {"type": "string", "description": "Search query to find relevant memories."}
        },
        "required": ["query"]
    })
);

define_tool!(
    MemoryWriteTool,
    "memory_write",
    "Write important information to memory. Use MEMORY.md for persistent notes or YYYY-MM-DD.md for daily logs.",
    json!({
        "type": "object",
        "properties": {
            "file": {"type": "string", "description": "Target file: MEMORY.md or YYYY-MM-DD.md"},
            "content": {"type": "string", "description": "Markdown content to append."}
        },
        "required": ["file", "content"]
    })
);

// ---------------------------------------------------------------------------
// Team Communication Tool
// ---------------------------------------------------------------------------

define_tool!(
    SendMessageTool,
    "send_message",
    "Send a message to a teammate, 'owner', or '__all__' to broadcast.",
    json!({
        "type": "object",
        "properties": {
            "to": {"type": "string", "description": "Recipient agent ID, 'owner', or '__all__'."},
            "content": {"type": "string", "description": "Message content."}
        },
        "required": ["to", "content"]
    })
);

// ---------------------------------------------------------------------------
// Team Task Board Tools
// ---------------------------------------------------------------------------

define_tool!(
    ListTasksTool,
    "list_tasks",
    "List tasks on the team task board.",
    json!({
        "type": "object",
        "properties": {
            "status": {"type": "string", "description": "Filter by status (optional)."}
        }
    })
);

define_tool!(
    CreateTaskTool,
    "create_task",
    "Create a new task on the task board.",
    json!({
        "type": "object",
        "properties": {
            "title": {"type": "string", "description": "Task title."},
            "description": {"type": "string", "description": "Task description."}
        },
        "required": ["title"]
    })
);

define_tool!(
    ClaimTaskTool,
    "claim_task",
    "Claim a task from the task board.",
    json!({
        "type": "object",
        "properties": {
            "task_id": {"type": "string", "description": "ID of the task to claim."}
        },
        "required": ["task_id"]
    })
);

define_tool!(
    CompleteTaskTool,
    "complete_task",
    "Mark a task as done with a result.",
    json!({
        "type": "object",
        "properties": {
            "task_id": {"type": "string", "description": "ID of the task to complete."},
            "result": {"type": "string", "description": "Result or summary of work done."}
        },
        "required": ["task_id"]
    })
);

define_tool!(
    UpdateTaskTool,
    "update_task",
    "Update the status of a task.",
    json!({
        "type": "object",
        "properties": {
            "task_id": {"type": "string", "description": "ID of the task to update."},
            "status": {"type": "string", "description": "New status."}
        },
        "required": ["task_id", "status"]
    })
);

define_tool!(
    AssignTaskTool,
    "assign_task",
    "Assign a task to a specific agent.",
    json!({
        "type": "object",
        "properties": {
            "task_id": {"type": "string", "description": "ID of the task."},
            "agent_id": {"type": "string", "description": "Agent ID to assign to."}
        },
        "required": ["task_id", "agent_id"]
    })
);

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all built-in tools into a registry.
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    // GUI
    registry.register(Box::new(ClickTool));
    registry.register(Box::new(TypeTextTool));
    registry.register(Box::new(HotkeyTool));
    registry.register(Box::new(ScrollTool));
    registry.register(Box::new(DragTool));
    registry.register(Box::new(ScreenshotTool));
    // System
    registry.register(Box::new(ShellTool));
    registry.register(Box::new(WaitTool));
    // Control flow
    registry.register(Box::new(FinishedTool));
    registry.register(Box::new(CallUserTool));
    // Memory
    registry.register(Box::new(MemorySearchTool));
    registry.register(Box::new(MemoryWriteTool));
    // Team
    registry.register(Box::new(SendMessageTool));
    registry.register(Box::new(ListTasksTool));
    registry.register(Box::new(CreateTaskTool));
    registry.register(Box::new(ClaimTaskTool));
    registry.register(Box::new(CompleteTaskTool));
    registry.register(Box::new(UpdateTaskTool));
    registry.register(Box::new(AssignTaskTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_all_builtin_tools() {
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg);

        // 19 tools total
        assert_eq!(reg.len(), 19);

        // Verify all tools are accessible
        let names = reg.names();
        assert!(names.contains(&"click".to_owned()));
        assert!(names.contains(&"type_text".to_owned()));
        assert!(names.contains(&"hotkey".to_owned()));
        assert!(names.contains(&"scroll".to_owned()));
        assert!(names.contains(&"drag".to_owned()));
        assert!(names.contains(&"screenshot".to_owned()));
        assert!(names.contains(&"shell".to_owned()));
        assert!(names.contains(&"wait".to_owned()));
        assert!(names.contains(&"finished".to_owned()));
        assert!(names.contains(&"call_user".to_owned()));
        assert!(names.contains(&"memory_search".to_owned()));
        assert!(names.contains(&"memory_write".to_owned()));
        assert!(names.contains(&"send_message".to_owned()));
        assert!(names.contains(&"list_tasks".to_owned()));
        assert!(names.contains(&"create_task".to_owned()));
        assert!(names.contains(&"claim_task".to_owned()));
        assert!(names.contains(&"complete_task".to_owned()));
        assert!(names.contains(&"update_task".to_owned()));
        assert!(names.contains(&"assign_task".to_owned()));
    }

    #[test]
    fn all_schemas_valid() {
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg);

        let schemas = reg.get_schemas();
        assert_eq!(schemas.len(), 19);

        for schema in &schemas {
            assert_eq!(schema.schema_type, "function");
            assert!(!schema.function.name.is_empty());
            assert!(!schema.function.description.is_empty());
            assert!(schema.function.parameters.is_object());
        }
    }

    #[test]
    fn filter_disabled_tools() {
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg);

        let disabled = vec!["shell".to_owned(), "drag".to_owned()];
        let schemas = reg.get_schemas_filtered(&disabled);
        assert_eq!(schemas.len(), 17);

        let names: Vec<&str> = schemas.iter().map(|s| s.function.name.as_str()).collect();
        assert!(!names.contains(&"shell"));
        assert!(!names.contains(&"drag"));
        assert!(names.contains(&"click"));
    }

    #[tokio::test]
    async fn execute_stub_tool() {
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg);

        let result = reg
            .execute("finished", serde_json::json!({"summary": "done"}))
            .await
            .unwrap();
        assert!(result.text.contains("finished"));
    }
}
