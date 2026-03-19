use std::path::PathBuf;

use crate::error::Result;
use crate::io::{write_json, write_text};
use crate::types::paths::WorkspaceDir;
use crate::types::{AgentDefinition, Config};

const SYSTEM_SKILL: &str = include_str!("../../../templates/system-skill/SKILL.md");
const SYSTEM_SOUL: &str = include_str!("../../../templates/system-soul.md");
const AGENTS_TEMPLATE: &str = include_str!("../../../templates/AGENTS.md");
const CLAWHUB_SKILL: &str = include_str!("../../../templates/clawhub-skill/SKILL.md");

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

    // Write default config.json if absent
    let config_path = workspace.config();
    if !config_path.exists() {
        let default_config = Config::default();
        let json = serde_json::to_string_pretty(&default_config)?;
        std::fs::write(&config_path, json)?;
    }

    // Built-in global skill: clawhub
    let clawhub_dir = workspace.skills().join("clawhub");
    if !clawhub_dir.join("SKILL.md").exists() {
        std::fs::create_dir_all(&clawhub_dir)?;
        std::fs::write(clawhub_dir.join("SKILL.md"), CLAWHUB_SKILL)?;
    }

    // Ensure system agent is fully initialized (per-item checks for upgrades)
    let system_dir = workspace.system_agent();
    std::fs::create_dir_all(system_dir.path())?;
    std::fs::create_dir_all(system_dir.memory_dir())?;
    std::fs::create_dir_all(system_dir.session().path())?;
    std::fs::create_dir_all(system_dir.session().screenshots())?;

    // agent.json (no skills.dirs needed — built-in defaults cover agents/{id}/skills/)
    if !system_dir.agent_json().exists() {
        let mut def = AgentDefinition::new("system");
        def.is_system = true;
        write_json(&system_dir.agent_json(), &def)?;
    }

    // Skills directory + SKILL.md
    let skills_dir = system_dir.path().join("skills").join("system-management");
    if !skills_dir.join("SKILL.md").exists() {
        std::fs::create_dir_all(&skills_dir)?;
        std::fs::write(skills_dir.join("SKILL.md"), SYSTEM_SKILL)?;
    }

    // SOUL.md
    if !system_dir.soul_md().exists() {
        write_text(&system_dir.soul_md(), SYSTEM_SOUL)?;
    }

    // AGENTS.md
    if !system_dir.agents_md().exists() {
        write_text(&system_dir.agents_md(), AGENTS_TEMPLATE)?;
    }

    // IDENTITY.md
    if !system_dir.identity_md().exists() {
        write_text(
            &system_dir.identity_md(),
            "# Identity\n\n**Name:** Steward\n**Emoji:** 🦞\n**Race:** 🦞\n\n系统管家，负责 workspace 管理和系统配置。\n",
        )?;
    }

    // Inbox / cursor / messages / memory
    if !system_dir.inbox().exists() {
        write_text(&system_dir.inbox(), "")?;
    }
    if !system_dir.inbox_cursor().exists() {
        write_json(&system_dir.inbox_cursor(), &serde_json::json!({"collect": 0, "steer": 0}))?;
    }
    if !system_dir.session().messages().exists() {
        write_text(&system_dir.session().messages(), "")?;
    }
    if !system_dir.memory_md().exists() {
        write_text(&system_dir.memory_md(), "")?;
    }

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
        assert!(ws.skills().join("clawhub").join("SKILL.md").exists());
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

    #[test]
    fn ensure_workspace_upgrades_existing_system_agent() {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());

        // Simulate old workspace: system agent.json exists but no SOUL.md/AGENTS.md/skills
        let sys = ws.system_agent();
        std::fs::create_dir_all(sys.path()).unwrap();
        let mut def = AgentDefinition::new("system");
        def.is_system = true;
        write_json(&sys.agent_json(), &def).unwrap();

        ensure_workspace(&ws).unwrap();

        // Should have been upgraded with all files
        assert!(sys.soul_md().exists(), "SOUL.md should be created");
        assert!(sys.agents_md().exists(), "AGENTS.md should be created");
        assert!(sys.identity_md().exists(), "IDENTITY.md should be created");
        let skill_path = sys.path().join("skills").join("system-management").join("SKILL.md");
        assert!(skill_path.exists(), "SKILL.md should be created");
    }
}
