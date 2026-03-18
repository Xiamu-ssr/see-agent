use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use agentcorp::agent::list_agents;
use agentcorp::skill::{gate_skills, load_skills};
use agentcorp::team::list_teams;
use agentcorp::tool::builtin_tool_infos;

use crate::server::AppState;

#[derive(Serialize)]
struct DashboardResponse {
    agents_count: usize,
    agents_running: usize,
    teams_count: usize,
    tools_count: usize,
    skills_count: usize,
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

    let config = state.inner.config.read().await;
    let tools_count = builtin_tool_infos().len();
    let skills_count = gate_skills(load_skills(&config.skills.dirs)).len();

    Ok(Json(DashboardResponse {
        agents_count: agents.len(),
        agents_running: running,
        teams_count: teams.len(),
        tools_count,
        skills_count,
        version: agentcorp::consts::VERSION.into(),
    }))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/dashboard", get(get_dashboard_handler))
        .with_state(state)
}
