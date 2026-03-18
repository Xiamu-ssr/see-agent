pub mod agent;
pub mod config_cmd;
pub mod daemon;
pub mod send;
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
    /// Initialize workspace (~/.agentcorp/)
    Init,
    /// Show workspace status
    Status,
    /// Start the HTTP server (foreground)
    Serve {
        /// Port to listen on (default: 28789)
        #[arg(short, long)]
        port: Option<u16>,
        /// PID file path (internal, used by daemon mode)
        #[arg(long, hide = true)]
        pid_file: Option<String>,
    },
    /// Manage agents
    #[command(subcommand)]
    Agent(agent::AgentCmd),
    /// Manage teams
    #[command(subcommand)]
    Team(team::TeamCmd),
    /// Manage configuration
    #[command(subcommand)]
    Config(config_cmd::ConfigCmd),
    /// Start server as background daemon
    Start {
        /// Port to listen on (default: 28789)
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Stop running daemon
    Stop,
    /// Restart daemon (stop + start)
    Restart {
        /// Port to listen on (default: 28789)
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Send a message to an agent
    Send {
        /// Agent id
        agent_id: String,
        /// Message content
        message: String,
        /// Send as steer (high priority) message
        #[arg(short, long)]
        steer: bool,
    },
    /// Run as a worker process (used by supervisor, not invoked directly)
    #[command(hide = true)]
    Worker {
        /// Agent id
        agent_id: String,
        /// Workspace path
        workspace_path: String,
    },
}
