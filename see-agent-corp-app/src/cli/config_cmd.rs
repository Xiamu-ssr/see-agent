use see_agent_corp::types::WorkspaceDir;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Show current configuration
    Show,
    /// Show configuration file path
    Path,
}

pub fn run(workspace: &WorkspaceDir, cmd: ConfigCmd) {
    match cmd {
        ConfigCmd::Show => {
            let config = see_agent_corp::config::load_config(workspace).unwrap_or_default();
            let json = serde_json::to_string_pretty(&config).unwrap();
            println!("{json}");
        }
        ConfigCmd::Path => {
            println!("{}", workspace.config().display());
        }
    }
}
