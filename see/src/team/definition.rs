use chrono::Utc;

use crate::error::{Result, SeeError};
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
    };

    write_json(&team_dir.team_json(), &definition)?;

    // Initialize empty tasklist
    write_json(&team_dir.tasklist(), &Vec::<serde_json::Value>::new())?;

    Ok(definition)
}

/// Load a team definition from disk.
pub fn load_team(workspace: &WorkspaceDir, id: &str) -> Result<TeamDefinition> {
    let team_dir = workspace.team(id);
    if !team_dir.team_json().exists() {
        return Err(SeeError::NotFound {
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
        });
    }

    teams.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(teams)
}

/// Delete a team by id.
pub fn delete_team(workspace: &WorkspaceDir, id: &str) -> Result<()> {
    let team_dir = workspace.team(id);
    if !team_dir.path().exists() {
        return Err(SeeError::NotFound {
            what: format!("team '{id}'"),
        });
    }
    std::fs::remove_dir_all(team_dir.path())?;
    Ok(())
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
