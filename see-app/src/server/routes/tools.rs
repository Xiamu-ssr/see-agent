use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use see::tool::builtin_tool_infos;

use crate::server::AppState;

#[derive(Serialize)]
struct ToolInfoResponse {
    name: String,
    description: String,
}

async fn list_tools_handler(
    State(_state): State<AppState>,
) -> Result<Json<Vec<ToolInfoResponse>>, StatusCode> {
    let tools: Vec<ToolInfoResponse> = builtin_tool_infos()
        .into_iter()
        .map(|(name, desc)| ToolInfoResponse {
            name: name.to_owned(),
            description: desc.to_owned(),
        })
        .collect();

    Ok(Json(tools))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/tools", get(list_tools_handler))
        .with_state(state)
}
