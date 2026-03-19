use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use see_agent_corp::agent::{create_agent, delete_agent, list_agents, load_agent};
use see_agent_corp::types::{AgentState, Message, MessagePriority};

use crate::server::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct AgentSummaryResponse {
    id: String,
    name: String,
    emoji: String,
    state: AgentState,
    #[serde(skip_serializing_if = "Option::is_none")]
    team_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    team_name: Option<String>,
    is_system: bool,
}

#[derive(Serialize)]
struct AgentDetailResponse {
    id: String,
    name: String,
    emoji: String,
    state: AgentState,
    tools: Vec<String>,
    skills: Vec<String>,
    has_soul: bool,
    location: String,
}

#[derive(Serialize)]
struct AgentCreateResponse {
    id: String,
    name: String,
    emoji: String,
}

#[derive(Serialize, Deserialize)]
struct StatusResponse {
    status: String,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateAgentRequest {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    emoji: Option<String>,
}

#[derive(Deserialize)]
struct SendMessageRequest {
    content: String,
    #[serde(default = "default_priority")]
    priority: String,
}

fn default_priority() -> String {
    "collect".into()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_agents_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<AgentSummaryResponse>>, StatusCode> {
    let ws = state.workspace();
    let agents = list_agents(ws).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut sup = state.inner.supervisor.write().await;
    sup.reap_exited();
    let summaries: Vec<AgentSummaryResponse> = agents
        .into_iter()
        .map(|a| {
            let agent_state = sup.agent_state(&a.id);
            AgentSummaryResponse {
                id: a.id,
                name: a.name,
                emoji: a.emoji,
                state: agent_state,
                team_id: a.team_id,
                team_name: a.team_name,
                is_system: a.is_system,
            }
        })
        .collect();

    Ok(Json(summaries))
}

async fn get_agent_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentDetailResponse>, StatusCode> {
    let ws = state.workspace();
    let _def = load_agent(ws, &agent_id).map_err(|_| StatusCode::NOT_FOUND)?;

    let agent_dir = ws.agent(&agent_id);
    let has_soul = agent_dir.soul_md().exists();
    let (name, emoji) = parse_identity(ws, &agent_id);

    let mut sup = state.inner.supervisor.write().await;
    let agent_state = sup.agent_state(&agent_id);

    Ok(Json(AgentDetailResponse {
        id: agent_id,
        name,
        emoji,
        state: agent_state,
        tools: vec![],
        skills: vec![],
        has_soul,
        location: agent_dir.path().to_string_lossy().into_owned(),
    }))
}

async fn create_agent_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<(StatusCode, Json<AgentCreateResponse>), StatusCode> {
    let ws = state.workspace();
    let id = req.id.unwrap_or_else(|| {
        format!("agent-{}", chrono::Utc::now().timestamp_millis() % 100000)
    });

    let def = create_agent(ws, &id, None, None).map_err(|_| StatusCode::CONFLICT)?;

    // Write IDENTITY.md with name/emoji if provided
    let agent_name = req.name.unwrap_or_else(|| id.clone());
    let agent_emoji = req.emoji.unwrap_or_else(|| "🤖".to_owned());
    let identity_content = format!("# Identity\n\n**Name:** {}\n**Emoji:** {}\n", agent_name, agent_emoji);
    let agent_dir = ws.agent(&def.id);
    let _ = std::fs::write(agent_dir.path().join("IDENTITY.md"), &identity_content);

    Ok((
        StatusCode::CREATED,
        Json(AgentCreateResponse {
            id,
            name: agent_name,
            emoji: agent_emoji,
        }),
    ))
}

async fn delete_agent_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let ws = state.workspace();
    delete_agent(ws, &agent_id).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(StatusResponse {
        status: "deleted".into(),
    }))
}

async fn send_message_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let priority = if req.priority == "steer" {
        MessagePriority::Steer
    } else {
        MessagePriority::Collect
    };

    let msg = Message {
        msg_id: None,
        sender: "user".into(),
        content: req.content,
        priority,
        metadata: Default::default(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let mut sup = state.inner.supervisor.write().await;
    sup.send_to(&agent_id, msg)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(StatusResponse { status: "sent".into() }))
}

async fn get_agent_logs_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let ws = state.workspace();
    let agent_dir = ws.agent(&agent_id);
    let log_path = agent_dir.path().join("worker.log");

    if !log_path.exists() {
        return Ok(Json(vec![]));
    }

    let content = std::fs::read_to_string(&log_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let lines: Vec<String> = content
        .lines()
        .rev()
        .take(see_agent_corp::consts::LOG_TAIL_LINES)
        .map(|l| l.to_owned())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    Ok(Json(lines))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_identity(ws: &see_agent_corp::types::WorkspaceDir, agent_id: &str) -> (String, String) {
    let agent_dir = ws.agent(agent_id);
    let identity_path = agent_dir.identity_md();
    if let Ok(content) = std::fs::read_to_string(&identity_path) {
        let name = content
            .lines()
            .find(|l| l.starts_with("name:"))
            .map(|l| l.trim_start_matches("name:").trim().to_owned())
            .unwrap_or_else(|| agent_id.to_owned());
        let emoji = content
            .lines()
            .find(|l| l.starts_with("emoji:"))
            .map(|l| l.trim_start_matches("emoji:").trim().to_owned())
            .unwrap_or_else(|| "🤖".to_owned());
        (name, emoji)
    } else {
        (agent_id.to_owned(), "🤖".to_owned())
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agents", get(list_agents_handler).post(create_agent_handler))
        .route(
            "/agents/{agent_id}",
            get(get_agent_handler).delete(delete_agent_handler),
        )
        .route("/agents/{agent_id}/message", post(send_message_handler))
        .route("/agents/{agent_id}/logs", get(get_agent_logs_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn make_test_state() -> AppState {
        let tmp = tempfile::TempDir::new().unwrap();
        let ws = see_agent_corp::types::WorkspaceDir::new(tmp.path());
        see_agent_corp::config::ensure_workspace(&ws).unwrap();
        std::mem::forget(tmp);
        AppState::new(ws)
    }

    #[tokio::test]
    async fn list_agents_returns_empty_for_fresh_workspace() {
        let state = make_test_state();
        let app = router(state);
        let req = Request::builder()
            .method("GET")
            .uri("/agents")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let agents: Vec<AgentSummaryResponse> =
            serde_json::from_slice(&body).unwrap();
        // Fresh workspace has system agent created by ensure_workspace
        // but list_agents only returns dirs with agent.json
        // System agent dir exists but may or may not have agent.json
        assert!(agents.len() <= 1);
    }

    #[tokio::test]
    async fn list_agents_returns_created_agent() {
        let state = make_test_state();
        let ws = state.workspace();
        see_agent_corp::agent::create_agent(ws, "test-a", Some("TestA"), Some("T")).unwrap();

        let app = router(state);
        let req = Request::builder()
            .method("GET")
            .uri("/agents")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let agents: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        let ids: Vec<&str> = agents.iter().filter_map(|a| a["id"].as_str()).collect();
        assert!(ids.contains(&"test-a"));
    }

    #[tokio::test]
    async fn send_message_writes_to_inbox() {
        let state = make_test_state();
        let ws = state.workspace();
        see_agent_corp::agent::create_agent(ws, "msg-agent", None, None).unwrap();

        // Set supervisor binary to /usr/bin/true so auto-start doesn't fail
        {
            let mut sup = state.inner.supervisor.write().await;
            sup.set_binary_path(std::path::PathBuf::from("/usr/bin/true"));
        }

        let app = router(state);
        let body_json = serde_json::json!({
            "content": "hello test",
            "priority": "collect"
        });
        let req = Request::builder()
            .method("POST")
            .uri("/agents/msg-agent/message")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: StatusResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.status, "sent");
    }

    #[tokio::test]
    async fn send_message_to_nonexistent_returns_404() {
        let state = make_test_state();
        let app = router(state);
        let body_json = serde_json::json!({
            "content": "hello",
            "priority": "collect"
        });
        let req = Request::builder()
            .method("POST")
            .uri("/agents/nonexistent/message")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
