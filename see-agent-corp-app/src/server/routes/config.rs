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
) -> Result<Json<Value>, StatusCode> {
    // Deep merge updates into current config
    let ws = state.workspace();

    if let Some(obj) = updates.as_object() {
        let config_path = ws.config();
        let current: Value = if config_path.exists() {
            let text =
                std::fs::read_to_string(&config_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            serde_json::from_str(&text).unwrap_or(Value::Object(Default::default()))
        } else {
            Value::Object(Default::default())
        };

        let merged = see_agent_corp::config::deep_merge(&current, &Value::Object(obj.clone()));

        let json =
            serde_json::to_string_pretty(&merged).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        std::fs::write(&config_path, json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Reload config
        let new_config =
            see_agent_corp::config::load_config(ws).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let mut cfg = state.inner.config.write().await;
        *cfg = new_config;
    }

    Ok(Json(serde_json::json!({"status": "updated"})))
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
}
