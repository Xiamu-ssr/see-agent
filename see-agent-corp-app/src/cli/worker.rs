use std::sync::Arc;

use tracing::info;

use see_agent_corp::agent::{AgentLoop, ConversationContext};
use see_agent_corp::brain::{build_system_prompt, OpenAiBrain, PromptContext, TeamContext};
use see_agent_corp::io::read_json;
use see_agent_corp::session::SessionStore;
use see_agent_corp::types::TeamDefinition;
use see_agent_corp::config::load_agent_config;
use see_agent_corp::skill::{filter_skills, gate_skills, load_skills};
use see_agent_corp::tool::{register_builtin_tools, ToolContext, ToolRegistry};
use see_agent_corp::types::WorkspaceDir;

/// Run as a worker process for a single agent.
///
/// This is not invoked by users directly — the supervisor spawns it:
/// `see-agent-corp worker <agent_id> <workspace_path>`
pub async fn run(agent_id: &str, workspace_path: &str) {
    let workspace = WorkspaceDir::new(std::path::Path::new(workspace_path));
    let agent_dir = workspace.agent(agent_id);

    if !agent_dir.path().exists() {
        eprintln!("Agent directory not found: {}", agent_dir.path().display());
        std::process::exit(1);
    }

    info!(agent = agent_id, "worker starting");

    // Write PID file so other agents can send us SIGUSR1
    let pid = std::process::id();
    let _ = std::fs::write(agent_dir.worker_pid(), pid.to_string());

    // 1. Look up team membership (needed for config merge chain)
    let team_dir = find_agent_team(&workspace, agent_id);
    let team_id = team_dir.as_ref().and_then(|td| {
        td.path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    });

    // 2. Load merged config: default < config.json < team.json.config < agent.json < env vars
    let config = match load_agent_config(&workspace, agent_id, team_id.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Failed to load config: {e}");
            eprintln!("{msg}");
            let _ = std::fs::write(agent_dir.path().join("worker_error.log"), &msg);
            std::process::exit(1);
        }
    };

    // 3. Create Brain (LLM client)
    let brain = Box::new(OpenAiBrain::new(&config.llm));

    // 4. Create Eye (screen capture)
    let eye: Arc<dyn see_agent_corp::eye::Eye> = create_eye();

    // 5. Create ToolContext and register builtin tools
    let shared_dir = team_dir.as_ref().map(|td| td.shared());
    let is_team_agent = team_dir.is_some();
    let heartbeat_team_dir = team_dir.clone();
    // Create wake_fn: reads target agent's worker.pid and sends SIGUSR1
    let wake_workspace = workspace.clone();
    let wake_fn: Option<see_agent_corp::tool::WakeFn> = Some(Arc::new(move |target_id: &str| {
        let target_dir = wake_workspace.agent(target_id);
        let pid_path = target_dir.worker_pid();
        if let Ok(content) = std::fs::read_to_string(&pid_path)
            && let Ok(pid) = content.trim().parse::<u32>()
        {
            wake_process(pid);
        }
    }));

    let tool_ctx = Arc::new(ToolContext {
        agent_id: agent_id.to_owned(),
        agent_dir: agent_dir.clone(),
        team_dir,
        eye: eye.clone(),
        workspace: workspace.clone(),
        shared_dir: shared_dir.clone(),
        wake_fn,
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

    // 6b. Screenshots saved to disk (path-ref mode) to avoid base64 bloat in memory
    let session_dir = agent_dir.session();
    let screenshots_dir = session_dir.screenshots();
    agent_loop.set_screenshots_dir(screenshots_dir);

    // 6c. Set inbox paths for real-time steer injection during reasoning loop
    agent_loop.set_inbox_paths(agent_dir.inbox(), agent_dir.inbox_cursor());

    // 6c. Session store for persisting messages to disk (visible in chat UI)
    let _ = std::fs::create_dir_all(session_dir.path());
    let _ = std::fs::create_dir_all(session_dir.screenshots());
    if !session_dir.messages().exists() {
        let _ = std::fs::write(session_dir.messages(), "");
    }
    let session_store = SessionStore::new(session_dir);
    agent_loop.set_session_store(session_store);

    // 7. Build system prompt (with team context if agent belongs to a team)
    let team_def: Option<TeamDefinition> = heartbeat_team_dir.as_ref().and_then(|td| {
        read_json::<TeamDefinition>(&td.team_json()).ok()
    });
    let shared_dir_str = shared_dir.as_ref().map(|p| p.to_string_lossy().into_owned());
    let team_context = team_def.as_ref().map(|def| {
        let my_role = if def.leader == agent_id { "leader" } else { "worker" };
        TeamContext {
            name: &def.name,
            my_role,
            leader_id: &def.leader,
            members: &def.members,
            shared_dir: shared_dir_str.as_deref(),
        }
    });
    // 7b. Load skills (built-in defaults + config extras + agent extras)
    let agent_extra_dirs = read_json::<see_agent_corp::types::AgentDefinition>(&agent_dir.agent_json())
        .ok()
        .and_then(|def| def.skills)
        .map(|s| s.dirs);
    let skill_dirs = see_agent_corp::skill::resolve_skill_dirs(
        &workspace,
        agent_id,
        &config.skills.dirs,
        agent_extra_dirs.as_deref(),
    );
    let skills = gate_skills(filter_skills(
        load_skills(&skill_dirs),
        &config.skills.disabled,
    ));
    info!(count = skills.len(), "loaded skills for agent");

    let prompt_ctx = PromptContext {
        agent_dir: agent_dir.path(),
        max_steps: config.agent.max_steps,
        skills: &skills,
        team: team_context,
    };
    let system_prompt = build_system_prompt(&prompt_ctx);

    // 8. Create conversation context (with restore if previous session exists)
    let mut conv_ctx = {
        let session_dir_for_restore = agent_dir.session();
        let mut restore_store = SessionStore::new(session_dir_for_restore);

        if restore_store.dir().messages().exists() {
            match restore_store.read_for_restore() {
                Ok((Some(summary), kept_msgs)) if !kept_msgs.is_empty() => {
                    info!(
                        agent = agent_id,
                        kept = kept_msgs.len(),
                        "restoring session from disk"
                    );
                    let mut ctx =
                        ConversationContext::for_restore(config.agent.compact.image_high_count as usize,
                        config.agent.compact.image_low_count as usize);
                    ctx.push_raw(serde_json::json!({
                        "role": "system",
                        "content": &system_prompt
                    }));
                    ctx.inject_summary(&summary);
                    for msg in &kept_msgs {
                        if let Some(openai_msg) = session_msg_to_openai(msg) {
                            ctx.push_raw(openai_msg);
                        }
                    }
                    ctx
                }
                Ok((None, kept_msgs)) if !kept_msgs.is_empty() => {
                    info!(
                        agent = agent_id,
                        kept = kept_msgs.len(),
                        "restoring session (no compact summary)"
                    );
                    let mut ctx =
                        ConversationContext::for_restore(config.agent.compact.image_high_count as usize,
                        config.agent.compact.image_low_count as usize);
                    ctx.push_raw(serde_json::json!({
                        "role": "system",
                        "content": &system_prompt
                    }));
                    for msg in &kept_msgs {
                        if let Some(openai_msg) = session_msg_to_openai(msg) {
                            ctx.push_raw(openai_msg);
                        }
                    }
                    ctx
                }
                _ => {
                    info!(agent = agent_id, "no previous session, starting fresh");
                    ConversationContext::new(
                        &system_prompt,
                        config.agent.compact.image_high_count as usize,
                        config.agent.compact.image_low_count as usize,
                        None,
                    )
                }
            }
        } else {
            ConversationContext::new(&system_prompt, config.agent.compact.image_high_count as usize,
                        config.agent.compact.image_low_count as usize, None)
        }
    };

    // 9. Set up SIGUSR1 handler to wake the inbox drain loop
    let (wake_tx, mut wake_rx) =
        tokio::sync::mpsc::channel::<()>(see_agent_corp::consts::WORKER_WAKE_CHANNEL_SIZE);

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

    // 10. Config hot-reload tracking
    let config_path = workspace.config();
    let mut config_mtime = std::fs::metadata(&config_path)
        .and_then(|m| m.modified())
        .ok();
    let mut _config = config;
    let mut system_prompt = system_prompt;

    // 11. Main inbox drain loop
    let inbox_path = agent_dir.inbox();
    let cursor_path = agent_dir.inbox_cursor();
    info!(agent = agent_id, inbox = %inbox_path.display(), "entering inbox loop");

    let mut is_first_drain = true;

    loop {
        // Config hot-reload: check mtime before each iteration
        if let Ok(meta) = std::fs::metadata(&config_path)
            && let Ok(new_mtime) = meta.modified()
            && config_mtime.is_none_or(|old| new_mtime > old)
            && config_mtime.is_some() // skip first load (already loaded above)
        {
            info!(agent = agent_id, "config.json changed, hot-reloading");
            match load_agent_config(&workspace, agent_id, team_id.as_deref()) {
                Ok(new_config) => {
                    let new_brain = Box::new(OpenAiBrain::new(&new_config.llm));

                    // Rebuild system prompt with new config
                    let new_agent_extra = read_json::<see_agent_corp::types::AgentDefinition>(&agent_dir.agent_json())
                        .ok()
                        .and_then(|def| def.skills)
                        .map(|s| s.dirs);
                    let new_skill_dirs = see_agent_corp::skill::resolve_skill_dirs(
                        &workspace,
                        agent_id,
                        &new_config.skills.dirs,
                        new_agent_extra.as_deref(),
                    );
                    let new_skills = gate_skills(filter_skills(
                        load_skills(&new_skill_dirs),
                        &new_config.skills.disabled,
                    ));

                    let new_team_context = team_def.as_ref().map(|def| {
                        let my_role = if def.leader == agent_id { "leader" } else { "worker" };
                        TeamContext {
                            name: &def.name,
                            my_role,
                            leader_id: &def.leader,
                            members: &def.members,
                            shared_dir: shared_dir_str.as_deref(),
                        }
                    });

                    let new_prompt_ctx = PromptContext {
                        agent_dir: agent_dir.path(),
                        max_steps: new_config.agent.max_steps,
                        skills: &new_skills,
                        team: new_team_context,
                    };
                    system_prompt = build_system_prompt(&new_prompt_ctx);

                    agent_loop.hot_reload(new_config.clone(), new_brain);
                    _config = new_config;
                    info!(agent = agent_id, "config hot-reload complete");
                }
                Err(e) => {
                    tracing::warn!(agent = agent_id, "config hot-reload failed: {e}");
                }
            }
            config_mtime = Some(new_mtime);
        }
        // Drain inbox
        if let Ok((steer_msgs, collect_msgs)) =
            see_agent_corp::supervisor::drain_inbox_split(&inbox_path, &cursor_path)
        {
            // Bug 54: On first drain, filter out historical shutdown messages
            // (left over from a previous worker's lifecycle)
            let (steer_msgs, collect_msgs) = if is_first_drain {
                is_first_drain = false;
                (
                    steer_msgs.into_iter().filter(|m| !m.is_shutdown()).collect::<Vec<_>>(),
                    collect_msgs.into_iter().filter(|m| !m.is_shutdown()).collect::<Vec<_>>(),
                )
            } else {
                (steer_msgs, collect_msgs)
            };

            // Check for shutdown
            for msg in steer_msgs.iter().chain(collect_msgs.iter()) {
                if msg.is_shutdown() {
                    info!(agent = agent_id, "received shutdown, exiting");
                    let _ = std::fs::remove_file(agent_dir.worker_pid());
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
        // All agents use the heartbeat interval; SIGUSR1 provides instant wake on new messages.
        let timeout = tokio::time::Duration::from_secs(see_agent_corp::consts::WORKER_HEARTBEAT_SECS);
        let wake_result = tokio::time::timeout(timeout, wake_rx.recv()).await;

        // On heartbeat timeout: drain inbox (all agents) + check TaskBoard (team agents)
        if wake_result.is_err() {
            // Re-drain inbox in case SIGUSR1 was missed
            if let Ok((steer, collect)) =
                see_agent_corp::supervisor::drain_inbox_split(&inbox_path, &cursor_path)
            {
                for msg in steer.iter().chain(collect.iter()) {
                    if msg.is_shutdown() {
                        info!(agent = agent_id, "heartbeat: received shutdown, exiting");
                        let _ = std::fs::remove_file(agent_dir.worker_pid());
                        return;
                    }
                }

                let heartbeat_inbox: Vec<serde_json::Value> = steer
                    .iter()
                    .chain(collect.iter())
                    .filter_map(|m| serde_json::to_value(m).ok())
                    .collect();

                if !heartbeat_inbox.is_empty() {
                    info!(agent = agent_id, count = heartbeat_inbox.len(), "heartbeat: draining missed inbox messages");
                    agent_loop
                        .run_one_turn(&mut conv_ctx, &heartbeat_inbox, &system_prompt)
                        .await;
                }
            }

            // Team agents: also check TaskBoard for pending tasks
            if is_team_agent
                && let Some(ref td) = heartbeat_team_dir
            {
                let board = see_agent_corp::team::TaskBoard::new(td.clone());
                let has_work = board
                    .list_tasks(Some(see_agent_corp::types::TaskStatus::Pending))
                    .map(|tasks| {
                        tasks.iter().any(|t| {
                            t.assigned_to.is_none()
                                || t.assigned_to.as_deref() == Some(agent_id)
                        })
                    })
                    .unwrap_or(false);

                if has_work {
                    info!(agent = agent_id, "heartbeat: found pending tasks, waking agent");
                    let heartbeat_msg = serde_json::json!({
                        "content": "Heartbeat: there are pending tasks on the task board. Check list_tasks and work on available tasks.",
                        "from": "system",
                        "priority": "steer"
                    });
                    agent_loop
                        .run_one_turn(&mut conv_ctx, &[heartbeat_msg], &system_prompt)
                        .await;
                }
            }
        }
    }

    // Suppress unreachable warning — loop is infinite but has `return` exits
    #[allow(unreachable_code)]
    {
        let _ = wake_tx;
    }
}

/// Convert a SessionMessage back to OpenAI chat format for context restore.
fn session_msg_to_openai(msg: &see_agent_corp::types::SessionMessage) -> Option<serde_json::Value> {
    use see_agent_corp::types::SessionMessageType;
    match msg.msg_type {
        SessionMessageType::UserTask | SessionMessageType::UserReply => {
            let text = msg
                .data
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Some(serde_json::json!({"role": "user", "content": text}))
        }
        SessionMessageType::Assistant => {
            // Restore assistant message with content; skip tool_calls to avoid
            // orphaned tool_call IDs confusing the API on restore.
            let mut m = serde_json::json!({"role": "assistant"});
            if let Some(content) = msg.data.get("content") {
                m["content"] = content.clone();
            }
            Some(m)
        }
        SessionMessageType::SystemHint => {
            let text = msg
                .data
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Some(serde_json::json!({"role": "user", "content": text}))
        }
        // ToolResult without matching tool_calls confuses the API. Skip.
        // Screenshot path-refs may not exist after restart. Skip.
        _ => None,
    }
}

/// Create the platform-appropriate Eye implementation.
#[cfg(target_os = "macos")]
fn create_eye() -> Arc<dyn see_agent_corp::eye::Eye> {
    Arc::new(see_agent_corp::eye::MacEye::new())
}

#[cfg(target_os = "linux")]
fn create_eye() -> Arc<dyn see_agent_corp::eye::Eye> {
    Arc::new(see_agent_corp::eye::LinuxEye)
}

/// Send SIGUSR1 to a process by PID.
#[cfg(unix)]
fn wake_process(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    if pid == 0 {
        return;
    }
    if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGUSR1) {
        tracing::warn!(pid, "failed to send SIGUSR1 to wake target: {e}");
    }
}

#[cfg(not(unix))]
fn wake_process(_pid: u32) {}

/// Find the team directory for an agent by scanning all teams.
pub(crate) fn find_agent_team(
    workspace: &WorkspaceDir,
    agent_id: &str,
) -> Option<see_agent_corp::types::paths::TeamDir> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_workspace() -> (tempfile::TempDir, WorkspaceDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        see_agent_corp::config::ensure_workspace(&ws).unwrap();
        (tmp, ws)
    }

    #[test]
    fn find_agent_team_returns_none_when_no_teams() {
        let (_tmp, ws) = make_workspace();
        assert!(find_agent_team(&ws, "alice").is_none());
    }

    #[test]
    fn find_agent_team_finds_matching_team() {
        let (_tmp, ws) = make_workspace();

        // Create a team that references "alice"
        let team_dir = ws.team("my-team");
        std::fs::create_dir_all(team_dir.path()).unwrap();
        let team_json = serde_json::json!({
            "id": "my-team",
            "name": "My Team",
            "members": [
                {"id": "alice", "role": "leader"},
                {"id": "bob", "role": "dev"}
            ],
            "leader": "alice",
            "status": "running",
            "created_at": "2025-01-01T00:00:00Z"
        });
        std::fs::write(team_dir.team_json(), serde_json::to_string(&team_json).unwrap()).unwrap();

        let result = find_agent_team(&ws, "alice");
        assert!(result.is_some());
        let found = result.unwrap();
        assert!(found.path().ends_with("my-team"));
    }

    #[test]
    fn find_agent_team_returns_none_for_nonmember() {
        let (_tmp, ws) = make_workspace();

        let team_dir = ws.team("my-team");
        std::fs::create_dir_all(team_dir.path()).unwrap();
        let team_json = serde_json::json!({
            "id": "my-team",
            "name": "My Team",
            "members": [{"id": "bob", "role": "dev"}],
            "leader": "bob",
            "status": "running",
            "created_at": "2025-01-01T00:00:00Z"
        });
        std::fs::write(team_dir.team_json(), serde_json::to_string(&team_json).unwrap()).unwrap();

        assert!(find_agent_team(&ws, "charlie").is_none());
    }

    #[test]
    fn system_prompt_includes_team_context_for_leader() {
        use see_agent_corp::brain::{build_system_prompt, PromptContext, TeamContext};
        use see_agent_corp::types::TeamMember;

        let (_tmp, ws) = make_workspace();

        let agent_dir = ws.agent("alice");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::write(agent_dir.path().join("IDENTITY.md"), "I am Alice.").unwrap();

        let team = see_agent_corp::team::create_team(
            &ws,
            "Alpha Team",
            vec![
                TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
                TeamMember { id: "bob".into(), role: "dev".into(), endpoint: None },
            ],
            Some("alice"),
        ).unwrap();

        let team_dir = ws.team(&team.id);
        let def: TeamDefinition = see_agent_corp::io::read_json(&team_dir.team_json()).unwrap();
        let shared_path = team_dir.shared().to_string_lossy().into_owned();

        let my_role = if def.leader == "alice" { "leader" } else { "worker" };
        let team_ctx = TeamContext {
            name: &def.name,
            my_role,
            leader_id: &def.leader,
            members: &def.members,
            shared_dir: Some(&shared_path),
        };

        let prompt_ctx = PromptContext {
            agent_dir: agent_dir.path(),
            max_steps: 50,
            skills: &[],
            team: Some(team_ctx),
        };

        let prompt = build_system_prompt(&prompt_ctx);
        assert!(prompt.contains("<TEAM_CONTEXT>"), "prompt should contain TEAM_CONTEXT block");
        assert!(prompt.contains("Alpha Team"), "prompt should contain team name");
        assert!(prompt.contains("alice"), "prompt should contain leader id");
        assert!(prompt.contains("bob (dev)"), "prompt should list members");
        assert!(prompt.contains("的领导（leader）"), "leader should get leader instructions");
        assert!(prompt.contains("Team Shared Workspace"), "prompt should mention shared workspace");
    }

    #[test]
    fn system_prompt_worker_role_for_non_leader() {
        use see_agent_corp::brain::{build_system_prompt, PromptContext, TeamContext};
        use see_agent_corp::types::TeamMember;

        let (_tmp, ws) = make_workspace();

        let agent_dir = ws.agent("bob");
        std::fs::create_dir_all(agent_dir.path()).unwrap();
        std::fs::write(agent_dir.path().join("IDENTITY.md"), "I am Bob.").unwrap();

        let team = see_agent_corp::team::create_team(
            &ws,
            "Beta Team",
            vec![
                TeamMember { id: "alice".into(), role: "leader".into(), endpoint: None },
                TeamMember { id: "bob".into(), role: "dev".into(), endpoint: None },
            ],
            Some("alice"),
        ).unwrap();

        let team_dir = ws.team(&team.id);
        let def: TeamDefinition = see_agent_corp::io::read_json(&team_dir.team_json()).unwrap();

        let my_role = if def.leader == "bob" { "leader" } else { "worker" };
        let team_ctx = TeamContext {
            name: &def.name,
            my_role,
            leader_id: &def.leader,
            members: &def.members,
            shared_dir: None,
        };

        let prompt_ctx = PromptContext {
            agent_dir: agent_dir.path(),
            max_steps: 50,
            skills: &[],
            team: Some(team_ctx),
        };

        let prompt = build_system_prompt(&prompt_ctx);
        assert!(prompt.contains("<TEAM_CONTEXT>"), "prompt should contain TEAM_CONTEXT block");
        assert!(prompt.contains("claim_task"), "worker should see claim_task instruction");
        assert!(prompt.contains("complete_task"), "worker should see complete_task instruction");
        assert!(!prompt.contains("的领导（leader）"), "worker should NOT get leader instructions");
    }
}
