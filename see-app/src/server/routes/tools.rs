use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use see::tool::{register_builtin_tools, ToolRegistry};

use crate::server::AppState;

#[derive(Serialize)]
struct ToolInfoResponse {
    name: String,
    description: String,
}

async fn list_tools_handler(
    State(_state): State<AppState>,
) -> Result<Json<Vec<ToolInfoResponse>>, StatusCode> {
    let mut registry = ToolRegistry::new();
    register_builtin_tools(&mut registry);

    let tools: Vec<ToolInfoResponse> = registry
        .names()
        .into_iter()
        .map(|name| {
            let desc = registry
                .get(&name)
                .map(|t| t.description().to_owned())
                .unwrap_or_default();
            ToolInfoResponse {
                name,
                description: desc,
            }
        })
        .collect();

    Ok(Json(tools))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/tools", get(list_tools_handler))
        .with_state(state)
}
