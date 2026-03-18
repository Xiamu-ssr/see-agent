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

#[derive(Serialize)]
struct AgentSummaryResponse {
    id: String,
    name: String,
    emoji: String,
    state: AgentState,
    team_id: Option<String>,
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

#[derive(Serialize)]
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

    let sup = state.inner.supervisor.read().await;
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

    let sup = state.inner.supervisor.read().await;
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
    let (name, emoji) = parse_identity(ws, &def.id);

    Ok((
        StatusCode::CREATED,
        Json(AgentCreateResponse { id, name, emoji }),
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
        .with_state(state)
}
