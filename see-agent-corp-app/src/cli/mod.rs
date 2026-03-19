pub mod agent;
pub mod daemon;
pub mod team;
pub mod worker;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "see-agent-corp", version, about = "agentcorp: AI agent orchestration")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start server (initializes workspace + system agent if needed)
    Start {
        /// Port to listen on (default: 28789)
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Stop running server
    Stop,
    /// Restart server (stop + start)
    Restart {
        /// Port to listen on (default: 28789)
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Show workspace status
    Status,
    /// Manage agents
    #[command(subcommand)]
    Agent(agent::AgentCmd),
    /// Manage teams
    #[command(subcommand)]
    Team(team::TeamCmd),
    /// Run as a worker process (used by supervisor, not invoked directly)
    #[command(hide = true)]
    Worker {
        /// Agent id
        agent_id: String,
        /// Workspace path
        workspace_path: String,
    },
    /// Start HTTP server in foreground (used internally by daemon)
    #[command(hide = true)]
    Serve {
        /// Port to listen on
        #[arg(short, long)]
        port: Option<u16>,
        /// PID file path
        #[arg(long, hide = true)]
        pid_file: Option<String>,
    },
}
