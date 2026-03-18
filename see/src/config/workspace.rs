use std::path::PathBuf;

use crate::error::Result;
use crate::types::paths::WorkspaceDir;
use crate::types::Config;

/// Resolve the workspace root directory.
///
/// Priority: `SEE_AGENT_HOME` env var > `~/.see-agent/`
pub fn resolve_workspace_root() -> PathBuf {
    if let Ok(home) = std::env::var("SEE_AGENT_HOME") {
        return PathBuf::from(home);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(crate::consts::WORKSPACE_DIR_NAME)
}

/// Ensure the workspace directory structure exists.
///
/// Creates all required subdirectories and writes default `config.json`
/// if it does not exist.
pub fn ensure_workspace(workspace: &WorkspaceDir) -> Result<()> {
    let dirs_to_create = [
        workspace.path().to_path_buf(),
        workspace.agents(),
        workspace.teams(),
        workspace.skills(),
        workspace.logs(),
    ];

    for dir in &dirs_to_create {
        std::fs::create_dir_all(dir)?;
    }

    // Write default config.json if absent
    let config_path = workspace.config();
    if !config_path.exists() {
        let default_config = Config::default();
        let json = serde_json::to_string_pretty(&default_config)?;
        std::fs::write(&config_path, json)?;
    }

    // Ensure system agent directory exists
    let system_dir = workspace.system_agent();
    std::fs::create_dir_all(system_dir.path())?;
    std::fs::create_dir_all(system_dir.memory_dir())?;
    std::fs::create_dir_all(system_dir.session().path())?;
    std::fs::create_dir_all(system_dir.session().screenshots())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn ensure_workspace_creates_structure() {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();

        assert!(ws.config().exists());
        assert!(ws.agents().exists());
        assert!(ws.teams().exists());
        assert!(ws.skills().exists());
        assert!(ws.logs().exists());
        assert!(ws.system_agent().path().exists());
        assert!(ws.system_agent().memory_dir().exists());
        assert!(ws.system_agent().session().screenshots().exists());
    }

    #[test]
    fn ensure_workspace_preserves_existing_config() {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());

        // Write custom config first
        std::fs::create_dir_all(ws.path()).unwrap();
        std::fs::write(ws.config(), r#"{"llm": {"model": "custom"}}"#).unwrap();

        ensure_workspace(&ws).unwrap();

        // Should not overwrite
        let content = std::fs::read_to_string(ws.config()).unwrap();
        assert!(content.contains("custom"));
    }

    #[test]
    fn ensure_workspace_idempotent() {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();
        ensure_workspace(&ws).unwrap(); // second call should not fail
        assert!(ws.config().exists());
    }
}
