use std::sync::Arc;

use tokio::sync::RwLock;

use see_agent_corp::config::load_config;
use see_agent_corp::supervisor::Supervisor;
use see_agent_corp::types::{Config, WorkspaceDir};

/// Shared application state passed to all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub workspace: WorkspaceDir,
    pub config: RwLock<Config>,
    pub supervisor: RwLock<Supervisor>,
    pub frozen_agents: RwLock<Vec<String>>,
}

impl AppState {
    pub fn new(workspace: WorkspaceDir) -> Self {
        let config = match load_config(&workspace) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("config load failed, using defaults: {e}");
                Config::default()
            }
        };
        let supervisor = Supervisor::new(workspace.clone());
        Self {
            inner: Arc::new(AppStateInner {
                workspace,
                config: RwLock::new(config),
                supervisor: RwLock::new(supervisor),
                frozen_agents: RwLock::new(Vec::new()),
            }),
        }
    }

    pub fn workspace(&self) -> &WorkspaceDir {
        &self.inner.workspace
    }
}
