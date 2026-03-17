use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use see::team::{create_team, list_teams, load_team};
use see::types::TeamMember;

use crate::server::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TeamSummaryResponse {
    id: String,
    name: String,
    members: Vec<TeamMemberResponse>,
    status: String,
}

#[derive(Serialize)]
struct TeamMemberResponse {
    id: String,
    role: String,
}

#[derive(Serialize)]
struct TeamCreateResponse {
    id: String,
    name: String,
    status: String,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateTeamRequest {
    name: String,
    members: Vec<TeamMemberInput>,
    leader: Option<String>,
}

#[derive(Deserialize)]
struct TeamMemberInput {
    id: String,
    #[serde(default = "default_role")]
    role: String,
}

fn default_role() -> String {
    "member".into()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_teams_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<TeamSummaryResponse>>, StatusCode> {
    let ws = state.workspace();
    let teams = list_teams(ws).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let summaries: Vec<TeamSummaryResponse> = teams
        .into_iter()
        .map(|t| TeamSummaryResponse {
            id: t.id,
            name: t.name,
            members: t
                .members
                .into_iter()
                .map(|m| TeamMemberResponse {
                    id: m.id,
                    role: m.role,
                })
                .collect(),
            status: format!("{:?}", t.status),
        })
        .collect();

    Ok(Json(summaries))
}

async fn create_team_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateTeamRequest>,
) -> Result<(StatusCode, Json<TeamCreateResponse>), StatusCode> {
    let ws = state.workspace();
    let members: Vec<TeamMember> = req
        .members
        .into_iter()
        .map(|m| TeamMember {
            id: m.id,
            role: m.role,
            endpoint: None,
        })
        .collect();

    let def = create_team(ws, &req.name, members, req.leader.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(TeamCreateResponse {
            id: def.id,
            name: def.name,
            status: "created".into(),
        }),
    ))
}

async fn get_team_handler(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
) -> Result<Json<TeamSummaryResponse>, StatusCode> {
    let ws = state.workspace();
    let def = load_team(ws, &team_id).map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(TeamSummaryResponse {
        id: team_id,
        name: def.name,
        members: def
            .members
            .into_iter()
            .map(|m| TeamMemberResponse {
                id: m.id,
                role: m.role,
            })
            .collect(),
        status: "created".into(),
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/teams", get(list_teams_handler).post(create_team_handler))
        .route("/teams/{team_id}", get(get_team_handler))
        .with_state(state)
}
