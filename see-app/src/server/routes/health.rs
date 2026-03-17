use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use see::consts::VERSION;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: VERSION.into(),
    })
}

pub fn router() -> Router {
    Router::new().route("/health", get(health_check))
}
