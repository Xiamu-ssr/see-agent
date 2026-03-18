use crate::error::{Result, AgentCorpError};
use crate::types::Message;

use super::TeamTransport;

/// Delivers messages to agents on remote machines via HTTP.
///
/// Posts to `{endpoint}/api/agents/{agent_id}/message` on the remote
/// agentcorp server.
pub struct RemoteTransport {
    endpoint: String,
    client: reqwest::Client,
}

impl RemoteTransport {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_owned(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl TeamTransport for RemoteTransport {
    async fn deliver(&self, agent_id: &str, message: Message) -> Result<()> {
        let url = format!(
            "{}/api/agents/{}/message",
            self.endpoint, agent_id
        );

        let body = serde_json::json!({
            "content": message.content,
            "priority": if message.is_steer() { "steer" } else { "collect" },
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentCorpError::Transport {
                message: format!("failed to deliver to {url}: {e}"),
            })?;

        if !resp.status().is_success() {
            return Err(AgentCorpError::Transport {
                message: format!(
                    "remote agent {agent_id} at {} returned {}",
                    self.endpoint,
                    resp.status()
                ),
            });
        }

        Ok(())
    }
}
