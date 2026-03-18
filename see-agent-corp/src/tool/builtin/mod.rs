mod control;
mod memory;
mod screen;
mod shell;
mod team;

use std::sync::Arc;

use crate::eye::Eye;
use crate::tool::ToolRegistry;
use crate::types::paths::{AgentDir, TeamDir, WorkspaceDir};

/// Shared context for all builtin tools.
pub struct ToolContext {
    pub agent_id: String,
    pub agent_dir: AgentDir,
    pub team_dir: Option<TeamDir>,
    pub eye: Arc<dyn Eye>,
    pub workspace: WorkspaceDir,
}

/// Register all 19 builtin tools with a shared ToolContext.
pub fn register_builtin_tools(registry: &mut ToolRegistry, ctx: Arc<ToolContext>) {
    shell::register(registry, &ctx);
    screen::register(registry, &ctx);
    memory::register(registry, &ctx);
    team::register(registry, &ctx);
    control::register(registry, &ctx);
}

/// Return (name, description) pairs for all builtin tools.
/// For API listing without needing a ToolContext.
pub fn builtin_tool_infos() -> Vec<(&'static str, &'static str)> {
    vec![
        ("click", "Click at screen coordinates"),
        ("type_text", "Type text using keyboard"),
        ("hotkey", "Press a keyboard shortcut"),
        ("scroll", "Scroll the screen"),
        ("drag", "Drag from one point to another"),
        ("screenshot", "Capture a screenshot"),
        ("shell", "Execute a shell command"),
        ("wait", "Wait for a specified duration"),
        ("finished", "Signal task completion"),
        ("call_user", "Request human intervention"),
        ("memory_search", "Search agent memory"),
        ("memory_write", "Write to agent memory"),
        ("send_message", "Send message to another agent"),
        ("list_tasks", "List team tasks"),
        ("create_task", "Create a team task"),
        ("claim_task", "Claim a team task"),
        ("complete_task", "Complete a team task"),
        ("update_task", "Update a team task"),
        ("assign_task", "Assign a team task"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eye::Screenshot;
    use async_trait::async_trait;
    use tempfile::TempDir;

    /// Mock Eye for testing — returns a 1x1 white pixel.
    struct MockEye;

    #[async_trait]
    impl Eye for MockEye {
        async fn capture(&self) -> crate::error::Result<Screenshot> {
            Ok(Screenshot {
                base64: "AAAA".to_owned(),
                width: 1,
                height: 1,
                scale_factor: 1.0,
                mime_type: "image/webp".to_owned(),
                screen_width: None,
                screen_height: None,
                image_bytes: None,
            })
        }
    }

    fn test_ctx(tmp: &TempDir) -> Arc<ToolContext> {
        let ws = WorkspaceDir::new(tmp.path());
        let agent_dir = ws.agent("test-agent");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        Arc::new(ToolContext {
            agent_id: "test-agent".to_owned(),
            agent_dir,
            team_dir: None,
            eye: Arc::new(MockEye),
            workspace: ws,
        })
    }

    fn test_ctx_with_team(tmp: &TempDir) -> Arc<ToolContext> {
        let ws = WorkspaceDir::new(tmp.path());
        let agent_dir = ws.agent("test-agent");
        let team_dir = ws.team("test-team");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::create_dir_all(team_dir.path()).unwrap();
        Arc::new(ToolContext {
            agent_id: "test-agent".to_owned(),
            agent_dir,
            team_dir: Some(team_dir),
            eye: Arc::new(MockEye),
            workspace: ws,
        })
    }

    #[test]
    fn registers_19_tools() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        assert_eq!(reg.len(), 19);
    }

    #[test]
    fn all_schemas_valid() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        let schemas = reg.get_schemas();
        assert_eq!(schemas.len(), 19);
        for s in &schemas {
            assert_eq!(s.schema_type, "function");
            assert!(!s.function.name.is_empty());
            assert!(!s.function.description.is_empty());
        }
    }

    #[test]
    fn filter_disabled_tools() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        let filtered = reg.get_schemas_filtered(&["shell".to_owned(), "click".to_owned()]);
        assert_eq!(filtered.len(), 17);
    }

    #[test]
    fn builtin_tool_infos_has_19() {
        let infos = builtin_tool_infos();
        assert_eq!(infos.len(), 19);
        for (name, desc) in &infos {
            assert!(!name.is_empty());
            assert!(!desc.is_empty());
        }
    }

    #[tokio::test]
    async fn shell_tool_executes_command() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        let result = reg
            .execute(
                "shell",
                serde_json::json!({"command": "echo hello-from-test"}),
            )
            .await
            .unwrap();
        assert!(result.text.contains("hello-from-test"));
    }

    #[tokio::test]
    async fn wait_tool_returns_quickly() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        let result = reg
            .execute("wait", serde_json::json!({"seconds": 0.01}))
            .await
            .unwrap();
        assert!(result.text.contains("0.01"));
    }

    #[tokio::test]
    async fn screenshot_tool_delegates_to_eye() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        let result = reg
            .execute("screenshot", serde_json::json!({}))
            .await
            .unwrap();
        assert!(result.text.contains("1x1"));
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].base64, "AAAA");
    }

    #[tokio::test]
    async fn memory_write_and_search() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        // Write
        let write_result = reg
            .execute(
                "memory_write",
                serde_json::json!({
                    "file": "MEMORY.md",
                    "content": "Safari crashes on retina display"
                }),
            )
            .await
            .unwrap();
        assert!(write_result.text.contains("written"));

        // Search
        let search_result = reg
            .execute(
                "memory_search",
                serde_json::json!({"query": "Safari crash"}),
            )
            .await
            .unwrap();
        assert!(search_result.text.contains("Safari"));
    }

    #[tokio::test]
    async fn team_create_task() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx_with_team(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute(
                "create_task",
                serde_json::json!({
                    "title": "Fix bug",
                    "description": "The login button is broken"
                }),
            )
            .await
            .unwrap();
        assert!(result.text.contains("Fix bug"));

        // Verify it appears in list
        let list_result = reg
            .execute("list_tasks", serde_json::json!({}))
            .await
            .unwrap();
        assert!(list_result.text.contains("Fix bug"));
    }

    #[tokio::test]
    async fn send_message_tool_writes_inbox() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        // Create target agent directory
        let target_dir = ctx.workspace.agent("target-agent");
        std::fs::create_dir_all(target_dir.path()).unwrap();

        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute(
                "send_message",
                serde_json::json!({
                    "to": "target-agent",
                    "content": "hello from test",
                    "priority": "collect"
                }),
            )
            .await
            .unwrap();
        assert!(result.text.contains("sent"));

        // Verify inbox file exists and contains the message
        let inbox_content = std::fs::read_to_string(target_dir.inbox()).unwrap();
        assert!(inbox_content.contains("hello from test"));
    }

    #[tokio::test]
    async fn finished_tool_returns_result() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        let result = reg
            .execute(
                "finished",
                serde_json::json!({"result": "task completed successfully"}),
            )
            .await
            .unwrap();
        assert!(result.text.contains("task completed successfully"));
    }
}
