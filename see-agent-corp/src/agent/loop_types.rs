use std::future::Future;
use std::pin::Pin;

/// Result of a completed agent run (Mode A).
#[derive(Debug, Clone)]
pub struct RunResult {
    pub summary: String,
    pub task_dir: String,
    pub total_steps: u32,
    pub elapsed_seconds: f64,
    pub success: bool,
    pub session_id: String,
}

/// Event emitted after each tool execution step.
#[derive(Debug, Clone)]
pub struct StepEvent {
    pub step: u32,
    pub max_steps: u32,
    pub thought: String,
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub tool_result: String,
    pub screenshot_path: Option<String>,
    pub wait_ms: u64,
    /// Screen-space coordinates (after scaling), None if no scaling applied.
    pub screen_tool_args: Option<serde_json::Value>,
}

/// Callback fired after each tool step.
pub type StepCallback =
    Box<dyn Fn(StepEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Callback for the `call_user` tool — asks the user a question, returns their reply.
pub type UserInputCallback =
    Box<dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;
