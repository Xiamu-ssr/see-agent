use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use see_agent_corp::skill::{gate_skills, load_skills};

use crate::server::AppState;

#[derive(Serialize)]
struct SkillInfoResponse {
    name: String,
    description: String,
    available: bool,
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

async fn list_skills_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<SkillInfoResponse>>, StatusCode> {
    let config = state.inner.config.read().await;
    let skills_disabled = &config.skills.disabled;
    let skills = load_skills(&config.skills.dirs);
    let skills = gate_skills(skills);

    let response: Vec<SkillInfoResponse> = skills
        .into_iter()
        .map(|s| {
            let disabled = skills_disabled.contains(&s.name);
            SkillInfoResponse {
                name: s.name,
                description: s.description,
                available: !s.blocked && !disabled,
                disabled,
            }
        })
        .collect();

    Ok(Json(response))
}

/// Per-agent skill toggle: reads/writes to agent.json, not config.json.
async fn toggle_agent_skill_handler(
    State(state): State<AppState>,
    Path((agent_id, skill_name)): Path<(String, String)>,
    Json(req): Json<ToggleRequest>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let agent_dir = state.workspace().agent(&agent_id);
    let agent_json_path = agent_dir.agent_json();

    // Read existing agent.json (or 404 if agent doesn't exist)
    let mut agent_value: Value = if agent_json_path.exists() {
        let content =
            std::fs::read_to_string(&agent_json_path).map_err(|_| StatusCode::NOT_FOUND)?;
        serde_json::from_str(&content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Ensure skills.disabled array exists
    let obj = agent_value
        .as_object_mut()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if !obj.contains_key("skills") {
        obj.insert(
            "skills".to_owned(),
            serde_json::json!({"disabled": []}),
        );
    }
    let skills_obj = obj
        .get_mut("skills")
        .and_then(|v| v.as_object_mut())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if !skills_obj.contains_key("disabled") {
        skills_obj.insert("disabled".to_owned(), serde_json::json!([]));
    }

    let disabled_arr = skills_obj
        .get_mut("disabled")
        .and_then(|v| v.as_array_mut())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    if req.disabled {
        let skill_val = Value::String(skill_name.clone());
        if !disabled_arr.contains(&skill_val) {
            disabled_arr.push(skill_val);
        }
    } else {
        disabled_arr.retain(|v| v.as_str() != Some(&skill_name));
    }

    // Write back to agent.json
    let json_str = serde_json::to_string_pretty(&agent_value)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(&agent_json_path, json_str).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let action = if req.disabled { "disabled" } else { "enabled" };
    Ok(Json(StatusResponse {
        status: format!("{skill_name} {action} for agent {agent_id}"),
    }))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/skills", get(list_skills_handler))
        .route(
            "/agents/{agent_id}/skills/{skill_name}/toggle",
            post(toggle_agent_skill_handler),
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
    async fn toggle_skill_writes_agent_json() {
        let state = make_test_state();
        let ws = state.workspace();
        see_agent_corp::agent::create_agent(ws, "skill-toggle", None, None).unwrap();

        let app = router(state.clone());
        let body_json = serde_json::json!({"disabled": true});
        let req = Request::builder()
            .method("POST")
            .uri("/agents/skill-toggle/skills/web_search/toggle")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify agent.json has web_search in skills.disabled list
        let agent_json: Value = serde_json::from_str(
            &std::fs::read_to_string(ws.agent("skill-toggle").agent_json()).unwrap(),
        )
        .unwrap();
        let disabled = agent_json["skills"]["disabled"].as_array().unwrap();
        assert!(disabled.contains(&Value::String("web_search".into())));
    }

    #[tokio::test]
    async fn toggle_skill_enable_removes_from_disabled() {
        let state = make_test_state();
        let ws = state.workspace();
        see_agent_corp::agent::create_agent(ws, "skill-re", None, None).unwrap();

        // First disable web_search
        let app = router(state.clone());
        let body_json = serde_json::json!({"disabled": true});
        let req = Request::builder()
            .method("POST")
            .uri("/agents/skill-re/skills/web_search/toggle")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Now re-enable web_search
        let app2 = router(state.clone());
        let body_json2 = serde_json::json!({"disabled": false});
        let req2 = Request::builder()
            .method("POST")
            .uri("/agents/skill-re/skills/web_search/toggle")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json2).unwrap()))
            .unwrap();
        let resp2 = app2.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        // Verify web_search is no longer in disabled list
        let agent_json: Value = serde_json::from_str(
            &std::fs::read_to_string(ws.agent("skill-re").agent_json()).unwrap(),
        )
        .unwrap();
        let disabled = agent_json["skills"]["disabled"].as_array().unwrap();
        assert!(!disabled.contains(&Value::String("web_search".into())));
    }

    #[tokio::test]
    async fn toggle_skill_nonexistent_agent_returns_404() {
        let state = make_test_state();
        let app = router(state);
        let body_json = serde_json::json!({"disabled": true});
        let req = Request::builder()
            .method("POST")
            .uri("/agents/nope/skills/web_search/toggle")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn toggle_skill_disable_is_idempotent() {
        let state = make_test_state();
        let ws = state.workspace();
        see_agent_corp::agent::create_agent(ws, "skill-idem", None, None).unwrap();

        // Disable web_search twice
        for _ in 0..2 {
            let app = router(state.clone());
            let body_json = serde_json::json!({"disabled": true});
            let req = Request::builder()
                .method("POST")
                .uri("/agents/skill-idem/skills/web_search/toggle")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string(&body_json).unwrap()))
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        // Verify web_search appears exactly once in disabled list
        let agent_json: Value = serde_json::from_str(
            &std::fs::read_to_string(ws.agent("skill-idem").agent_json()).unwrap(),
        )
        .unwrap();
        let disabled = agent_json["skills"]["disabled"].as_array().unwrap();
        let count = disabled
            .iter()
            .filter(|v| v.as_str() == Some("web_search"))
            .count();
        assert_eq!(count, 1);
    }
}
