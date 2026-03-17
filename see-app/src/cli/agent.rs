use clap::Subcommand;
use see::types::WorkspaceDir;

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
    /// Delete an agent
    Delete {
        /// Agent id
        id: String,
    },
}

pub fn run(workspace: &WorkspaceDir, cmd: AgentCmd) {
    match cmd {
        AgentCmd::Create { id, name, emoji } => {
            let agent_id = id.unwrap_or_else(generate_agent_id);
            match see::agent::create_agent(
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
        AgentCmd::List => match see::agent::list_agents(workspace) {
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
        AgentCmd::Show { id } => match see::agent::load_agent(workspace, &id) {
            Ok(def) => {
                let json = serde_json::to_string_pretty(&def).unwrap();
                println!("{json}");
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        AgentCmd::Delete { id } => match see::agent::delete_agent(workspace, &id) {
            Ok(()) => println!("Deleted agent: {id}"),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
    }
}

fn generate_agent_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // 6-char alphanumeric-ish from timestamp
    format!("{:06x}", nanos % 0xFFFFFF)
}
