use see::types::WorkspaceDir;
use tracing::info;

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

    // Set up SIGUSR1 handler to wake the inbox drain loop
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

    // Main inbox drain loop
    let inbox_path = agent_dir.inbox();
    let cursor_path = agent_dir.path().join("cursor.json");
    info!(agent = agent_id, inbox = %inbox_path.display(), "entering inbox loop");

    loop {
        // Drain inbox
        if let Ok((steer_msgs, collect_msgs)) =
            see::supervisor::drain_inbox_split(&inbox_path, &cursor_path)
        {
            for msg in &steer_msgs {
                if msg.is_shutdown() {
                    info!(agent = agent_id, "received shutdown, exiting");
                    return;
                }
                info!(agent = agent_id, sender = %msg.sender, "steer: {}", msg.content);
            }

            for msg in &collect_msgs {
                if msg.is_shutdown() {
                    info!(agent = agent_id, "received shutdown, exiting");
                    return;
                }
                info!(agent = agent_id, sender = %msg.sender, "collect: {}", msg.content);
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
