mod local;
mod remote;

use crate::error::Result;
use crate::types::{Message, TeamMember, WorkspaceDir};

pub use local::LocalTransport;
pub use remote::RemoteTransport;

/// Trait for delivering messages to team members.
///
/// The transport decides HOW to deliver a message based on the member's
/// endpoint configuration: None → local (file + signal), Some(addr) → HTTP.
#[async_trait::async_trait]
pub trait TeamTransport: Send + Sync {
    /// Send a message to a specific agent.
    async fn deliver(&self, agent_id: &str, message: Message) -> Result<()>;
}

/// Create the appropriate transport for a team member.
///
/// - `endpoint = None` → LocalTransport (writes to inbox.jsonl + SIGUSR1)
/// - `endpoint = Some(addr)` → RemoteTransport (HTTP POST to remote node)
pub fn transport_for_member(
    member: &TeamMember,
    workspace: &WorkspaceDir,
) -> Box<dyn TeamTransport> {
    match &member.endpoint {
        None => Box::new(LocalTransport::new(workspace.clone())),
        Some(endpoint) => Box::new(RemoteTransport::new(endpoint.clone())),
    }
}
