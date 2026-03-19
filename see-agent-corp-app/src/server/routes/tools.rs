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
    group: String,
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
        .map(|(name, desc, group)| ToolInfoResponse {
            disabled: disabled.contains(&name.to_owned()),
            name: name.to_owned(),
            description: desc.to_owned(),
            group: group.to_owned(),
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
    async fn list_tools_returns_all_builtin_tools() {
        let state = make_test_state();
        let app = router(state);
        let req = Request::builder()
            .method("GET")
            .uri("/tools")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let tools: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(tools.len(), builtin_tool_infos().len());
        // Every tool has name, description, disabled
        for tool in &tools {
            assert!(tool["name"].is_string());
            assert!(tool["description"].is_string());
            assert!(tool["disabled"].is_boolean());
        }
    }

    #[tokio::test]
    async fn toggle_tool_writes_agent_json() {
        let state = make_test_state();
        let ws = state.workspace();
        see_agent_corp::agent::create_agent(ws, "toggle-test", None, None).unwrap();

        let app = router(state.clone());
        let body_json = serde_json::json!({"disabled": true});
        let req = Request::builder()
            .method("POST")
            .uri("/agents/toggle-test/tools/shell/toggle")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify agent.json has shell in disabled list
        let agent_json: Value =
            serde_json::from_str(&std::fs::read_to_string(ws.agent("toggle-test").agent_json()).unwrap())
                .unwrap();
        let disabled = agent_json["tools"]["disabled"].as_array().unwrap();
        assert!(disabled.contains(&Value::String("shell".into())));
    }

    #[tokio::test]
    async fn toggle_tool_enable_removes_from_disabled() {
        let state = make_test_state();
        let ws = state.workspace();
        see_agent_corp::agent::create_agent(ws, "toggle-e", None, None).unwrap();

        // First disable shell
        let app = router(state.clone());
        let body_json = serde_json::json!({"disabled": true});
        let req = Request::builder()
            .method("POST")
            .uri("/agents/toggle-e/tools/shell/toggle")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Now re-enable shell
        let app2 = router(state.clone());
        let body_json2 = serde_json::json!({"disabled": false});
        let req2 = Request::builder()
            .method("POST")
            .uri("/agents/toggle-e/tools/shell/toggle")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json2).unwrap()))
            .unwrap();
        let resp2 = app2.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        // Verify shell is no longer in disabled list
        let agent_json: Value =
            serde_json::from_str(&std::fs::read_to_string(ws.agent("toggle-e").agent_json()).unwrap())
                .unwrap();
        let disabled = agent_json["tools"]["disabled"].as_array().unwrap();
        assert!(!disabled.contains(&Value::String("shell".into())));
    }

    #[tokio::test]
    async fn toggle_tool_nonexistent_agent_returns_404() {
        let state = make_test_state();
        let app = router(state);
        let body_json = serde_json::json!({"disabled": true});
        let req = Request::builder()
            .method("POST")
            .uri("/agents/nope/tools/shell/toggle")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
