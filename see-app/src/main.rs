mod cli;
mod server;

use clap::Parser;
use cli::{Cli, Commands};
use see::config::{ensure_workspace, load_config};
use see::consts::VERSION;
use see::types::WorkspaceDir;

fn main() {
    let cli = Cli::parse();
    let workspace = WorkspaceDir::new(see::config::resolve_workspace_root());

    // Ensure workspace exists for all commands
    if let Err(e) = ensure_workspace(&workspace) {
        eprintln!("Failed to initialize workspace: {e}");
        std::process::exit(1);
    }

    match cli.command {
        Commands::Init => {
            // Also create system agents on first init
            create_system_agents(&workspace);
            println!("Workspace initialized at {}", workspace.path().display());
        }
        Commands::Status => {
            let config = load_config(&workspace).unwrap_or_default();
            println!("see-agent v{VERSION}");
            println!("Workspace: {}", workspace.path().display());
            println!("LLM model: {}", config.llm.model);

            let agents = see::agent::list_agents(&workspace).unwrap_or_default();
            println!("Agents: {}", agents.len());

            let teams = see::team::list_teams(&workspace).unwrap_or_default();
            println!("Teams: {}", teams.len());
        }
        Commands::Serve { port } => {
            tracing_subscriber::fmt::init();
            // Ensure system agents exist before serving
            create_system_agents(&workspace);
            let state = server::AppState::new(workspace);
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(server::serve(state, port));
        }
        Commands::Agent(cmd) => cli::agent::run(&workspace, cmd),
        Commands::Team(cmd) => cli::team::run(&workspace, cmd),
        Commands::Config(cmd) => cli::config_cmd::run(&workspace, cmd),
        Commands::Send {
            agent_id,
            message,
            steer,
        } => cli::send::run(&workspace, &agent_id, &message, steer),
        Commands::Worker {
            agent_id,
            workspace_path,
        } => {
            tracing_subscriber::fmt::init();
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(cli::worker::run(&agent_id, &workspace_path));
        }
    }
}

/// Create system agents if they don't already exist.
///
/// The "system" agent handles management commands (status queries, etc.).
fn create_system_agents(workspace: &WorkspaceDir) {
    let system_dir = workspace.agent("system");
    if !system_dir.path().exists() {
        let _ = see::agent::create_agent(workspace, "system", Some("System"), Some("🔧"));
    }
}
