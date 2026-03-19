use std::path::PathBuf;

use crate::error::Result;
use crate::types::paths::WorkspaceDir;
use crate::types::{Config, SkillsConfig};

/// Resolve the workspace root directory.
///
/// Priority: `SAC_HOME` env var > `~/.agentcorp/`
pub fn resolve_workspace_root() -> PathBuf {
    if let Ok(home) = std::env::var("SAC_HOME") {
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
    ];

    for dir in &dirs_to_create {
        std::fs::create_dir_all(dir)?;
    }

    // Write default config.json if absent, or patch empty skills.dirs
    let config_path = workspace.config();
    if !config_path.exists() {
        let default_config = Config::default();
        let json = serde_json::to_string_pretty(&default_config)?;
        std::fs::write(&config_path, json)?;
    } else {
        // Patch: if skills.dirs is empty, fill with default
        let content = std::fs::read_to_string(&config_path)?;
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
            let needs_patch = val
                .get("skills")
                .and_then(|s| s.get("dirs"))
                .and_then(|d| d.as_array())
                .is_some_and(|a| a.is_empty());
            if needs_patch {
                let default_dirs = SkillsConfig::default().dirs;
                val["skills"]["dirs"] = serde_json::to_value(default_dirs)?;
                let json = serde_json::to_string_pretty(&val)?;
                std::fs::write(&config_path, json)?;
            }
        }
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
    fn ensure_workspace_patches_empty_skills_dirs() {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());

        // Write config with empty skills.dirs
        std::fs::create_dir_all(ws.path()).unwrap();
        std::fs::write(
            ws.config(),
            r#"{"skills": {"dirs": [], "disabled": []}}"#,
        )
        .unwrap();

        ensure_workspace(&ws).unwrap();

        let content = std::fs::read_to_string(ws.config()).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        let dirs = val["skills"]["dirs"].as_array().unwrap();
        assert!(!dirs.is_empty(), "skills.dirs should be patched with default");
        assert!(dirs[0].as_str().unwrap().contains("skills"));
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
