use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use see_agent_corp::tool::builtin_tool_infos;

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

/// Per-agent tool toggle: reads/writes to agent.json, not config.json.
async fn toggle_agent_tool_handler(
    State(state): State<AppState>,
    Path((agent_id, tool_name)): Path<(String, String)>,
    Json(req): Json<ToggleRequest>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let agent_dir = state.workspace().agent(&agent_id);
    let agent_json_path = agent_dir.agent_json();

    // Read existing agent.json (or start from minimal object)
    let mut agent_value: Value = if agent_json_path.exists() {
        let content =
            std::fs::read_to_string(&agent_json_path).map_err(|_| StatusCode::NOT_FOUND)?;
        serde_json::from_str(&content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Ensure tools.disabled array exists
    let obj = agent_value
        .as_object_mut()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if !obj.contains_key("tools") {
        obj.insert(
            "tools".to_owned(),
            serde_json::json!({"disabled": []}),
        );
    }
    let tools_obj = obj
        .get_mut("tools")
        .and_then(|v| v.as_object_mut())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if !tools_obj.contains_key("disabled") {
        tools_obj.insert("disabled".to_owned(), serde_json::json!([]));
    }

    let disabled_arr = tools_obj
        .get_mut("disabled")
        .and_then(|v| v.as_array_mut())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    if req.disabled {
        let tool_val = Value::String(tool_name.clone());
        if !disabled_arr.contains(&tool_val) {
            disabled_arr.push(tool_val);
        }
    } else {
        disabled_arr.retain(|v| v.as_str() != Some(&tool_name));
    }

    // Write back to agent.json
    let json_str = serde_json::to_string_pretty(&agent_value)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(&agent_json_path, json_str).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let action = if req.disabled { "disabled" } else { "enabled" };
    Ok(Json(StatusResponse {
        status: format!("{tool_name} {action} for agent {agent_id}"),
    }))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/tools", get(list_tools_handler))
        .route(
            "/agents/{agent_id}/tools/{tool_name}/toggle",
            post(toggle_agent_tool_handler),
        )
        .with_state(state)
}
