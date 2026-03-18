use clap::Subcommand;
use see_agent_corp::types::{TeamMember, WorkspaceDir};

#[derive(Subcommand)]
pub enum TeamCmd {
    /// Create a new team
    Create {
        /// Team name
        name: String,
        /// Members in "id:role" format (can specify multiple)
        #[arg(short, long, value_delimiter = ',')]
        members: Vec<String>,
        /// Leader agent id
        #[arg(short, long)]
        leader: Option<String>,
    },
    /// List all teams
    List,
    /// Show team details
    Show {
        /// Team id
        id: String,
    },
    /// Delete a team
    Delete {
        /// Team id
        id: String,
    },
}

pub fn run(workspace: &WorkspaceDir, cmd: TeamCmd) {
    match cmd {
        TeamCmd::Create {
            name,
            members,
            leader,
        } => {
            let parsed_members: Vec<TeamMember> = members
                .iter()
                .map(|m| {
                    let parts: Vec<&str> = m.splitn(2, ':').collect();
                    TeamMember {
                        id: parts[0].to_owned(),
                        role: parts.get(1).unwrap_or(&"member").to_string(),
                        endpoint: None,
                    }
                })
                .collect();

            match see_agent_corp::team::create_team(workspace, &name, parsed_members, leader.as_deref()) {
                Ok(team) => println!("Created team: {} ({})", team.name, team.id),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        TeamCmd::List => match see_agent_corp::team::list_teams(workspace) {
            Ok(teams) => {
                if teams.is_empty() {
                    println!("No teams found.");
                    return;
                }
                for t in teams {
                    let member_ids: Vec<&str> = t.members.iter().map(|m| m.id.as_str()).collect();
                    println!(
                        "{} - {} [{:?}] members: {}",
                        t.id,
                        t.name,
                        t.status,
                        member_ids.join(", ")
                    );
                }
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        TeamCmd::Show { id } => match see_agent_corp::team::load_team(workspace, &id) {
            Ok(team) => {
                let json = serde_json::to_string_pretty(&team).unwrap();
                println!("{json}");
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        TeamCmd::Delete { id } => match see_agent_corp::team::delete_team(workspace, &id) {
            Ok(()) => println!("Deleted team: {id}"),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
    }
}
