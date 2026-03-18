use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use agentcorp::tool::builtin_tool_infos;

use crate::server::AppState;

#[derive(Serialize)]
struct ToolInfoResponse {
    name: String,
    description: String,
    disabled: bool,
}

#[derive(Deserialize)]
struct ToggleRequest {
    disabled: bool,
}

#[derive(Serialize)]
struct StatusResponse {
    status: String,
}

async fn list_tools_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<ToolInfoResponse>>, StatusCode> {
    let config = state.inner.config.read().await;
    let disabled = &config.tools.disabled;

    let tools: Vec<ToolInfoResponse> = builtin_tool_infos()
        .into_iter()
        .map(|(name, desc)| ToolInfoResponse {
            disabled: disabled.contains(&name.to_owned()),
            name: name.to_owned(),
            description: desc.to_owned(),
        })
        .collect();

    Ok(Json(tools))
}

async fn toggle_tool_handler(
    State(state): State<AppState>,
    Path(tool_name): Path<String>,
    Json(req): Json<ToggleRequest>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let mut config = state.inner.config.write().await;

    if req.disabled {
        if !config.tools.disabled.contains(&tool_name) {
            config.tools.disabled.push(tool_name.clone());
        }
    } else {
        config.tools.disabled.retain(|t| t != &tool_name);
    }

    // Persist to disk
    let config_path = state.workspace().config();
    let config_json = serde_json::to_string_pretty(&*config)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(&config_path, config_json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let action = if req.disabled { "disabled" } else { "enabled" };
    Ok(Json(StatusResponse {
        status: format!("{tool_name} {action}"),
    }))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/tools", get(list_tools_handler))
        .route("/tools/{tool_name}/toggle", post(toggle_tool_handler))
        .with_state(state)
}
