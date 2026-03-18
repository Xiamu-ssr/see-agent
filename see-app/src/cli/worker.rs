use std::sync::Arc;

use tracing::info;

use see::agent::{AgentLoop, ConversationContext};
use see::brain::{build_system_prompt, OpenAiBrain, PromptContext};
use see::config::load_agent_config;
use see::tool::{register_builtin_tools, ToolContext, ToolRegistry};
use see::types::WorkspaceDir;

/// Run as a worker process for a single agent.
///
/// This is not invoked by users directly — the supervisor spawns it:
/// `see-app worker <agent_id> <workspace_path>`
pub async fn run(agent_id: &str, workspace_path: &str) {
    let workspace = WorkspaceDir::new(std::path::Path::new(workspace_path));
    let agent_dir = workspace.agent(agent_id);

    if !agent_dir.path().exists() {
        eprintln!("Agent directory not found: {}", agent_dir.path().display());
        std::process::exit(1);
    }

    info!(agent = agent_id, "worker starting");

    // 1. Load merged config: default < config.json < agent.json < env vars
    let config = match load_agent_config(&workspace, agent_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    // 2. Create Brain (LLM client)
    let brain = Box::new(OpenAiBrain::new(&config.llm));

    // 3. Create Eye (screen capture)
    let eye: Arc<dyn see::eye::Eye> = create_eye();

    // 4. Look up team membership (optional)
    let team_dir = find_agent_team(&workspace, agent_id);

    // 5. Create ToolContext and register builtin tools
    let tool_ctx = Arc::new(ToolContext {
        agent_id: agent_id.to_owned(),
        agent_dir: agent_dir.clone(),
        team_dir,
        eye: eye.clone(),
        workspace: workspace.clone(),
    });

    let mut registry = ToolRegistry::new();
    register_builtin_tools(&mut registry, tool_ctx);

    // 6. Create AgentLoop
    let mut agent_loop = AgentLoop::new(
        brain,
        eye,
        registry,
        config.clone(),
        agent_id.to_owned(),
    );

    // 7. Build system prompt
    let prompt_ctx = PromptContext {
        agent_dir: agent_dir.path(),
        max_steps: config.agent.max_steps,
        skills: &[],
        team: None,
    };
    let system_prompt = build_system_prompt(&prompt_ctx);

    // 8. Create conversation context
    let mut conv_ctx =
        ConversationContext::new(&system_prompt, config.screen.max_images as usize, None);

    // 9. Set up SIGUSR1 handler to wake the inbox drain loop
    let (wake_tx, mut wake_rx) =
        tokio::sync::mpsc::channel::<()>(see::consts::WORKER_WAKE_CHANNEL_SIZE);

    #[cfg(unix)]
    {
        let tx = wake_tx.clone();
        tokio::spawn(async move {
            let mut sig =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::user_defined1())
                    .expect("failed to register SIGUSR1 handler");
            loop {
                sig.recv().await;
                let _ = tx.send(()).await;
            }
        });
    }

    // 10. Main inbox drain loop
    let inbox_path = agent_dir.inbox();
    let cursor_path = agent_dir.inbox_cursor();
    info!(agent = agent_id, inbox = %inbox_path.display(), "entering inbox loop");

    loop {
        // Drain inbox
        if let Ok((steer_msgs, collect_msgs)) =
            see::supervisor::drain_inbox_split(&inbox_path, &cursor_path)
        {
            // Check for shutdown
            for msg in steer_msgs.iter().chain(collect_msgs.iter()) {
                if msg.is_shutdown() {
                    info!(agent = agent_id, "received shutdown, exiting");
                    return;
                }
            }

            // Convert messages to JSON values for AgentLoop
            let all_msgs: Vec<serde_json::Value> = steer_msgs
                .iter()
                .chain(collect_msgs.iter())
                .filter_map(|m| serde_json::to_value(m).ok())
                .collect();

            if !all_msgs.is_empty() {
                info!(
                    agent = agent_id,
                    count = all_msgs.len(),
                    "processing {} inbox messages",
                    all_msgs.len()
                );

                // Run one turn of the agent loop
                agent_loop
                    .run_one_turn(&mut conv_ctx, &all_msgs, &system_prompt)
                    .await;

                info!(agent = agent_id, "turn complete, returning to idle");
            }
        }

        // Wait for wake signal or timeout
        let timeout =
            tokio::time::Duration::from_secs(see::consts::WORKER_SIGNAL_TIMEOUT_SECS);
        let _ = tokio::time::timeout(timeout, wake_rx.recv()).await;
    }

    // Suppress unreachable warning — loop is infinite but has `return` exits
    #[allow(unreachable_code)]
    {
        let _ = wake_tx;
    }
}

/// Create the platform-appropriate Eye implementation.
#[cfg(target_os = "macos")]
fn create_eye() -> Arc<dyn see::eye::Eye> {
    Arc::new(see::eye::MacEye::new())
}

#[cfg(target_os = "linux")]
fn create_eye() -> Arc<dyn see::eye::Eye> {
    Arc::new(see::eye::LinuxEye)
}

/// Find the team directory for an agent by scanning all teams.
fn find_agent_team(
    workspace: &WorkspaceDir,
    agent_id: &str,
) -> Option<see::types::paths::TeamDir> {
    let teams_dir = workspace.teams();
    if !teams_dir.exists() {
        return None;
    }

    let entries = match std::fs::read_dir(&teams_dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let team_dir = workspace.team(&name.to_string_lossy());
        let team_json = team_dir.team_json();
        if team_json.exists()
            && let Ok(content) = std::fs::read_to_string(&team_json)
            && content.contains(agent_id)
        {
            return Some(team_dir);
        }
    }

    None
}
