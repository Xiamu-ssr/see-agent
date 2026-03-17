pub mod agent;
pub mod team;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "see", version, about = "see-agent: AI screen automation")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize workspace (~/.see-agent/)
    Init,
    /// Show workspace status
    Status,
    /// Start the HTTP server
    Serve {
        /// Port to listen on (default: 28789)
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Manage agents
    #[command(subcommand)]
    Agent(agent::AgentCmd),
    /// Manage teams
    #[command(subcommand)]
    Team(team::TeamCmd),
}
