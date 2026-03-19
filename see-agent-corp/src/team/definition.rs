use chrono::Utc;

use crate::error::{Result, CorpError};
use crate::io::{read_json, write_json};
use crate::types::paths::WorkspaceDir;
use crate::types::{TeamDefinition, TeamMember, TeamStatus, TeamSummary};

/// Create a new team.
///
/// Generates an 8-char hex id, writes team.json, creates supporting directories.
pub fn create_team(
    workspace: &WorkspaceDir,
    name: &str,
    members: Vec<TeamMember>,
    leader: Option<&str>,
) -> Result<TeamDefinition> {
    let id = generate_team_id();
    let team_dir = workspace.team(&id);

    std::fs::create_dir_all(team_dir.path())?;
    std::fs::create_dir_all(team_dir.shared())?;

    let leader_id = leader
        .map(|s| s.to_owned())
        .or_else(|| members.first().map(|m| m.id.clone()))
        .unwrap_or_default();

    let definition = TeamDefinition {
        id: id.clone(),
        name: name.to_owned(),
        members,
        leader: leader_id,
        status: TeamStatus::Created,
        created_at: Utc::now().to_rfc3339(),
        config: None,
    };

    write_json(&team_dir.team_json(), &definition)?;

    // Initialize empty tasklist
    write_json(&team_dir.tasklist(), &Vec::<serde_json::Value>::new())?;

    // Initialize empty messages
    std::fs::write(team_dir.messages(), "")?;

    Ok(definition)
}

/// Load a team definition from disk.
pub fn load_team(workspace: &WorkspaceDir, id: &str) -> Result<TeamDefinition> {
    let team_dir = workspace.team(id);
    if !team_dir.team_json().exists() {
        return Err(CorpError::NotFound {
            what: format!("team '{id}'"),
        });
    }
    read_json(&team_dir.team_json())
}

/// List all teams in the workspace.
pub fn list_teams(workspace: &WorkspaceDir) -> Result<Vec<TeamSummary>> {
    let teams_dir = workspace.teams();
    if !teams_dir.exists() {
        return Ok(Vec::new());
    }

    let mut teams = Vec::new();
    for entry in std::fs::read_dir(teams_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let id = entry.file_name().to_string_lossy().to_string();
        let team_dir = workspace.team(&id);

        if !team_dir.team_json().exists() {
            continue;
        }

        let def: TeamDefinition = match read_json(&team_dir.team_json()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        teams.push(TeamSummary {
            id: def.id,
            name: def.name,
            status: def.status,
            members: def.members,
            leader: def.leader,
        });
    }

    teams.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(teams)
}

/// Delete a team by id.
pub fn delete_team(workspace: &WorkspaceDir, id: &str) -> Result<()> {
    let team_dir = workspace.team(id);
    if !team_dir.path().exists() {
        return Err(CorpError::NotFound {
            what: format!("team '{id}'"),
        });
    }
    std::fs::remove_dir_all(team_dir.path())?;
    Ok(())
}

/// Set a new leader for the team. The new leader must be an existing member.
pub fn set_leader(workspace: &WorkspaceDir, team_id: &str, new_leader: &str) -> Result<()> {
    let team_dir = workspace.team(team_id);
    let mut def: TeamDefinition = read_json(&team_dir.team_json()).map_err(|_| CorpError::NotFound {
        what: format!("team '{team_id}'"),
    })?;

    if !def.members.iter().any(|m| m.id == new_leader) {
        return Err(CorpError::Team {
            message: format!("'{new_leader}' is not a member of team '{team_id}'"),
        });
    }

    def.leader = new_leader.to_owned();
    write_json(&team_dir.team_json(), &def)?;
    Ok(())
}

/// Remove a member from a team. Returns the updated team or error.
///
/// If the agent is the leader, returns an error.
/// If the team would become empty, deletes the entire team and returns Ok(None).
pub fn remove_member_from_team(
    workspace: &WorkspaceDir,
    team_id: &str,
    agent_id: &str,
) -> Result<Option<TeamDefinition>> {
    let team_dir = workspace.team(team_id);
    let mut def: TeamDefinition = read_json(&team_dir.team_json()).map_err(|_| CorpError::NotFound {
        what: format!("team '{team_id}'"),
    })?;

    // If the agent is the leader and team has other members, error
    if def.leader == agent_id && def.members.len() > 1 {
        return Err(CorpError::Team {
            message: format!("cannot remove leader '{agent_id}' from team '{team_id}' — change leader first"),
        });
    }

    def.members.retain(|m| m.id != agent_id);

    if def.members.is_empty() {
        delete_team(workspace, team_id)?;
        return Ok(None);
    }

    write_json(&team_dir.team_json(), &def)?;
    Ok(Some(def))
}

/// Add a member to a team.
pub fn add_member_to_team(
    workspace: &WorkspaceDir,
    team_id: &str,
    agent_id: &str,
    role: &str,
) -> Result<TeamDefinition> {
    let team_dir = workspace.team(team_id);
    let mut def: TeamDefinition = read_json(&team_dir.team_json()).map_err(|_| CorpError::NotFound {
        what: format!("team '{team_id}'"),
    })?;

    if def.members.iter().any(|m| m.id == agent_id) {
        return Err(CorpError::Team {
            message: format!("'{agent_id}' is already a member of team '{team_id}'"),
        });
    }

    def.members.push(TeamMember {
        id: agent_id.to_owned(),
        role: role.to_owned(),
        endpoint: None,
    });

    write_json(&team_dir.team_json(), &def)?;
    Ok(def)
}

/// Find which team an agent belongs to (if any).
pub fn find_agent_team(workspace: &WorkspaceDir, agent_id: &str) -> Result<Option<String>> {
    let teams_dir = workspace.teams();
    if !teams_dir.exists() {
        return Ok(None);
    }
    for entry in std::fs::read_dir(teams_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let tid = entry.file_name().to_string_lossy().to_string();
        let team_dir = workspace.team(&tid);
        if let Ok(def) = read_json::<TeamDefinition>(&team_dir.team_json())
            && def.members.iter().any(|m| m.id == agent_id)
        {
            return Ok(Some(tid));
        }
    }
    Ok(None)
}

/// Generate an 8-character hex id for a team.
fn generate_team_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{nanos:08x}")
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
    fn create_and_load_team() {
        let (_tmp, ws) = setup();

        let members = vec![
            TeamMember {
                id: "alice".into(),
                role: "leader".into(),
                endpoint: None,
            },
            TeamMember {
                id: "bob".into(),
                role: "dev".into(),
                endpoint: None,
            },
        ];

        let team = create_team(&ws, "Test Team", members, Some("alice")).unwrap();
        assert_eq!(team.name, "Test Team");
        assert_eq!(team.leader, "alice");
        assert_eq!(team.members.len(), 2);

        let loaded = load_team(&ws, &team.id).unwrap();
        assert_eq!(loaded.name, "Test Team");
    }

    #[test]
    fn list_teams_empty() {
        let (_tmp, ws) = setup();
        let teams = list_teams(&ws).unwrap();
        assert!(teams.is_empty());
    }

    #[test]
    fn list_teams_finds_created() {
        let (_tmp, ws) = setup();

        let members = vec![TeamMember {
            id: "a".into(),
            role: "r".into(),
            endpoint: None,
        }];
        create_team(&ws, "T1", members, None).unwrap();

        let teams = list_teams(&ws).unwrap();
        assert_eq!(teams.len(), 1);
        assert_eq!(teams[0].name, "T1");
    }

    #[test]
    fn delete_team_works() {
        let (_tmp, ws) = setup();

        let members = vec![TeamMember {
            id: "a".into(),
            role: "r".into(),
            endpoint: None,
        }];
        let team = create_team(&ws, "Del", members, None).unwrap();
        assert!(ws.team(&team.id).path().exists());

        delete_team(&ws, &team.id).unwrap();
        assert!(!ws.team(&team.id).path().exists());
    }

    #[test]
    fn delete_nonexistent_fails() {
        let (_tmp, ws) = setup();
        let result = delete_team(&ws, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn set_leader_to_existing_member() {
        let (_tmp, ws) = setup();

        let members = vec![
            TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
            TeamMember { id: "bob".into(), role: "dev".into(), endpoint: None },
        ];
        let team = create_team(&ws, "SetLeader", members, Some("alice")).unwrap();
        assert_eq!(team.leader, "alice");

        set_leader(&ws, &team.id, "bob").unwrap();
        let loaded = load_team(&ws, &team.id).unwrap();
        assert_eq!(loaded.leader, "bob");
    }

    #[test]
    fn set_leader_non_member_fails() {
        let (_tmp, ws) = setup();

        let members = vec![
            TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
        ];
        let team = create_team(&ws, "T", members, Some("alice")).unwrap();

        let result = set_leader(&ws, &team.id, "charlie");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("charlie"), "error should mention the invalid member: {msg}");
    }

    #[test]
    fn set_leader_nonexistent_team_fails() {
        let (_tmp, ws) = setup();
        let result = set_leader(&ws, "no-such-team", "alice");
        assert!(result.is_err());
    }

    #[test]
    fn remove_member_from_team_works() {
        let (_tmp, ws) = setup();
        let members = vec![
            TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
            TeamMember { id: "bob".into(), role: "dev".into(), endpoint: None },
        ];
        let team = create_team(&ws, "T", members, Some("alice")).unwrap();

        let result = remove_member_from_team(&ws, &team.id, "bob").unwrap();
        assert!(result.is_some());
        let updated = result.unwrap();
        assert_eq!(updated.members.len(), 1);
        assert_eq!(updated.members[0].id, "alice");
    }

    #[test]
    fn remove_leader_from_team_fails() {
        let (_tmp, ws) = setup();
        let members = vec![
            TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
            TeamMember { id: "bob".into(), role: "dev".into(), endpoint: None },
        ];
        let team = create_team(&ws, "T", members, Some("alice")).unwrap();

        let result = remove_member_from_team(&ws, &team.id, "alice");
        assert!(result.is_err());
    }

    #[test]
    fn remove_last_member_deletes_team() {
        let (_tmp, ws) = setup();
        let members = vec![
            TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
        ];
        let team = create_team(&ws, "T", members, Some("alice")).unwrap();

        // Remove the only member (also leader) → team should be deleted
        let result = remove_member_from_team(&ws, &team.id, "alice").unwrap();
        assert!(result.is_none());
        assert!(!ws.team(&team.id).path().exists());
    }

    #[test]
    fn add_member_to_team_works() {
        let (_tmp, ws) = setup();
        let members = vec![
            TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
        ];
        let team = create_team(&ws, "T", members, Some("alice")).unwrap();

        let updated = add_member_to_team(&ws, &team.id, "bob", "dev").unwrap();
        assert_eq!(updated.members.len(), 2);
        assert!(updated.members.iter().any(|m| m.id == "bob"));
    }

    #[test]
    fn add_duplicate_member_fails() {
        let (_tmp, ws) = setup();
        let members = vec![
            TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
        ];
        let team = create_team(&ws, "T", members, Some("alice")).unwrap();

        let result = add_member_to_team(&ws, &team.id, "alice", "dev");
        assert!(result.is_err());
    }

    #[test]
    fn find_agent_team_works() {
        let (_tmp, ws) = setup();
        let members = vec![
            TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
        ];
        let team = create_team(&ws, "T", members, Some("alice")).unwrap();

        let found = find_agent_team(&ws, "alice").unwrap();
        assert_eq!(found, Some(team.id));

        let not_found = find_agent_team(&ws, "bob").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn default_leader_is_first_member() {
        let (_tmp, ws) = setup();

        let members = vec![TeamMember {
            id: "first".into(),
            role: "r".into(),
            endpoint: None,
        }];
        let team = create_team(&ws, "T", members, None).unwrap();
        assert_eq!(team.leader, "first");
    }
}
