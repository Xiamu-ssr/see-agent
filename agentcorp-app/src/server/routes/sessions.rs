use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use agentcorp::session::SessionStore;
use agentcorp::types::SessionMessageType;

use crate::server::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SessionMetaResponse {
    id: String,
    task: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
    total_steps: u32,
    elapsed_seconds: f64,
    summary: String,
}

#[derive(Serialize)]
struct SessionMessageResponse {
    msg_id: u64,
    timestamp: String,
    msg_type: String,
    data: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn get_session_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<SessionMetaResponse>, StatusCode> {
    let ws = state.workspace();
    let agent_dir = ws.agent(&agent_id);

    if !agent_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let store = SessionStore::new(agent_dir.session());
    let meta = store.load_meta().map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(SessionMetaResponse {
        id: meta.id,
        task: meta.task,
        status: format!("{:?}", meta.status).to_lowercase(),
        created_at: meta.created_at,
        updated_at: meta.updated_at,
        total_steps: meta.total_steps,
        elapsed_seconds: meta.elapsed_seconds,
        summary: meta.summary,
    }))
}

async fn get_session_messages_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<SessionMessageResponse>>, StatusCode> {
    let ws = state.workspace();
    let agent_dir = ws.agent(&agent_id);

    if !agent_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut store = SessionStore::new(agent_dir.session());
    let messages = store.read_messages().map_err(|_| StatusCode::NOT_FOUND)?;

    let response: Vec<SessionMessageResponse> = messages
        .into_iter()
        .map(|m| {
            let type_str = match m.msg_type {
                SessionMessageType::System => "system",
                SessionMessageType::UserTask => "user_task",
                SessionMessageType::Assistant => "assistant",
                SessionMessageType::ToolResult => "tool_result",
                SessionMessageType::Screenshot => "screenshot",
                SessionMessageType::UserReply => "user_reply",
                SessionMessageType::SystemHint => "system_hint",
                SessionMessageType::Compact => "compact",
            };
            SessionMessageResponse {
                msg_id: m.msg_id,
                timestamp: m.timestamp,
                msg_type: type_str.to_owned(),
                data: m.data,
            }
        })
        .collect();

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agents/{agent_id}/session", get(get_session_handler))
        .route(
            "/agents/{agent_id}/session/messages",
            get(get_session_messages_handler),
        )
        .with_state(state)
}
