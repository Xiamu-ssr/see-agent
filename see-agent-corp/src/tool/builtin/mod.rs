mod control;
mod memory;
mod read;
mod screen;
mod shell;
mod team;

use std::path::PathBuf;
use std::sync::Arc;

use crate::eye::Eye;
use crate::tool::ToolRegistry;
use crate::types::paths::{AgentDir, TeamDir, WorkspaceDir};

/// Callback to wake a target agent's worker process (e.g., via SIGUSR1).
pub type WakeFn = Arc<dyn Fn(&str) + Send + Sync>;

/// Shared context for all builtin tools.
pub struct ToolContext {
    pub agent_id: String,
    pub agent_dir: AgentDir,
    pub team_dir: Option<TeamDir>,
    pub eye: Arc<dyn Eye>,
    pub workspace: WorkspaceDir,
    pub shared_dir: Option<PathBuf>,
    /// Optional callback to wake another agent's worker after sending a message.
    pub wake_fn: Option<WakeFn>,
}

/// Core (non-team) tool info triples: (name, description, group).
pub fn core_tool_infos() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("click", "Click at screen coordinates", "screen"),
        ("type_text", "Type text using keyboard", "screen"),
        ("hotkey", "Press a keyboard shortcut", "screen"),
        ("scroll", "Scroll the screen", "screen"),
        ("drag", "Drag from one point to another", "screen"),
        ("screenshot", "Capture a screenshot", "screen"),
        ("shell", "Execute a shell command", "core"),
        ("wait", "Wait for a specified duration", "core"),
        ("read", "Read a file (text or image)", "core"),
        ("finished", "Signal task completion", "control"),
        ("call_user", "Request human intervention", "control"),
        ("memory_search", "Search agent memory", "memory"),
        ("memory_get", "Read memory file by path and line range", "memory"),
    ]
}

/// Team-only tool info triples: (name, description, group).
pub fn team_tool_infos() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("send_message", "Send message to another agent", "team"),
        ("list_tasks", "List team tasks", "team"),
        ("create_task", "Create a team task", "team"),
        ("claim_task", "Claim a team task", "team"),
        ("complete_task", "Complete a team task", "team"),
        ("update_task", "Update a team task", "team"),
        ("assign_task", "Assign a team task", "team"),
    ]
}

/// Register builtin tools. Team tools are only registered when ctx.team_dir is Some.
pub fn register_builtin_tools(registry: &mut ToolRegistry, ctx: Arc<ToolContext>) {
    shell::register(registry, &ctx);
    screen::register(registry, &ctx);
    memory::register(registry, &ctx);
    control::register(registry, &ctx);
    read::register(registry, &ctx);
    if ctx.team_dir.is_some() {
        team::register(registry, &ctx);
    }
}

/// Return (name, description, group) triples for all builtin tools (core + team).
/// For API listing without needing a ToolContext.
pub fn builtin_tool_infos() -> Vec<(&'static str, &'static str, &'static str)> {
    let mut all = core_tool_infos();
    all.extend(team_tool_infos());
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eye::Screenshot;
    use crate::io::write_json;
    use crate::types::{TeamDefinition, TeamMember, TeamStatus};
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
            shared_dir: None,
            wake_fn: None,
        })
    }

    fn test_ctx_with_team(tmp: &TempDir) -> Arc<ToolContext> {
        let ws = WorkspaceDir::new(tmp.path());
        let agent_dir = ws.agent("test-agent");
        let team_dir = ws.team("test-team");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::create_dir_all(team_dir.path()).unwrap();
        // Write team.json with test-agent as leader
        let def = TeamDefinition {
            id: "test-team".to_owned(),
            name: "Test Team".to_owned(),
            members: vec![
                TeamMember { id: "test-agent".to_owned(), role: "leader".to_owned(), endpoint: None },
                TeamMember { id: "worker-1".to_owned(), role: "dev".to_owned(), endpoint: None },
            ],
            leader: "test-agent".to_owned(),
            status: TeamStatus::Running,
            created_at: "2025-01-01T00:00:00Z".to_owned(),
            config: None,
        };
        write_json(&team_dir.team_json(), &def).unwrap();
        let shared = team_dir.shared();
        Arc::new(ToolContext {
            agent_id: "test-agent".to_owned(),
            agent_dir,
            team_dir: Some(team_dir),
            eye: Arc::new(MockEye),
            workspace: ws,
            shared_dir: Some(shared),
            wake_fn: None,
        })
    }

    /// Create a team context where the agent is NOT the leader.
    fn test_ctx_non_leader(tmp: &TempDir) -> Arc<ToolContext> {
        let ws = WorkspaceDir::new(tmp.path());
        let agent_dir = ws.agent("worker-bob");
        let team_dir = ws.team("test-team");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::create_dir_all(team_dir.path()).unwrap();
        let def = TeamDefinition {
            id: "test-team".to_owned(),
            name: "Test Team".to_owned(),
            members: vec![
                TeamMember { id: "alice".to_owned(), role: "leader".to_owned(), endpoint: None },
                TeamMember { id: "worker-bob".to_owned(), role: "dev".to_owned(), endpoint: None },
            ],
            leader: "alice".to_owned(),
            status: TeamStatus::Running,
            created_at: "2025-01-01T00:00:00Z".to_owned(),
            config: None,
        };
        write_json(&team_dir.team_json(), &def).unwrap();
        let shared = team_dir.shared();
        Arc::new(ToolContext {
            agent_id: "worker-bob".to_owned(),
            agent_dir,
            team_dir: Some(team_dir),
            eye: Arc::new(MockEye),
            workspace: ws,
            shared_dir: Some(shared),
            wake_fn: None,
        })
    }

    #[test]
    fn registers_13_core_tools_without_team() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        assert_eq!(reg.len(), 13);
    }

    #[test]
    fn registers_20_tools_with_team() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx_with_team(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        assert_eq!(reg.len(), 20);
    }

    #[test]
    fn all_schemas_valid() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx_with_team(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        let schemas = reg.get_schemas();
        assert_eq!(schemas.len(), 20);
        for s in &schemas {
            assert_eq!(s.schema_type, "function");
            assert!(!s.function.name.is_empty());
            assert!(!s.function.description.is_empty());
        }
    }

    #[test]
    fn filter_disabled_tools() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx_with_team(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);
        let filtered = reg.get_schemas_filtered(&["shell".to_owned(), "click".to_owned()]);
        assert_eq!(filtered.len(), 18);
    }

    #[test]
    fn builtin_tool_infos_has_20() {
        let infos = builtin_tool_infos();
        assert_eq!(infos.len(), 20);
        for (name, desc, group) in &infos {
            assert!(!name.is_empty());
            assert!(!desc.is_empty());
            assert!(!group.is_empty());
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
    async fn memory_get_reads_file_lines() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);

        // Write a memory file manually
        let mem_dir = ctx.agent_dir.memory_dir();
        std::fs::create_dir_all(&mem_dir).unwrap();
        std::fs::write(
            mem_dir.join("MEMORY.md"),
            "line1\nline2\nline3\nline4\nline5\n",
        )
        .unwrap();

        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        // Read lines 2-4
        let result = reg
            .execute(
                "memory_get",
                serde_json::json!({"path": "MEMORY.md", "from": 2, "lines": 3}),
            )
            .await
            .unwrap();
        assert!(result.text.contains("line2"));
        assert!(result.text.contains("line3"));
        assert!(result.text.contains("line4"));
        assert!(!result.text.contains("line1"));

        // Default read (from start)
        let result2 = reg
            .execute(
                "memory_get",
                serde_json::json!({"path": "MEMORY.md"}),
            )
            .await
            .unwrap();
        assert!(result2.text.contains("line1"));
        assert!(result2.text.contains("line5"));
    }

    #[tokio::test]
    async fn memory_get_rejects_path_traversal() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute(
                "memory_get",
                serde_json::json!({"path": "../../../etc/passwd"}),
            )
            .await
            .unwrap();
        assert!(result.text.contains("must not contain"));
    }

    #[tokio::test]
    async fn memory_get_missing_file() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute(
                "memory_get",
                serde_json::json!({"path": "nonexistent.md"}),
            )
            .await
            .unwrap();
        assert!(result.text.contains("file not found"));
    }

    #[tokio::test]
    async fn memory_search_and_get_workflow() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);

        // Write a memory file manually
        let mem_dir = ctx.agent_dir.memory_dir();
        std::fs::create_dir_all(&mem_dir).unwrap();
        std::fs::write(
            mem_dir.join("MEMORY.md"),
            "Safari crashes on retina display\nFixed by updating WebKit\n",
        )
        .unwrap();

        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        // Search
        let search_result = reg
            .execute(
                "memory_search",
                serde_json::json!({"query": "Safari crash"}),
            )
            .await
            .unwrap();
        assert!(search_result.text.contains("Safari"));

        // Then get the specific file
        let get_result = reg
            .execute(
                "memory_get",
                serde_json::json!({"path": "MEMORY.md", "from": 1, "lines": 2}),
            )
            .await
            .unwrap();
        assert!(get_result.text.contains("Safari"));
        assert!(get_result.text.contains("WebKit"));
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
        let ctx = test_ctx_with_team(&tmp);
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

    #[test]
    fn create_team_creates_shared_directory() {
        use crate::config::ensure_workspace;
        use crate::types::TeamMember;

        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();

        let team = crate::team::create_team(
            &ws,
            "SharedTest",
            vec![TeamMember {
                id: "a1".into(),
                role: "dev".into(),
                endpoint: None,
            }],
            None,
        )
        .unwrap();

        let team_dir = ws.team(&team.id);
        assert!(team_dir.shared().exists());
    }

    #[tokio::test]
    async fn read_tool_reads_text_file() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        // Write a text file in the agent dir
        let file_path = ctx.agent_dir.path().join("hello.txt");
        std::fs::write(&file_path, "Hello, World!").unwrap();

        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute("read", serde_json::json!({"path": "hello.txt"}))
            .await
            .unwrap();
        assert!(result.text.contains("Hello, World!"));
        assert!(result.images.is_empty());
    }

    #[tokio::test]
    async fn read_tool_reads_image_file() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let file_path = ctx.agent_dir.path().join("test.png");
        std::fs::write(&file_path, b"fake-png-bytes").unwrap();

        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute("read", serde_json::json!({"path": "test.png"}))
            .await
            .unwrap();
        assert!(result.text.contains("test.png"));
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].mime_type, "image/png");
    }

    #[tokio::test]
    async fn read_tool_file_not_found() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute("read", serde_json::json!({"path": "nope.txt"}))
            .await
            .unwrap();
        assert!(result.text.contains("file not found"));
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

    // -----------------------------------------------------------------------
    // 3A: Leader enforcement tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn create_task_non_leader_returns_error() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx_non_leader(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute(
                "create_task",
                serde_json::json!({
                    "title": "Should fail",
                    "description": "Non-leader cannot create"
                }),
            )
            .await;
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("only team leader can create_task"), "got: {msg}");
    }

    #[tokio::test]
    async fn assign_task_non_leader_returns_error() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx_non_leader(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute(
                "assign_task",
                serde_json::json!({
                    "task_id": "t1",
                    "agent_id": "worker-bob"
                }),
            )
            .await;
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("only team leader can assign_task"), "got: {msg}");
    }

    #[tokio::test]
    async fn create_task_leader_succeeds() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx_with_team(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let result = reg
            .execute(
                "create_task",
                serde_json::json!({
                    "title": "Leader task",
                    "description": "Should succeed"
                }),
            )
            .await
            .unwrap();
        assert!(result.text.contains("Leader task"));
    }

    // -----------------------------------------------------------------------
    // 3B: Conditional tool registration tests
    // -----------------------------------------------------------------------

    #[test]
    fn no_team_tools_when_no_team_dir() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx(&tmp); // team_dir = None
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let schemas = reg.get_schemas();
        let team_tool_names = ["create_task", "assign_task", "list_tasks", "claim_task",
                               "complete_task", "update_task", "send_message"];
        for name in &team_tool_names {
            assert!(
                !schemas.iter().any(|s| s.function.name == *name),
                "team tool '{name}' should not be registered without team_dir"
            );
        }
    }

    #[test]
    fn team_tools_present_when_team_dir_set() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_ctx_with_team(&tmp);
        let mut reg = ToolRegistry::new();
        register_builtin_tools(&mut reg, ctx);

        let schemas = reg.get_schemas();
        let team_tool_names = ["create_task", "assign_task", "list_tasks", "claim_task",
                               "complete_task", "update_task", "send_message"];
        for name in &team_tool_names {
            assert!(
                schemas.iter().any(|s| s.function.name == *name),
                "team tool '{name}' should be registered with team_dir"
            );
        }
    }
}
