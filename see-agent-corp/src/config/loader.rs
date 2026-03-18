use serde_json::Value;

use crate::error::{Result, CorpError};
use crate::types::paths::WorkspaceDir;
use crate::types::Config;

use super::merge::deep_merge;

/// Load the global config with full merge chain:
///
///   DEFAULT_CONFIG < config.json < SAC_* env vars
///
/// Isolation chamber: operates on `serde_json::Value` internally,
/// returns strong-typed `Config` at boundary.
pub fn load_config(workspace: &WorkspaceDir) -> Result<Config> {
    let default = serde_json::to_value(Config::default())?;

    let merged = if workspace.config().exists() {
        let file_content = std::fs::read_to_string(workspace.config())?;
        let file_value: Value = serde_json::from_str(&file_content).map_err(|e| {
            CorpError::Config {
                message: format!("invalid config.json: {e}"),
            }
        })?;
        deep_merge(&default, &file_value)
    } else {
        default
    };

    let merged = apply_env_overrides(merged, |k| std::env::var(k));

    let config: Config = serde_json::from_value(merged).map_err(|e| CorpError::Config {
        message: format!("config deserialization failed: {e}"),
    })?;

    Ok(config)
}

/// Load agent-specific config with full merge chain:
///
///   DEFAULT_CONFIG < config.json < team.json.config < agent.json < SAC_* env vars
///
/// Pass `team_id = None` when the agent is not part of any team.
pub fn load_agent_config(
    workspace: &WorkspaceDir,
    agent_id: &str,
    team_id: Option<&str>,
) -> Result<Config> {
    // 1. DEFAULT < config.json
    let global = {
        let default = serde_json::to_value(Config::default())?;
        if workspace.config().exists() {
            let file_content = std::fs::read_to_string(workspace.config())?;
            let file_value: Value =
                serde_json::from_str(&file_content).map_err(|e| CorpError::Config {
                    message: format!("invalid config.json: {e}"),
                })?;
            deep_merge(&default, &file_value)
        } else {
            default
        }
    };

    // 2. < team.json.config (if team_id provided)
    let after_team = if let Some(tid) = team_id {
        let team_dir = workspace.team(tid);
        let team_json_path = team_dir.team_json();
        if team_json_path.exists() {
            let team_content = std::fs::read_to_string(&team_json_path)?;
            let team_value: Value =
                serde_json::from_str(&team_content).map_err(|e| CorpError::Config {
                    message: format!("invalid team.json for {tid}: {e}"),
                })?;
            if let Some(team_config) = team_value.get("config") {
                if team_config.is_object() {
                    deep_merge(&global, team_config)
                } else {
                    global
                }
            } else {
                global
            }
        } else {
            global
        }
    } else {
        global
    };

    // 3. < agent.json
    let agent_dir = workspace.agent(agent_id);
    let merged = if agent_dir.agent_json().exists() {
        let agent_content = std::fs::read_to_string(agent_dir.agent_json())?;
        let mut agent_value: Value =
            serde_json::from_str(&agent_content).map_err(|e| CorpError::Config {
                message: format!("invalid agent.json for {agent_id}: {e}"),
            })?;
        // Strip "id" field — it's agent metadata, not config
        if let Some(obj) = agent_value.as_object_mut() {
            obj.remove("id");
        }
        deep_merge(&after_team, &agent_value)
    } else {
        after_team
    };

    // 4. < SAC_* env vars
    let merged = apply_env_overrides(merged, |k| std::env::var(k));

    let config: Config = serde_json::from_value(merged).map_err(|e| CorpError::Config {
        message: format!("agent config deserialization failed: {e}"),
    })?;

    Ok(config)
}

/// Apply SAC_* environment variable overrides.
///
/// Accepts a reader function for testability without mutating global state.
fn apply_env_overrides(
    mut value: Value,
    get_env: impl Fn(&str) -> std::result::Result<String, std::env::VarError>,
) -> Value {
    if let Ok(base_url) = get_env("SAC_BASE_URL") {
        set_nested(&mut value, &["llm", "base_url"], Value::String(base_url));
    }
    if let Ok(api_key) = get_env("SAC_API_KEY") {
        set_nested(&mut value, &["llm", "api_key"], Value::String(api_key));
    }
    if let Ok(model) = get_env("SAC_MODEL") {
        set_nested(&mut value, &["llm", "model"], Value::String(model));
    }
    value
}

/// Set a nested key in a JSON Value object.
fn set_nested(value: &mut Value, keys: &[&str], target: Value) {
    let mut current = value;
    for (i, key) in keys.iter().enumerate() {
        if i == keys.len() - 1 {
            if let Some(obj) = current.as_object_mut() {
                obj.insert((*key).to_string(), target);
            }
            return;
        }
        if !current.get(*key).is_some_and(|v| v.is_object())
            && let Some(obj) = current.as_object_mut()
        {
            obj.insert((*key).to_string(), Value::Object(Default::default()));
        }
        current = current.get_mut(*key).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ensure_workspace;
    use crate::types::paths::WorkspaceDir;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn setup_workspace() -> (TempDir, WorkspaceDir) {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();
        (tmp, ws)
    }

    #[test]
    fn load_default_config() {
        let (_tmp, ws) = setup_workspace();
        let config = load_config(&ws).unwrap();
        assert_eq!(config.llm.model, "gpt-4o");
        assert_eq!(config.agent.max_steps, 50);
    }

    #[test]
    fn load_config_with_overrides() {
        let (_tmp, ws) = setup_workspace();

        std::fs::write(
            ws.config(),
            r#"{"llm": {"model": "claude-opus-4-6"}, "agent": {"max_steps": 100}}"#,
        )
        .unwrap();

        let config = load_config(&ws).unwrap();
        assert_eq!(config.llm.model, "claude-opus-4-6");
        assert_eq!(config.agent.max_steps, 100);
        assert_eq!(config.llm.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn load_agent_config_merges() {
        let (_tmp, ws) = setup_workspace();

        std::fs::write(
            ws.config(),
            r#"{"llm": {"model": "gpt-4o", "api_key": "sk-global"}}"#,
        )
        .unwrap();

        let agent_dir = ws.agent("test-agent");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::write(
            agent_dir.agent_json(),
            r#"{"id": "test-agent", "llm": {"model": "claude-opus-4-6"}, "tools": {"disabled": ["shell"]}}"#,
        )
        .unwrap();

        let config = load_agent_config(&ws, "test-agent", None).unwrap();
        assert_eq!(config.llm.model, "claude-opus-4-6");
        assert_eq!(config.llm.api_key, "sk-global");
        assert_eq!(config.tools.disabled, vec!["shell"]);
    }

    #[test]
    fn env_vars_override_all() {
        // Test env overrides without mutating global state
        let env_map: HashMap<&str, String> = HashMap::from([
            ("SAC_MODEL", "env-model".to_owned()),
            ("SAC_API_KEY", "env-key".to_owned()),
            ("SAC_BASE_URL", "http://localhost:8080".to_owned()),
        ]);
        let get_env = |key: &str| {
            env_map
                .get(key)
                .cloned()
                .ok_or(std::env::VarError::NotPresent)
        };

        let base = serde_json::to_value(Config::default()).unwrap();
        let result = apply_env_overrides(base, get_env);
        let config: Config = serde_json::from_value(result).unwrap();

        assert_eq!(config.llm.model, "env-model");
        assert_eq!(config.llm.api_key, "env-key");
        assert_eq!(config.llm.base_url, "http://localhost:8080");
    }

    #[test]
    fn env_vars_partial_override() {
        // Only override model, keep others default
        let env_map: HashMap<&str, String> =
            HashMap::from([("SAC_MODEL", "custom-model".to_owned())]);
        let get_env = |key: &str| {
            env_map
                .get(key)
                .cloned()
                .ok_or(std::env::VarError::NotPresent)
        };

        let base = serde_json::to_value(Config::default()).unwrap();
        let result = apply_env_overrides(base, get_env);
        let config: Config = serde_json::from_value(result).unwrap();

        assert_eq!(config.llm.model, "custom-model");
        assert_eq!(config.llm.base_url, "https://api.openai.com/v1"); // unchanged
    }

    #[test]
    fn load_config_no_config_file() {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        std::fs::create_dir_all(ws.path()).unwrap();

        let config = load_config(&ws).unwrap();
        assert_eq!(config.llm.model, "gpt-4o");
    }

    #[test]
    fn agent_config_without_agent_json() {
        let (_tmp, ws) = setup_workspace();

        let config = load_agent_config(&ws, "nonexistent", None).unwrap();
        assert_eq!(config.llm.model, "gpt-4o");
    }

    #[test]
    fn three_level_merge_global_team_agent() {
        let (_tmp, ws) = setup_workspace();

        // Global config: model = gpt-4o, api_key = sk-global
        std::fs::write(
            ws.config(),
            r#"{"llm": {"model": "gpt-4o", "api_key": "sk-global"}, "agent": {"max_steps": 30}}"#,
        )
        .unwrap();

        // Team config: overrides model, adds max_steps override
        let team_dir = ws.team("team-alpha");
        std::fs::create_dir_all(team_dir.path()).unwrap();
        std::fs::write(
            team_dir.team_json(),
            r#"{"id": "team-alpha", "name": "Alpha", "members": [], "leader": "a1", "status": "created", "created_at": "2025-01-01",
                "config": {"llm": {"model": "claude-opus-4-6"}, "agent": {"max_steps": 80}}}"#,
        )
        .unwrap();

        // Agent config: overrides model again
        let agent_dir = ws.agent("a1");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::write(
            agent_dir.agent_json(),
            r#"{"id": "a1", "llm": {"model": "gpt-4o-mini"}, "tools": {"disabled": ["shell"]}}"#,
        )
        .unwrap();

        let config = load_agent_config(&ws, "a1", Some("team-alpha")).unwrap();
        // Agent level wins for model
        assert_eq!(config.llm.model, "gpt-4o-mini");
        // Global level api_key preserved through team + agent merges
        assert_eq!(config.llm.api_key, "sk-global");
        // Team level max_steps wins (agent didn't override it)
        assert_eq!(config.agent.max_steps, 80);
        // Agent level tools
        assert_eq!(config.tools.disabled, vec!["shell"]);
    }

    #[test]
    fn team_config_overrides_global() {
        let (_tmp, ws) = setup_workspace();

        std::fs::write(
            ws.config(),
            r#"{"llm": {"model": "gpt-4o"}, "agent": {"max_steps": 50}}"#,
        )
        .unwrap();

        let team_dir = ws.team("team-beta");
        std::fs::create_dir_all(team_dir.path()).unwrap();
        std::fs::write(
            team_dir.team_json(),
            r#"{"id": "team-beta", "name": "Beta", "members": [], "leader": "b1", "status": "created", "created_at": "2025-01-01",
                "config": {"llm": {"model": "claude-opus-4-6"}, "agent": {"max_steps": 100}}}"#,
        )
        .unwrap();

        // No agent.json — just global + team
        let config = load_agent_config(&ws, "b1", Some("team-beta")).unwrap();
        assert_eq!(config.llm.model, "claude-opus-4-6");
        assert_eq!(config.agent.max_steps, 100);
    }

    #[test]
    fn agent_config_overrides_team() {
        let (_tmp, ws) = setup_workspace();

        let team_dir = ws.team("team-gamma");
        std::fs::create_dir_all(team_dir.path()).unwrap();
        std::fs::write(
            team_dir.team_json(),
            r#"{"id": "team-gamma", "name": "Gamma", "members": [], "leader": "g1", "status": "created", "created_at": "2025-01-01",
                "config": {"tools": {"disabled": ["shell", "drag"]}}}"#,
        )
        .unwrap();

        let agent_dir = ws.agent("g1");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::write(
            agent_dir.agent_json(),
            r#"{"id": "g1", "tools": {"disabled": ["screenshot"]}}"#,
        )
        .unwrap();

        let config = load_agent_config(&ws, "g1", Some("team-gamma")).unwrap();
        // Agent array replaces team array entirely (deep_merge behavior)
        assert_eq!(config.tools.disabled, vec!["screenshot"]);
    }

    #[test]
    fn no_team_id_still_works() {
        let (_tmp, ws) = setup_workspace();

        std::fs::write(
            ws.config(),
            r#"{"llm": {"model": "gpt-4o", "api_key": "sk-global"}}"#,
        )
        .unwrap();

        let agent_dir = ws.agent("solo");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::write(
            agent_dir.agent_json(),
            r#"{"id": "solo", "llm": {"model": "claude-opus-4-6"}}"#,
        )
        .unwrap();

        let config = load_agent_config(&ws, "solo", None).unwrap();
        assert_eq!(config.llm.model, "claude-opus-4-6");
        assert_eq!(config.llm.api_key, "sk-global");
    }
}
