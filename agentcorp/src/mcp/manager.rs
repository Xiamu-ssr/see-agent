use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::error::Result;
use crate::tool::{Tool, ToolRegistry};
use crate::types::McpServerConfig;

use super::client::McpClient;
use super::tool_wrapper::McpToolWrapper;

// ---------------------------------------------------------------------------
// McpManager
// ---------------------------------------------------------------------------

/// Manages connections to multiple MCP servers.
///
/// Each server is connected lazily on `connect_all()`, and discovered tools
/// are registered with the `ToolRegistry`.
pub struct McpManager {
    clients: Vec<Arc<Mutex<McpClient>>>,
    /// Server names, parallel to `clients`.
    names: Vec<String>,
}

impl McpManager {
    /// Create from a map of server configurations.
    pub fn new(
        servers: &HashMap<String, McpServerConfig>,
        global_env: &HashMap<String, String>,
    ) -> Self {
        let mut clients = Vec::new();
        let mut names = Vec::new();

        for (name, config) in servers {
            let client = McpClient::new(
                name.clone(),
                config.command.clone(),
                config.args.clone(),
                config.env.clone(),
                global_env,
            );
            clients.push(Arc::new(Mutex::new(client)));
            names.push(name.clone());
        }

        Self { clients, names }
    }

    /// Connect to all configured MCP servers.
    ///
    /// Individual connection failures are logged but do not prevent
    /// other servers from connecting.
    pub async fn connect_all(&self) {
        for (i, client) in self.clients.iter().enumerate() {
            let name = &self.names[i];
            let mut c = client.lock().await;
            if let Err(e) = c.connect().await {
                warn!(server = %name, "MCP connect failed: {e}");
            } else {
                info!(server = %name, "MCP server connected");
            }
        }
    }

    /// Disconnect all MCP servers.
    pub async fn disconnect_all(&self) {
        for (i, client) in self.clients.iter().enumerate() {
            let name = &self.names[i];
            let mut c = client.lock().await;
            c.disconnect().await;
            info!(server = %name, "MCP server disconnected");
        }
    }

    /// Discover tools from all connected servers and register them.
    ///
    /// Skips servers that aren't connected. Duplicate tool names are
    /// logged as warnings.
    pub async fn register_tools(&self, registry: &mut ToolRegistry) -> Result<()> {
        for (i, client_arc) in self.clients.iter().enumerate() {
            let name = &self.names[i];
            let mut client = client_arc.lock().await;

            if !client.is_connected() {
                warn!(server = %name, "skipping tool discovery — not connected");
                continue;
            }

            let tools = match client.list_tools().await {
                Ok(t) => t,
                Err(e) => {
                    warn!(server = %name, "tool discovery failed: {e}");
                    continue;
                }
            };

            // Drop the lock before registering (McpToolWrapper holds its own Arc)
            drop(client);

            for tool_info in tools {
                let wrapper = McpToolWrapper::new(
                    name,
                    &tool_info.name,
                    &tool_info.description,
                    tool_info.input_schema,
                    Arc::clone(client_arc),
                );

                let full_name = wrapper.name().to_owned();
                if registry.get(&full_name).is_some() {
                    warn!(tool = %full_name, "duplicate MCP tool, skipping");
                    continue;
                }

                info!(tool = %full_name, server = %name, "registered MCP tool");
                registry.register(Box::new(wrapper));
            }
        }

        Ok(())
    }

    /// Get the names of all configured servers.
    pub fn server_names(&self) -> &[String] {
        &self.names
    }

    /// Check if a specific server is connected.
    pub async fn is_connected(&self, name: &str) -> bool {
        for (i, client) in self.clients.iter().enumerate() {
            if self.names[i] == name {
                let c = client.lock().await;
                return c.is_connected();
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::McpServerType;

    #[test]
    fn manager_from_empty_config() {
        let servers = HashMap::new();
        let mgr = McpManager::new(&servers, &HashMap::new());
        assert!(mgr.server_names().is_empty());
    }

    #[test]
    fn manager_creates_clients() {
        let mut servers = HashMap::new();
        servers.insert(
            "test-server".into(),
            McpServerConfig {
                server_type: McpServerType::Stdio,
                command: "echo".into(),
                args: vec!["hello".into()],
                env: HashMap::new(),
                url: None,
            },
        );
        let mgr = McpManager::new(&servers, &HashMap::new());
        assert_eq!(mgr.server_names().len(), 1);
        assert!(mgr.server_names().contains(&"test-server".to_owned()));
    }

    #[tokio::test]
    async fn not_connected_by_default() {
        let mut servers = HashMap::new();
        servers.insert(
            "srv".into(),
            McpServerConfig {
                server_type: McpServerType::Stdio,
                command: "echo".into(),
                args: vec![],
                env: HashMap::new(),
                url: None,
            },
        );
        let mgr = McpManager::new(&servers, &HashMap::new());
        assert!(!mgr.is_connected("srv").await);
    }
}
