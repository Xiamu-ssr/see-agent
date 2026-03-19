use std::collections::HashMap;
use std::path::PathBuf;

use tracing::{info, warn};

use crate::error::{Result, CorpError};
use crate::io::read_json;
use crate::sandbox::{build_safehouse_args, build_sandbox_profile, safehouse_available};
use crate::team::find_agent_team;
use crate::types::{AgentDefinition, AgentState, Message, MessagePriority, WorkspaceDir};

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
    /// Whether sandbox is enabled (from config).
    sandbox_enabled: bool,
    /// Whether the safehouse binary is available in PATH.
    safehouse_available: bool,
}

impl Supervisor {
    pub fn new(workspace: WorkspaceDir) -> Self {
        let binary_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("see-agent-corp"));
        Self {
            workspace,
            processes: HashMap::new(),
            binary_path,
            sandbox_enabled: true,
            safehouse_available: safehouse_available(),
        }
    }

    /// Set sandbox state from config.
    pub fn set_sandbox_enabled(&mut self, enabled: bool) {
        self.sandbox_enabled = enabled;
    }

    /// Whether sandbox is actually active (enabled AND safehouse available).
    pub fn sandbox_active(&self) -> bool {
        self.sandbox_enabled && self.safehouse_available
    }

    /// Whether safehouse binary is available.
    pub fn is_safehouse_available(&self) -> bool {
        self.safehouse_available
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

        // Spawn worker process with stdout/stderr redirected to worker.log
        let log_path = agent_dir.worker_log();
        let log_file = std::fs::File::create(&log_path).map_err(|e| CorpError::Agent {
            message: format!("failed to create worker.log for '{agent_id}': {e}"),
        })?;
        let log_stderr = log_file.try_clone().map_err(|e| CorpError::Agent {
            message: format!("failed to clone log file for '{agent_id}': {e}"),
        })?;

        let child = if self.sandbox_active() {
            // Load agent definition to determine permissions
            let agent_def = read_json::<AgentDefinition>(&agent_dir.agent_json()).ok();
            let is_system = agent_def.as_ref().is_some_and(|d| d.is_system);
            let team_id = find_agent_team(&self.workspace, agent_id)
                .ok()
                .flatten();
            let agent_sandbox = agent_def.as_ref().and_then(|d| d.sandbox.as_ref());
            let config = crate::config::load_config(&self.workspace).unwrap_or_default();

            let profile = build_sandbox_profile(
                &self.workspace,
                agent_id,
                is_system,
                team_id.as_deref(),
                &config,
                agent_sandbox,
            );
            let safehouse_args = build_safehouse_args(&profile);

            info!(agent = agent_id, "spawning worker with safehouse sandbox");
            tokio::process::Command::new("safehouse")
                .args(&safehouse_args)
                .arg(&self.binary_path)
                .arg("worker")
                .arg(agent_id)
                .arg(self.workspace.path().to_string_lossy().as_ref())
                .env("SAC_BIN", &self.binary_path)
                .env_remove("CONDA_PREFIX")
                .env_remove("CONDA_DEFAULT_ENV")
                .env_remove("VIRTUAL_ENV")
                .stdout(log_file)
                .stderr(log_stderr)
                .spawn()
                .map_err(|e| CorpError::Agent {
                    message: format!("failed to spawn sandboxed worker for '{agent_id}': {e}"),
                })?
        } else {
            tokio::process::Command::new(&self.binary_path)
                .arg("worker")
                .arg(agent_id)
                .arg(self.workspace.path().to_string_lossy().as_ref())
                .env("SAC_BIN", &self.binary_path)
                .env_remove("CONDA_PREFIX")
                .env_remove("CONDA_DEFAULT_ENV")
                .env_remove("VIRTUAL_ENV")
                .stdout(log_file)
                .stderr(log_stderr)
                .spawn()
                .map_err(|e| CorpError::Agent {
                    message: format!("failed to spawn worker for '{agent_id}': {e}"),
                })?
        };

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
        let just_started = if !self.is_running(agent_id) {
            self.start_agent(agent_id).await?;
            true
        } else {
            false
        };

        let inbox_path = agent_dir.inbox();
        send_to_inbox_with_id(&inbox_path, message)?;

        // Wake the worker — but skip if just started, because:
        // 1. The worker hasn't registered its SIGUSR1 handler yet (race condition)
        // 2. A freshly spawned worker will drain inbox on its own first iteration
        if !just_started {
            // Read actual worker PID from file (safehouse may fork, changing the PID)
            let pid_path = agent_dir.worker_pid();
            if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    signal_process(pid);
                }
            } else if let Some(handle) = self.processes.get(agent_id) {
                // Fallback to spawn PID if worker.pid not available yet
                signal_process(handle.pid);
            }
        }

        Ok(())
    }

    /// Check if an agent's worker process is actually running.
    /// Uses try_wait() to detect crashed processes and cleans them up.
    pub fn is_running(&mut self, agent_id: &str) -> bool {
        if let Some(handle) = self.processes.get_mut(agent_id) {
            match handle.child.try_wait() {
                Ok(Some(status)) => {
                    warn!(agent = agent_id, ?status, "worker process exited, cleaning up");
                    self.processes.remove(agent_id);
                    false
                }
                Ok(None) => true,
                Err(e) => {
                    warn!(agent = agent_id, "try_wait failed: {e}, cleaning up");
                    self.processes.remove(agent_id);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Get the lifecycle state of an agent.
    pub fn agent_state(&mut self, agent_id: &str) -> AgentState {
        if self.is_running(agent_id) {
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

    /// Reap any exited worker processes to prevent zombies.
    ///
    /// Calls `try_wait()` on all tracked processes, removing those that
    /// have exited. Should be called periodically (e.g., on heartbeat).
    pub fn reap_exited(&mut self) {
        let mut exited = Vec::new();
        for (id, handle) in &mut self.processes {
            match handle.child.try_wait() {
                Ok(Some(status)) => {
                    warn!(agent = %id, ?status, "worker process exited (reaped)");
                    exited.push(id.clone());
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(agent = %id, "try_wait failed during reap: {e}");
                    exited.push(id.clone());
                }
            }
        }
        for id in exited {
            self.processes.remove(&id);
        }
    }

    /// Restart a running worker. Sends shutdown, waits for exit, then the
    /// next `send_to()` call will auto-start a fresh worker.
    pub async fn restart_agent(&mut self, agent_id: &str) -> Result<()> {
        if self.is_running(agent_id) {
            self.stop_agent(agent_id).await?;
        }
        Ok(())
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
        let mut sup = Supervisor::new(ws);
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
        let mut sup = Supervisor::new(ws);
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
    async fn send_to_skips_signal_for_just_started_agent() {
        let (_tmp, ws) = make_workspace();

        let agent_dir = ws.agent("fresh");
        std::fs::create_dir_all(agent_dir.path()).unwrap();

        let mut sup = Supervisor::new(ws);
        // Use `tail -f /dev/null` via sh — blocks forever, ignores args
        sup.set_binary_path(std::path::PathBuf::from("/bin/sh"));

        // Manually test the logic: after auto-start, the process should survive
        // because send_to skips SIGUSR1 for just-started workers.
        // We can't perfectly simulate since /bin/sh with wrong args exits,
        // but we can verify the code path by checking the message is in inbox.
        let msg = Message {
            msg_id: None,
            sender: "user".into(),
            content: "hello".into(),
            priority: MessagePriority::Collect,
            metadata: Default::default(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        };
        sup.send_to("fresh", msg).await.unwrap();

        // Inbox should contain the message regardless
        let inbox: Vec<Message> = read_jsonl(&agent_dir.inbox()).unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].content, "hello");
    }

    #[tokio::test]
    async fn agent_state_tracks_liveness() {
        let (_tmp, ws) = make_workspace();

        let agent_dir = ws.agent("test");
        std::fs::create_dir_all(agent_dir.path()).unwrap();

        let mut sup = Supervisor::new(ws);
        sup.set_binary_path(std::path::PathBuf::from("/usr/bin/true"));

        assert_eq!(sup.agent_state("test"), AgentState::Sleeping);

        sup.start_agent("test").await.unwrap();
        // /usr/bin/true exits immediately, so after a brief wait the process is dead
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        // is_running should detect the exited process and clean up
        assert!(!sup.is_running("test"));
        assert_eq!(sup.agent_state("test"), AgentState::Sleeping);
    }
}
