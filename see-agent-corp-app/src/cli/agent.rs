use clap::Subcommand;
use see_agent_corp::types::WorkspaceDir;

#[derive(Subcommand)]
pub enum AgentCmd {
    /// Create a new agent
    Create {
        /// Agent id (auto-generated if omitted)
        #[arg(short, long)]
        id: Option<String>,
        /// Display name
        #[arg(short, long)]
        name: Option<String>,
        /// Emoji icon
        #[arg(short, long)]
        emoji: Option<String>,
    },
    /// List all agents
    List,
    /// Show agent details
    Show {
        /// Agent id
        id: String,
    },
    /// Delete an agent (cascading: removes from team, kills worker)
    Delete {
        /// Agent id
        id: String,
    },
    /// Change an agent's team membership
    Team {
        /// Agent id
        id: String,
        /// Team id to join, or "none" to leave current team
        team_id: String,
    },
}

pub fn run(workspace: &WorkspaceDir, cmd: AgentCmd) {
    match cmd {
        AgentCmd::Create { id, name, emoji } => {
            let agent_id = id.unwrap_or_else(generate_agent_id);
            match see_agent_corp::agent::create_agent(
                workspace,
                &agent_id,
                name.as_deref(),
                emoji.as_deref(),
            ) {
                Ok(def) => println!("Created agent: {}", def.id),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        AgentCmd::List => match see_agent_corp::agent::list_agents(workspace) {
            Ok(agents) => {
                if agents.is_empty() {
                    println!("No agents found.");
                    return;
                }
                for a in agents {
                    let team = a
                        .team_id
                        .as_deref()
                        .map(|t| format!(" [team: {t}]"))
                        .unwrap_or_default();
                    println!("{} {} {}{}", a.emoji, a.id, a.name, team);
                }
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        AgentCmd::Show { id } => match see_agent_corp::agent::load_agent(workspace, &id) {
            Ok(def) => {
                let json = serde_json::to_string_pretty(&def).unwrap();
                println!("{json}");
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        AgentCmd::Delete { id } => {
            if let Err(e) = delete_agent_cascading(workspace, &id) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
            println!("Deleted agent: {id}");
        }
        AgentCmd::Team { id, team_id } => {
            if let Err(e) = change_agent_team(workspace, &id, &team_id) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}

/// Delete agent with cascading: remove from team, kill worker, unassign tasks, delete dir.
fn delete_agent_cascading(workspace: &WorkspaceDir, id: &str) -> see_agent_corp::error::Result<()> {
    // Check if agent is a team leader
    if let Some(team_id) = see_agent_corp::team::find_agent_team(workspace, id)? {
        let team = see_agent_corp::team::load_team(workspace, &team_id)?;
        if team.leader == id {
            return Err(see_agent_corp::error::CorpError::Agent {
                message: format!("agent '{id}' is the leader of team '{team_id}' — change leader first"),
            });
        }
        // Remove from team (may delete team if last member)
        see_agent_corp::team::remove_member_from_team(workspace, &team_id, id)?;
    }

    // Kill worker process if running (read worker.pid)
    kill_worker_by_pid(workspace, id);

    // Unassign tasks in any team taskboard
    unassign_agent_tasks(workspace, id);

    // Delete agent directory
    see_agent_corp::agent::delete_agent(workspace, id)
}

/// Kill worker process via worker.pid file.
fn kill_worker_by_pid(workspace: &WorkspaceDir, agent_id: &str) {
    let pid_path = workspace.agent(agent_id).path().join("worker.pid");
    if let Ok(content) = std::fs::read_to_string(&pid_path)
        && let Ok(pid) = content.trim().parse::<i32>()
    {
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
        }
        let _ = pid;
    }
}

/// Unassign tasks assigned to this agent across all team taskboards.
fn unassign_agent_tasks(workspace: &WorkspaceDir, agent_id: &str) {
    let teams_dir = workspace.teams();
    if !teams_dir.exists() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(teams_dir) {
        for entry in entries.flatten() {
            let tid = entry.file_name().to_string_lossy().to_string();
            let team_dir = workspace.team(&tid);
            let board = see_agent_corp::team::TaskBoard::new(team_dir);
            let _ = board.unassign_agent(agent_id);
        }
    }
}

/// Change an agent's team membership.
fn change_agent_team(workspace: &WorkspaceDir, agent_id: &str, target: &str) -> see_agent_corp::error::Result<()> {
    // Verify agent exists
    let agent_dir = workspace.agent(agent_id);
    if !agent_dir.path().exists() {
        return Err(see_agent_corp::error::CorpError::NotFound {
            what: format!("agent '{agent_id}'"),
        });
    }

    let current_team = see_agent_corp::team::find_agent_team(workspace, agent_id)?;

    if target == "none" {
        // Leave current team
        if let Some(team_id) = current_team {
            see_agent_corp::team::remove_member_from_team(workspace, &team_id, agent_id)?;
            println!("Agent '{agent_id}' left team '{team_id}'");
        } else {
            println!("Agent '{agent_id}' is not in any team");
        }
    } else {
        // Join new team
        // If already in another team, leave first
        if let Some(old_team) = &current_team {
            if old_team == target {
                println!("Agent '{agent_id}' is already in team '{target}'");
                return Ok(());
            }
            see_agent_corp::team::remove_member_from_team(workspace, old_team, agent_id)?;
            println!("Agent '{agent_id}' left team '{old_team}'");
        }
        see_agent_corp::team::add_member_to_team(workspace, target, agent_id, "member")?;
        println!("Agent '{agent_id}' joined team '{target}'");
    }

    Ok(())
}

fn generate_agent_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:06x}", nanos % 0xFFFFFF)
}
