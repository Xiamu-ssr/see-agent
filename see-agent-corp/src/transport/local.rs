use crate::error::Result;
use crate::supervisor::inbox::send_to_inbox_with_id;
use crate::types::{Message, WorkspaceDir};

use super::TeamTransport;

/// Delivers messages to agents on the same machine.
///
/// Writes the message to `inbox.jsonl` and optionally sends SIGUSR1
/// to wake the worker process.
pub struct LocalTransport {
    workspace: WorkspaceDir,
}

impl LocalTransport {
    pub fn new(workspace: WorkspaceDir) -> Self {
        Self { workspace }
    }
}

#[async_trait::async_trait]
impl TeamTransport for LocalTransport {
    async fn deliver(&self, agent_id: &str, message: Message) -> Result<()> {
        let agent_dir = self.workspace.agent(agent_id);
        let inbox_path = agent_dir.inbox();
        send_to_inbox_with_id(&inbox_path, message)?;

        // Signal the worker if running (best-effort, PID not available here)
        // The supervisor handles signaling; this transport just writes.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::read_jsonl;
    use crate::types::MessagePriority;
    use tempfile::TempDir;

    #[tokio::test]
    async fn local_deliver_writes_to_inbox() {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());

        // Create agent directory
        let agent_dir = ws.agent("alice");
        std::fs::create_dir_all(agent_dir.path()).unwrap();

        let transport = LocalTransport::new(ws);

        let msg = Message {
            msg_id: None,
            sender: "bob".into(),
            content: "hello from local".into(),
            priority: MessagePriority::Collect,
            metadata: Default::default(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        };

        transport.deliver("alice", msg).await.unwrap();

        let inbox: Vec<Message> = read_jsonl(&agent_dir.inbox()).unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].content, "hello from local");
        assert_eq!(inbox[0].msg_id, Some(0));
    }
}
