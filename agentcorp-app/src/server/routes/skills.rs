use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use agentcorp::skill::{gate_skills, load_skills};

use crate::server::AppState;

#[derive(Serialize)]
struct SkillInfoResponse {
    name: String,
    description: String,
    available: bool,
    disabled: bool,
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

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/skills", get(list_skills_handler))
        .with_state(state)
}
