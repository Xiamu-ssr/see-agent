use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::server::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct McpServerResponse {
    name: String,
    command: String,
    args: Vec<String>,
    #[serde(rename = "type")]
    server_type: String,
    url: Option<String>,
    disabled: bool,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_mcp_servers_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<McpServerResponse>>, StatusCode> {
    let config = state.inner.config.read().await;

    let servers: Vec<McpServerResponse> = config
        .mcp
        .servers
        .iter()
        .map(|(name, server_config)| {
            let disabled = config.mcp.disabled.contains(name);
            McpServerResponse {
                name: name.clone(),
                command: server_config.command.clone(),
                args: server_config.args.clone(),
                server_type: format!("{:?}", server_config.server_type).to_lowercase(),
                url: server_config.url.clone(),
                disabled,
            }
        })
        .collect();

    Ok(Json(servers))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/mcp/servers", get(list_mcp_servers_handler))
        .with_state(state)
}
