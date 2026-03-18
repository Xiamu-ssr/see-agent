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
