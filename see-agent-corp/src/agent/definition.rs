use crate::error::{Result, CorpError};
use crate::io::{read_json, read_text, write_json, write_text};
use crate::types::paths::WorkspaceDir;
use crate::types::{AgentDefinition, AgentStatus, AgentSummary};

// Templates embedded at compile time
const TEMPLATE_IDENTITY: &str = include_str!("../../../templates/IDENTITY.md");
const TEMPLATE_SOUL: &str = include_str!("../../../templates/SOUL.md");
const TEMPLATE_AGENTS: &str = include_str!("../../../templates/AGENTS.md");

/// Create a new agent with the given id and optional config overrides.
///
/// Creates the agent directory, writes agent.json, and copies template files.
pub fn create_agent(
    workspace: &WorkspaceDir,
    id: &str,
    name: Option<&str>,
    emoji: Option<&str>,
) -> Result<AgentDefinition> {
    let agent_dir = workspace.agent(id);

    if agent_dir.path().exists() {
        return Err(CorpError::Agent {
            message: format!("agent '{id}' already exists"),
        });
    }

    // Create directory structure
    std::fs::create_dir_all(agent_dir.path())?;
    std::fs::create_dir_all(agent_dir.memory_dir())?;
    std::fs::create_dir_all(agent_dir.session().path())?;
    std::fs::create_dir_all(agent_dir.session().screenshots())?;

    // Write agent.json
    let definition = AgentDefinition::new(id);
    write_json(&agent_dir.agent_json(), &definition)?;

    // Write IDENTITY.md with name/emoji if provided
    let identity = if name.is_some() || emoji.is_some() {
        format!(
            "# Identity\n\n**Name:** {}\n**Emoji:** {}\n\n你是一个 AI 助手，能够看到用户的屏幕并操作 Mac 电脑。\n",
            name.unwrap_or(id),
            emoji.unwrap_or("🤖")
        )
    } else {
        TEMPLATE_IDENTITY.to_owned()
    };
    write_text(&agent_dir.identity_md(), &identity)?;

    // Write SOUL.md and AGENTS.md from templates
    write_text(&agent_dir.soul_md(), TEMPLATE_SOUL)?;
    write_text(&agent_dir.agents_md(), TEMPLATE_AGENTS)?;

    // Initialize empty MEMORY.md
    write_text(&agent_dir.memory_md(), "")?;

    Ok(definition)
}

/// Load an agent definition from disk.
pub fn load_agent(workspace: &WorkspaceDir, id: &str) -> Result<AgentDefinition> {
    let agent_dir = workspace.agent(id);
    if !agent_dir.agent_json().exists() {
        return Err(CorpError::NotFound {
            what: format!("agent '{id}'"),
        });
    }
    read_json(&agent_dir.agent_json())
}

/// List all agents in the workspace, returning summaries.
pub fn list_agents(workspace: &WorkspaceDir) -> Result<Vec<AgentSummary>> {
    let agents_dir = workspace.agents();
    if !agents_dir.exists() {
        return Ok(Vec::new());
    }

    let mut agents = Vec::new();
    for entry in std::fs::read_dir(agents_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let id = entry.file_name().to_string_lossy().to_string();
        let agent_dir = workspace.agent(&id);

        if !agent_dir.agent_json().exists() {
            continue;
        }

        // Parse name/emoji from IDENTITY.md
        let identity = read_text(&agent_dir.identity_md()).unwrap_or_default();
        let name = parse_identity_field(&identity, "Name").unwrap_or_else(|| id.clone());
        let emoji = parse_identity_field(&identity, "Emoji").unwrap_or_else(|| "🤖".to_owned());

        agents.push(AgentSummary {
            id,
            name,
            emoji,
            status: AgentStatus::Idle,
            team_id: None,
            team_name: None,
        });
    }

    agents.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(agents)
}

/// Delete an agent by id. Refuses to delete the system agent.
pub fn delete_agent(workspace: &WorkspaceDir, id: &str) -> Result<()> {
    if id == "system" {
        return Err(CorpError::Agent {
            message: "cannot delete the system agent".to_owned(),
        });
    }

    let agent_dir = workspace.agent(id);
    if !agent_dir.path().exists() {
        return Err(CorpError::NotFound {
            what: format!("agent '{id}'"),
        });
    }

    std::fs::remove_dir_all(agent_dir.path())?;
    Ok(())
}

/// Parse a field from IDENTITY.md. Looks for `**Field:** value` pattern.
fn parse_identity_field(content: &str, field: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        let prefix = format!("**{field}:**");
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ensure_workspace;
    use tempfile::TempDir;

    fn setup() -> (TempDir, WorkspaceDir) {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();
        (tmp, ws)
    }

    #[test]
    fn create_and_load_agent() {
        let (_tmp, ws) = setup();
        let def = create_agent(&ws, "test1", Some("Test Agent"), Some("🧪")).unwrap();
        assert_eq!(def.id, "test1");

        let loaded = load_agent(&ws, "test1").unwrap();
        assert_eq!(loaded.id, "test1");
    }

    #[test]
    fn create_agent_duplicate_fails() {
        let (_tmp, ws) = setup();
        create_agent(&ws, "dup", None, None).unwrap();
        let result = create_agent(&ws, "dup", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn create_agent_writes_templates() {
        let (_tmp, ws) = setup();
        create_agent(&ws, "t1", Some("Alice"), Some("👩")).unwrap();

        let agent_dir = ws.agent("t1");
        let identity = std::fs::read_to_string(agent_dir.identity_md()).unwrap();
        assert!(identity.contains("Alice"));
        assert!(identity.contains("👩"));

        let soul = std::fs::read_to_string(agent_dir.soul_md()).unwrap();
        assert!(soul.contains("高效"));

        let agents_md = std::fs::read_to_string(agent_dir.agents_md()).unwrap();
        assert!(agents_md.contains("操作规则"));
    }

    #[test]
    fn list_agents_includes_system() {
        let (_tmp, ws) = setup();
        // ensure_workspace creates system agent dir, but we need agent.json
        let system_dir = ws.system_agent();
        crate::io::write_json(
            &system_dir.agent_json(),
            &AgentDefinition::new("system"),
        )
        .unwrap();

        create_agent(&ws, "a1", None, None).unwrap();

        let agents = list_agents(&ws).unwrap();
        let ids: Vec<&str> = agents.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"system"));
        assert!(ids.contains(&"a1"));
    }

    #[test]
    fn delete_agent_works() {
        let (_tmp, ws) = setup();
        create_agent(&ws, "del1", None, None).unwrap();
        assert!(ws.agent("del1").path().exists());

        delete_agent(&ws, "del1").unwrap();
        assert!(!ws.agent("del1").path().exists());
    }

    #[test]
    fn delete_system_agent_fails() {
        let (_tmp, ws) = setup();
        let result = delete_agent(&ws, "system");
        assert!(result.is_err());
    }

    #[test]
    fn load_nonexistent_fails() {
        let (_tmp, ws) = setup();
        let result = load_agent(&ws, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn parse_identity_fields() {
        let content = "# Identity\n\n**Name:** Alice\n**Emoji:** 👩\n";
        assert_eq!(parse_identity_field(content, "Name"), Some("Alice".into()));
        assert_eq!(parse_identity_field(content, "Emoji"), Some("👩".into()));
        assert_eq!(parse_identity_field(content, "Missing"), None);
    }
}
