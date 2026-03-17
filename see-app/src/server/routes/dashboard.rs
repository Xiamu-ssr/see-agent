use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use see::agent::list_agents;
use see::team::list_teams;

use crate::server::AppState;

#[derive(Serialize)]
struct DashboardResponse {
    agents_count: usize,
    agents_running: usize,
    teams_count: usize,
    version: String,
}

async fn get_dashboard_handler(
    State(state): State<AppState>,
) -> Result<Json<DashboardResponse>, StatusCode> {
    let ws = state.workspace();

    let agents = list_agents(ws).unwrap_or_default();
    let teams = list_teams(ws).unwrap_or_default();

    let sup = state.inner.supervisor.read().await;
    let running = sup.running_agents().len();

    Ok(Json(DashboardResponse {
        agents_count: agents.len(),
        agents_running: running,
        teams_count: teams.len(),
        version: see::consts::VERSION.into(),
    }))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/dashboard", get(get_dashboard_handler))
        .with_state(state)
}
