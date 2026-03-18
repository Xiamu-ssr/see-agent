use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;

use see_agent_corp::types::Config;

use crate::server::AppState;

async fn get_config_handler(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let config = state.inner.config.read().await;
    let mut value = serde_json::to_value(&*config).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Mask the API key
    if let Some(llm) = value.get_mut("llm")
        && let Some(key) = llm.get("api_key").and_then(|v| v.as_str())
        && key.len() > 8
    {
        let masked = format!("{}...{}", &key[..4], &key[key.len() - 4..]);
        llm["api_key"] = Value::String(masked);
    }

    Ok(Json(value))
}

async fn update_config_handler(
    State(state): State<AppState>,
    Json(updates): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let ws = state.workspace();

    if let Some(obj) = updates.as_object() {
        // Strip masked API key so the real key on disk is preserved.
        // The GET endpoint masks it (e.g. "sk-x...abcd"); if the frontend
        // sends that masked value back, we must not overwrite the real key.
        let mut sanitized = obj.clone();
        if let Some(llm) = sanitized.get_mut("llm")
            && let Some(llm_obj) = llm.as_object_mut()
            && llm_obj
                .get("api_key")
                .and_then(|v| v.as_str())
                .is_some_and(|k| k.contains("..."))
        {
            llm_obj.remove("api_key");
        }

        let config_path = ws.config();
        let current: Value = if config_path.exists() {
            let text = std::fs::read_to_string(&config_path)
                .map_err(|e| server_error(format!("read config: {e}")))?;
            serde_json::from_str(&text).unwrap_or(Value::Object(Default::default()))
        } else {
            Value::Object(Default::default())
        };

        let merged =
            see_agent_corp::config::deep_merge(&current, &Value::Object(sanitized));

        // Validate BEFORE writing to disk — reject bad types with 400
        serde_json::from_value::<Config>(merged.clone()).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("invalid config: {e}") })),
            )
        })?;

        let json = serde_json::to_string_pretty(&merged)
            .map_err(|e| server_error(format!("serialize config: {e}")))?;
        std::fs::write(&config_path, json)
            .map_err(|e| server_error(format!("write config: {e}")))?;

        // Reload config
        let new_config = see_agent_corp::config::load_config(ws)
            .map_err(|e| server_error(format!("reload config: {e}")))?;
        let mut cfg = state.inner.config.write().await;
        *cfg = new_config;
    }

    Ok(Json(serde_json::json!({"status": "updated"})))
}

fn server_error(msg: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": msg })),
    )
}

/// Return a JSON schema derived from Config via schemars — single source of truth.
async fn get_config_schema_handler() -> Json<Value> {
    let schema = schemars::schema_for!(Config);
    let value = serde_json::to_value(schema).unwrap_or_default();
    Json(value)
}

/// Return default config values (derived from Config::default()).
async fn get_config_defaults_handler() -> Result<Json<Value>, StatusCode> {
    let defaults = Config::default();
    let value =
        serde_json::to_value(defaults).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(value))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/config", get(get_config_handler).put(update_config_handler))
        .route("/config/schema", get(get_config_schema_handler))
        .route("/config/defaults", get(get_config_defaults_handler))
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
    async fn config_schema_returns_valid_json_schema() {
        let state = make_test_state();
        let app = router(state);
        let req = Request::builder()
            .method("GET")
            .uri("/config/schema")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let schema: Value = serde_json::from_slice(&body).unwrap();
        // Must be a valid JSON schema object with $schema or type fields
        assert!(schema.is_object());
        // Config schema should have properties for llm, agent, tools, etc.
        let props = &schema["properties"];
        assert!(props["llm"].is_object(), "schema should have llm property");
        assert!(props["agent"].is_object(), "schema should have agent property");
        assert!(props["tools"].is_object(), "schema should have tools property");
    }

    #[tokio::test]
    async fn config_defaults_returns_valid_config() {
        let state = make_test_state();
        let app = router(state);
        let req = Request::builder()
            .method("GET")
            .uri("/config/defaults")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let defaults: Value = serde_json::from_slice(&body).unwrap();
        assert!(defaults.is_object());
        // Default config should have expected top-level keys
        assert!(defaults["llm"].is_object());
        assert!(defaults["agent"].is_object());
        assert!(defaults["tools"].is_object());
        // Check a specific default value
        assert!(defaults["agent"]["max_steps"].is_number());
    }

    #[tokio::test]
    async fn config_defaults_roundtrips_to_config() {
        let state = make_test_state();
        let app = router(state);
        let req = Request::builder()
            .method("GET")
            .uri("/config/defaults")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        // Verify the defaults JSON can deserialize back into a Config
        let _config: Config = serde_json::from_slice(&body).unwrap();
    }

    #[tokio::test]
    async fn update_config_masked_api_key_preserves_real_key() {
        let state = make_test_state();
        let real_key = "sk-realkey1234567890abcdef";

        // Write a config with a real API key
        let config_path = state.workspace().config();
        let initial = serde_json::json!({ "llm": { "api_key": real_key } });
        std::fs::write(&config_path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

        // Reload state so it picks up the real key
        let new_config = see_agent_corp::config::load_config(state.workspace()).unwrap();
        *state.inner.config.write().await = new_config;

        // Simulate what the frontend does: GET config (gets masked key), then PUT it back
        let app = router(state.clone());
        let req = Request::builder()
            .method("PUT")
            .uri("/config")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({
                    "llm": { "api_key": "sk-r...cdef" }
                }))
                .unwrap(),
            ))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify the real key on disk is preserved (not corrupted by masked value)
        let disk_text = std::fs::read_to_string(&config_path).unwrap();
        let disk_json: Value = serde_json::from_str(&disk_text).unwrap();
        assert_eq!(
            disk_json["llm"]["api_key"].as_str().unwrap(),
            real_key,
            "masked api_key must not overwrite real key on disk"
        );
    }

    #[tokio::test]
    async fn update_config_invalid_type_returns_400() {
        let state = make_test_state();
        let app = router(state);

        // Send a string where a number is expected (agent.max_steps)
        let req = Request::builder()
            .method("PUT")
            .uri("/config")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({
                    "agent": { "max_steps": "not_a_number" }
                }))
                .unwrap(),
            ))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "invalid config type should return 400, not 500"
        );

        // Verify response body has error info
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_json: Value = serde_json::from_slice(&body).unwrap();
        assert!(
            body_json["error"].is_string(),
            "error response should include error message"
        );
    }

    #[tokio::test]
    async fn update_config_valid_data_returns_200() {
        let state = make_test_state();
        let app = router(state.clone());

        let req = Request::builder()
            .method("PUT")
            .uri("/config")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({
                    "llm": { "model": "gpt-5" }
                }))
                .unwrap(),
            ))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify the config was actually updated on disk
        let disk_text = std::fs::read_to_string(state.workspace().config()).unwrap();
        let disk_json: Value = serde_json::from_str(&disk_text).unwrap();
        assert_eq!(disk_json["llm"]["model"].as_str().unwrap(), "gpt-5");

        // Verify in-memory config was updated too
        let cfg = state.inner.config.read().await;
        assert_eq!(cfg.llm.model, "gpt-5");
    }
}
