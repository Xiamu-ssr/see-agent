mod cli;
mod server;

use clap::Parser;
use cli::{Cli, Commands};
use see_agent_corp::config::ensure_workspace;
use see_agent_corp::consts::VERSION;
use see_agent_corp::types::WorkspaceDir;

fn main() {
    let cli = Cli::parse();
    let workspace = WorkspaceDir::new(see_agent_corp::config::resolve_workspace_root());

    // Ensure workspace exists (skip for worker subcommand — workspace already initialized by serve)
    let needs_init = !matches!(cli.command, Commands::Worker { .. });
    if needs_init {
        if let Err(e) = ensure_workspace(&workspace) {
            eprintln!("Failed to initialize workspace: {e}");
            std::process::exit(1);
        }
    }

    match cli.command {
        Commands::Start { port } => cli::daemon::start(&workspace, port),
        Commands::Stop => cli::daemon::stop(&workspace),
        Commands::Restart { port } => cli::daemon::restart(&workspace, port),
        Commands::Status => {
            let config =
                see_agent_corp::config::load_config(&workspace).unwrap_or_default();
            println!("see-agent-corp v{VERSION}");
            println!("Workspace: {}", workspace.path().display());
            println!("LLM model: {}", config.llm.model);

            let agents =
                see_agent_corp::agent::list_agents(&workspace).unwrap_or_default();
            println!("Agents: {}", agents.len());

            let teams =
                see_agent_corp::team::list_teams(&workspace).unwrap_or_default();
            println!("Teams: {}", teams.len());

            if let Some(pid) = cli::daemon::check_stale_pid(&workspace.server_pid()) {
                println!("Server: running (PID {pid})");
            } else {
                println!("Server: stopped");
            }
        }
        Commands::Serve { port, pid_file } => {
            tracing_subscriber::fmt::init();
            let state = server::AppState::new(workspace);
            let pid_path = pid_file.map(std::path::PathBuf::from);
            let rt =
                tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(server::serve(state, port, pid_path));
        }
        Commands::Agent(cmd) => cli::agent::run(&workspace, cmd),
        Commands::Team(cmd) => cli::team::run(&workspace, cmd),
        Commands::Worker {
            agent_id,
            workspace_path,
        } => {
            tracing_subscriber::fmt::init();
            let rt =
                tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(cli::worker::run(&agent_id, &workspace_path));
        }
    }
}
