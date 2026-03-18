use std::sync::Arc;

use tokio::sync::RwLock;

use agentcorp::config::load_config;
use agentcorp::supervisor::Supervisor;
use agentcorp::types::{Config, WorkspaceDir};

/// Shared application state passed to all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub workspace: WorkspaceDir,
    pub config: RwLock<Config>,
    pub supervisor: RwLock<Supervisor>,
}

impl AppState {
    pub fn new(workspace: WorkspaceDir) -> Self {
        let config = load_config(&workspace).unwrap_or_default();
        let supervisor = Supervisor::new(workspace.clone());
        Self {
            inner: Arc::new(AppStateInner {
                workspace,
                config: RwLock::new(config),
                supervisor: RwLock::new(supervisor),
            }),
        }
    }

    pub fn workspace(&self) -> &WorkspaceDir {
        &self.inner.workspace
    }
}
