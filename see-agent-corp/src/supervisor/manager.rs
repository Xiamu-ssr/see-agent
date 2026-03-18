use std::collections::HashMap;
use std::path::PathBuf;

use tracing::{info, warn};

use crate::error::{Result, CorpError};
use crate::types::{AgentState, Message, MessagePriority, WorkspaceDir};

use super::inbox::send_to_inbox_with_id;

// ---------------------------------------------------------------------------
// ProcessHandle — tracks a spawned worker process
// ---------------------------------------------------------------------------

struct ProcessHandle {
    pid: u32,
    child: tokio::process::Child,
}

// ---------------------------------------------------------------------------
// Supervisor
// ---------------------------------------------------------------------------

/// Manages worker processes: spawn, stop, and message delivery.
///
/// Each agent gets a dedicated worker process (`see-agent-corp worker <agent_id>`).
/// Communication is file-based: messages are appended to the agent's
/// `inbox.jsonl`, then SIGUSR1 wakes the worker to drain.
pub struct Supervisor {
    workspace: WorkspaceDir,
    processes: HashMap<String, ProcessHandle>,
    /// Path to the agentcorp binary. Defaults to the current executable.
    binary_path: PathBuf,
}

impl Supervisor {
    pub fn new(workspace: WorkspaceDir) -> Self {
        let binary_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("see-agent-corp"));
        Self {
            workspace,
            processes: HashMap::new(),
            binary_path,
        }
    }

    /// Override the path to the worker binary.
    pub fn set_binary_path(&mut self, path: PathBuf) {
        self.binary_path = path;
    }

    /// Start a worker process for the given agent.
    ///
    /// The worker is launched as a child process: `see-agent-corp worker <agent_id>`
    pub async fn start_agent(&mut self, agent_id: &str) -> Result<()> {
        if self.processes.contains_key(agent_id) {
            return Err(CorpError::Agent {
                message: format!("agent '{agent_id}' is already running"),
            });
        }

        let agent_dir = self.workspace.agent(agent_id);
        if !agent_dir.path().exists() {
            return Err(CorpError::NotFound {
                what: format!("agent directory: {}", agent_dir.path().display()),
            });
        }

        // Ensure inbox file exists
        let inbox_path = agent_dir.inbox();
        if !inbox_path.exists() {
            std::fs::write(&inbox_path, "")?;
        }

        // Spawn worker process
        let child = tokio::process::Command::new(&self.binary_path)
            .arg("worker")
            .arg(agent_id)
            .arg(self.workspace.path().to_string_lossy().as_ref())
            // Clean environment: strip conda/venv vars
            .env_remove("CONDA_PREFIX")
            .env_remove("CONDA_DEFAULT_ENV")
            .env_remove("VIRTUAL_ENV")
            .spawn()
            .map_err(|e| CorpError::Agent {
                message: format!("failed to spawn worker for '{agent_id}': {e}"),
            })?;

        let pid = child.id().unwrap_or(0);
        info!(agent = agent_id, pid, "worker process started");

        self.processes.insert(
            agent_id.to_owned(),
            ProcessHandle { pid, child },
        );

        Ok(())
    }

    /// Stop a running worker process.
    ///
    /// Sends a "shutdown" message first (graceful), then waits briefly,
    /// and kills if still alive.
    pub async fn stop_agent(&mut self, agent_id: &str) -> Result<()> {
        // Send shutdown message with metadata flag
        let agent_dir = self.workspace.agent(agent_id);
        let mut shutdown_metadata = HashMap::new();
        shutdown_metadata.insert("shutdown".into(), "true".into());
        let shutdown_msg = Message {
            msg_id: None,
            sender: "supervisor".into(),
            content: "[system] 系统即将关闭。请立即完成当前工作，保存重要信息到记忆系统，为下次复苏做准备。".into(),
            priority: MessagePriority::Steer,
            metadata: shutdown_metadata,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let _ = send_to_inbox_with_id(&agent_dir.inbox(), shutdown_msg);

        // Signal the worker to wake up
        if let Some(handle) = self.processes.get(agent_id) {
            signal_process(handle.pid);
        }

        // Wait for graceful exit (120s), then kill
        if let Some(mut handle) = self.processes.remove(agent_id) {
            let timeout =
                tokio::time::Duration::from_secs(crate::consts::WORKER_SHUTDOWN_TIMEOUT_SECS);

            match tokio::time::timeout(timeout, handle.child.wait()).await {
                Ok(Ok(status)) => {
                    info!(agent = agent_id, ?status, "worker exited gracefully");
                }
                _ => {
                    warn!(agent = agent_id, "worker did not exit in time, killing");
                    let _ = handle.child.kill().await;
                }
            }
        }

        Ok(())
    }

    /// Send a message to an agent's inbox, auto-starting the worker if needed.
    pub async fn send_to(&mut self, agent_id: &str, message: Message) -> Result<()> {
        let agent_dir = self.workspace.agent(agent_id);

        if !agent_dir.path().exists() {
            return Err(CorpError::NotFound {
                what: format!("agent directory: {}", agent_dir.path().display()),
            });
        }

        // Auto-start the worker if not running
        if !self.is_running(agent_id) {
            self.start_agent(agent_id).await?;
        }

        let inbox_path = agent_dir.inbox();
        send_to_inbox_with_id(&inbox_path, message)?;

        // Wake the worker
        if let Some(handle) = self.processes.get(agent_id) {
            signal_process(handle.pid);
        }

        Ok(())
    }

    /// Check if an agent's worker process is running.
    pub fn is_running(&self, agent_id: &str) -> bool {
        self.processes.contains_key(agent_id)
    }

    /// Get the lifecycle state of an agent.
    pub fn agent_state(&self, agent_id: &str) -> AgentState {
        if self.processes.contains_key(agent_id) {
            AgentState::Active
        } else {
            AgentState::Sleeping
        }
    }

    /// Get PIDs of all running workers.
    pub fn running_agents(&self) -> Vec<(String, u32)> {
        self.processes
            .iter()
            .map(|(id, h)| (id.clone(), h.pid))
            .collect()
    }

    /// Stop all running workers.
    pub async fn stop_all(&mut self) {
        let ids: Vec<String> = self.processes.keys().cloned().collect();
        for id in ids {
            if let Err(e) = self.stop_agent(&id).await {
                warn!(agent = %id, "failed to stop: {e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Signal helpers
// ---------------------------------------------------------------------------

/// Send SIGUSR1 to a process by PID.
#[cfg(unix)]
fn signal_process(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    if pid == 0 {
        return;
    }
    if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGUSR1) {
        warn!(pid, "failed to send SIGUSR1: {e}");
    }
}

/// Non-unix stub.
#[cfg(not(unix))]
fn signal_process(_pid: u32) {
    // No signal support on non-unix
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::read_jsonl;
    use tempfile::TempDir;

    fn make_workspace() -> (TempDir, WorkspaceDir) {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        (tmp, ws)
    }

    #[tokio::test]
    async fn send_to_creates_inbox_message() {
        let (_tmp, ws) = make_workspace();

        // Create agent directory
        let agent_dir = ws.agent("alice");
        std::fs::create_dir_all(agent_dir.path()).unwrap();

        let mut sup = Supervisor::new(ws);
        // Set binary to something that won't actually start (auto-start will
        // be attempted but the spawned process will fail/exit immediately;
        // the inbox write still succeeds).
        sup.set_binary_path(std::path::PathBuf::from("/usr/bin/true"));

        let msg = Message {
            msg_id: None,
            sender: "bob".into(),
            content: "hello alice".into(),
            priority: MessagePriority::Collect,
            metadata: Default::default(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        };
        sup.send_to("alice", msg).await.unwrap();

        let inbox: Vec<Message> = read_jsonl(&agent_dir.inbox()).unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].content, "hello alice");
        assert_eq!(inbox[0].msg_id, Some(0));
    }

    #[tokio::test]
    async fn send_to_nonexistent_agent_errors() {
        let (_tmp, ws) = make_workspace();
        let mut sup = Supervisor::new(ws);

        let msg = Message {
            msg_id: None,
            sender: "bob".into(),
            content: "hello".into(),
            priority: MessagePriority::Collect,
            metadata: Default::default(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        };
        let result = sup.send_to("nonexistent", msg).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_to_auto_starts_agent() {
        let (_tmp, ws) = make_workspace();

        // Create agent directory but do NOT start
        let agent_dir = ws.agent("eve");
        std::fs::create_dir_all(agent_dir.path()).unwrap();

        let mut sup = Supervisor::new(ws);
        sup.set_binary_path(std::path::PathBuf::from("/usr/bin/true"));

        assert!(!sup.is_running("eve"));

        let msg = Message {
            msg_id: None,
            sender: "user".into(),
            content: "wake up".into(),
            priority: MessagePriority::Steer,
            metadata: Default::default(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        };
        sup.send_to("eve", msg).await.unwrap();

        // Agent should now be registered as running (auto-started)
        assert!(sup.is_running("eve"));

        // Message should be in the inbox
        let inbox: Vec<Message> = read_jsonl(&agent_dir.inbox()).unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].content, "wake up");
    }

    #[tokio::test]
    async fn start_nonexistent_agent_errors() {
        let (_tmp, ws) = make_workspace();
        let mut sup = Supervisor::new(ws);

        let result = sup.start_agent("nonexistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn is_running_false_by_default() {
        let (_tmp, ws) = make_workspace();
        let sup = Supervisor::new(ws);
        assert!(!sup.is_running("any"));
    }

    #[test]
    fn running_agents_empty_initially() {
        let (_tmp, ws) = make_workspace();
        let sup = Supervisor::new(ws);
        assert!(sup.running_agents().is_empty());
    }

    #[test]
    fn agent_state_sleeping_by_default() {
        let (_tmp, ws) = make_workspace();
        let sup = Supervisor::new(ws);
        assert_eq!(sup.agent_state("any"), AgentState::Sleeping);
    }

    #[tokio::test]
    async fn stop_agent_sends_shutdown_with_metadata() {
        let (_tmp, ws) = make_workspace();

        let agent_dir = ws.agent("stopper");
        std::fs::create_dir_all(agent_dir.path()).unwrap();

        let mut sup = Supervisor::new(ws);
        sup.set_binary_path(std::path::PathBuf::from("/usr/bin/true"));

        // Start the agent so stop_agent has a process to stop
        sup.start_agent("stopper").await.unwrap();
        sup.stop_agent("stopper").await.unwrap();

        // Read inbox to verify shutdown message format
        let inbox: Vec<Message> = read_jsonl(&agent_dir.inbox()).unwrap();
        assert!(!inbox.is_empty());
        let last = inbox.last().unwrap();
        assert!(last.is_shutdown());
        assert_eq!(last.metadata.get("shutdown").map(|s| s.as_str()), Some("true"));
        assert!(last.content.contains("系统即将关闭"));
    }

    #[tokio::test]
    async fn agent_state_active_after_start() {
        let (_tmp, ws) = make_workspace();

        let agent_dir = ws.agent("test");
        std::fs::create_dir_all(agent_dir.path()).unwrap();

        let mut sup = Supervisor::new(ws);
        sup.set_binary_path(std::path::PathBuf::from("/usr/bin/true"));

        assert_eq!(sup.agent_state("test"), AgentState::Sleeping);

        sup.start_agent("test").await.unwrap();
        assert_eq!(sup.agent_state("test"), AgentState::Active);
    }
}
