use tracing::{info, warn};

use crate::consts::WORKER_HEARTBEAT_SECS;
use crate::supervisor::inbox::drain_inbox_split;
use crate::types::AgentDir;

use super::runtime::AgentRuntime;

// ---------------------------------------------------------------------------
// Worker — long-running inbox drain loop
// ---------------------------------------------------------------------------

/// A worker process that drains an agent's inbox and runs Mode-B turns.
///
/// The worker sits idle until woken by:
/// 1. A SIGUSR1 signal (sent by the supervisor after writing to inbox)
/// 2. A heartbeat timeout (to catch missed signals)
///
/// On wake, it drains new messages from inbox.jsonl, dispatches them
/// through AgentRuntime (collect vs steer), and runs a turn if there
/// are pending collect messages.
pub struct Worker {
    agent_id: String,
    agent_dir: AgentDir,
    runtime: AgentRuntime,
}

impl Worker {
    pub fn new(agent_id: String, agent_dir: AgentDir, runtime: AgentRuntime) -> Self {
        Self {
            agent_id,
            agent_dir,
            runtime,
        }
    }

    /// Main loop. Runs until a shutdown message is received or the process
    /// is terminated externally.
    ///
    /// Returns when a "shutdown" message is encountered.
    pub async fn run(&mut self) -> crate::error::Result<()> {
        info!(agent = %self.agent_id, "worker starting inbox drain loop");

        let inbox_path = self.agent_dir.inbox();
        let cursor_path = self.agent_dir.inbox_cursor();

        loop {
            // Wait for signal or timeout
            wait_for_wake().await;

            // Drain new messages
            let (steer, collect) = match drain_inbox_split(&inbox_path, &cursor_path) {
                Ok(pair) => pair,
                Err(e) => {
                    warn!(agent = %self.agent_id, "inbox drain error: {e}");
                    continue;
                }
            };

            // Check for shutdown
            let has_shutdown = steer.iter().chain(collect.iter()).any(|m| m.is_shutdown());

            let total = steer.len() + collect.len();
            if total > 0 {
                info!(
                    agent = %self.agent_id,
                    steer = steer.len(),
                    collect = collect.len(),
                    "drained {} message(s)",
                    total
                );
            }

            // Dispatch steer messages (injected immediately)
            if !steer.is_empty() {
                self.runtime.dispatch(steer);
            }

            // Dispatch collect messages (batched for next turn)
            if !collect.is_empty() {
                self.runtime.dispatch(collect);
            }

            // Run a turn if there are pending messages
            if self.runtime.has_pending() {
                self.runtime.run_turn().await;
            }

            if has_shutdown {
                info!(agent = %self.agent_id, "shutdown message received, exiting");
                return Ok(());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Signal wait
// ---------------------------------------------------------------------------

/// Wait for SIGUSR1 or a heartbeat timeout, whichever comes first.
#[cfg(unix)]
async fn wait_for_wake() {
    use tokio::signal::unix::{signal, SignalKind};
    use tokio::time::{timeout, Duration};

    let mut sig =
        signal(SignalKind::user_defined1()).expect("failed to register SIGUSR1 handler");

    let _ = timeout(Duration::from_secs(WORKER_HEARTBEAT_SECS), sig.recv()).await;
}

/// Non-unix fallback: just sleep for the heartbeat interval.
#[cfg(not(unix))]
async fn wait_for_wake() {
    tokio::time::sleep(tokio::time::Duration::from_secs(WORKER_HEARTBEAT_SECS)).await;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::{Brain, BrainResponse};
    use crate::error::Result;
    use crate::eye::{Eye, Screenshot};
    use crate::supervisor::inbox::send_to_inbox;
    use crate::tool::ToolRegistry;
    use crate::types::{Config, Message, MessagePriority};
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::HashMap;
    use tempfile::TempDir;

    struct MockBrain;

    #[async_trait]
    impl Brain for MockBrain {
        async fn chat(&self, _: &[serde_json::Value], _: &[serde_json::Value]) -> Result<BrainResponse> {
            Ok(BrainResponse {
                content: Some("OK".into()),
                tool_calls: vec![],
                raw: json!({"role": "assistant", "content": "OK"}),
            })
        }
        async fn summarize(&self, _: &[serde_json::Value]) -> Result<String> {
            Ok("summary".into())
        }
    }

    struct MockEye;

    #[async_trait]
    impl Eye for MockEye {
        async fn capture(&self) -> Result<Screenshot> {
            Err(crate::error::CorpError::Agent {
                message: "mock".into(),
            })
        }
    }

    fn make_msg(content: &str, priority: MessagePriority) -> Message {
        Message {
            msg_id: None,
            sender: "alice".into(),
            content: content.into(),
            priority,
            metadata: HashMap::new(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn worker_processes_shutdown() {
        let tmp = TempDir::new().unwrap();
        let agent_dir = AgentDir::new(tmp.path());

        // Initialize cursor at 0 (like create_agent does)
        crate::supervisor::inbox::write_cursors(&agent_dir.inbox_cursor(), 0, 0).unwrap();

        // Write a shutdown message to inbox
        let inbox = agent_dir.inbox();
        send_to_inbox(&inbox, &make_msg("shutdown", MessagePriority::Collect)).unwrap();

        let brain = Box::new(MockBrain);
        let eye: std::sync::Arc<dyn crate::eye::Eye> = std::sync::Arc::new(MockEye);
        let registry = ToolRegistry::new();
        let config = Config::default();
        let agent_loop = crate::agent::AgentLoop::new(brain, eye, registry, config, "test".into());
        let runtime = super::AgentRuntime::new(agent_loop, "system".into(), 5);

        let _worker = Worker::new("test".into(), agent_dir, runtime);

        // Direct drain test: verify inbox contains the shutdown message.
        // We can't run the full worker loop in tests because wait_for_wake
        // blocks on SIGUSR1/timeout. Instead test the drain logic directly.
        let inbox_path = tmp.path().join("inbox.jsonl");
        let cursor_path = tmp.path().join("inbox_cursor.json");
        let (_steer, collect) = drain_inbox_split(&inbox_path, &cursor_path).unwrap();
        assert_eq!(collect.len(), 1);
        assert!(collect[0].is_shutdown());
    }
}
