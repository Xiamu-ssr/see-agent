use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use see_agent_corp::io::read_jsonl;
use see_agent_corp::team::{create_team, list_teams, load_team, TaskBoard};
use see_agent_corp::types::{Message, TaskStatus, TeamMember};

use crate::server::routes::files::{list_dir_entries, FileEntry};
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

#[derive(Serialize)]
struct TaskItemResponse {
    id: String,
    title: String,
    description: String,
    status: String,
    assigned_to: Option<String>,
    depends_on: Vec<String>,
    result: Option<String>,
    created_by: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct TeamMessageResponse {
    msg_id: Option<u64>,
    sender: String,
    content: String,
    priority: String,
    timestamp: String,
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

#[derive(Deserialize)]
struct CreateTaskRequest {
    title: String,
    #[serde(default)]
    description: String,
    created_by: String,
    #[serde(default)]
    depends_on: Vec<String>,
}

#[derive(Deserialize)]
struct UpdateTaskRequest {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    assigned_to: Option<String>,
    #[serde(default)]
    result: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers — team CRUD
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
// Handlers — task board
// ---------------------------------------------------------------------------

async fn list_tasks_handler(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<TaskItemResponse>>, StatusCode> {
    let ws = state.workspace();
    let team_dir = ws.team(&team_id);

    if !team_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let board = TaskBoard::new(team_dir);
    let tasks = board
        .list_tasks(None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<TaskItemResponse> = tasks
        .into_iter()
        .map(|t| TaskItemResponse {
            id: t.id,
            title: t.title,
            description: t.description,
            status: format!("{:?}", t.status).to_lowercase(),
            assigned_to: t.assigned_to,
            depends_on: t.depends_on,
            result: t.result,
            created_by: t.created_by,
            created_at: t.created_at,
            updated_at: t.updated_at,
        })
        .collect();

    Ok(Json(response))
}

async fn create_task_handler(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskItemResponse>), StatusCode> {
    let ws = state.workspace();
    let team_dir = ws.team(&team_id);

    if !team_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let board = TaskBoard::new(team_dir);
    let task = board
        .create_task(&req.title, &req.description, &req.created_by, req.depends_on)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(TaskItemResponse {
            id: task.id,
            title: task.title,
            description: task.description,
            status: format!("{:?}", task.status).to_lowercase(),
            assigned_to: task.assigned_to,
            depends_on: task.depends_on,
            result: task.result,
            created_by: task.created_by,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }),
    ))
}

async fn update_task_handler(
    State(state): State<AppState>,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<TaskItemResponse>, StatusCode> {
    let ws = state.workspace();
    let team_dir = ws.team(&team_id);

    if !team_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let status = req.status.as_deref().map(parse_task_status).transpose()?;

    let board = TaskBoard::new(team_dir);
    let task = board
        .update_task(
            &task_id,
            status,
            req.assigned_to.as_deref(),
            req.result.as_deref(),
        )
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(TaskItemResponse {
        id: task.id,
        title: task.title,
        description: task.description,
        status: format!("{:?}", task.status).to_lowercase(),
        assigned_to: task.assigned_to,
        depends_on: task.depends_on,
        result: task.result,
        created_by: task.created_by,
        created_at: task.created_at,
        updated_at: task.updated_at,
    }))
}

fn parse_task_status(s: &str) -> Result<TaskStatus, StatusCode> {
    match s {
        "pending" => Ok(TaskStatus::Pending),
        "claimed" => Ok(TaskStatus::Claimed),
        "in_progress" => Ok(TaskStatus::InProgress),
        "done" => Ok(TaskStatus::Done),
        "failed" => Ok(TaskStatus::Failed),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

// ---------------------------------------------------------------------------
// Handlers — team messages
// ---------------------------------------------------------------------------

async fn list_team_messages_handler(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<TeamMessageResponse>>, StatusCode> {
    let ws = state.workspace();
    let team_dir = ws.team(&team_id);

    if !team_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let messages_path = team_dir.messages();
    if !messages_path.exists() {
        return Ok(Json(vec![]));
    }

    let messages: Vec<Message> =
        read_jsonl(&messages_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<TeamMessageResponse> = messages
        .into_iter()
        .map(|m| TeamMessageResponse {
            msg_id: m.msg_id,
            sender: m.sender,
            content: m.content,
            priority: format!("{:?}", m.priority).to_lowercase(),
            timestamp: m.timestamp,
        })
        .collect();

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// Team shared files
// ---------------------------------------------------------------------------

async fn list_team_files_handler(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<FileEntry>>, StatusCode> {
    let ws = state.workspace();
    let team_dir = ws.team(&team_id);
    if !team_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    let shared = team_dir.shared();
    if !shared.exists() {
        return Ok(Json(vec![]));
    }
    let entries = list_dir_entries(&shared).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(entries))
}

async fn list_team_files_subdir_handler(
    State(state): State<AppState>,
    Path((team_id, subpath)): Path<(String, String)>,
) -> Result<Json<Vec<FileEntry>>, StatusCode> {
    let ws = state.workspace();
    let team_dir = ws.team(&team_id);
    if !team_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    let shared = team_dir.shared();
    let target = shared.join(&subpath);
    if !target.starts_with(&shared) {
        return Err(StatusCode::FORBIDDEN);
    }
    if !target.is_dir() {
        return Err(StatusCode::NOT_FOUND);
    }
    let entries = list_dir_entries(&target).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(entries))
}

async fn read_team_file_handler(
    State(state): State<AppState>,
    Path((team_id, filepath)): Path<(String, String)>,
) -> Result<String, StatusCode> {
    let ws = state.workspace();
    let team_dir = ws.team(&team_id);
    if !team_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    let shared = team_dir.shared();
    let target = shared.join(&filepath);
    if !target.starts_with(&shared) {
        return Err(StatusCode::FORBIDDEN);
    }
    if !target.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }
    std::fs::read_to_string(&target).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/teams", get(list_teams_handler).post(create_team_handler))
        .route("/teams/{team_id}", get(get_team_handler))
        .route(
            "/teams/{team_id}/tasks",
            get(list_tasks_handler).post(create_task_handler),
        )
        .route(
            "/teams/{team_id}/tasks/{task_id}",
            axum::routing::put(update_task_handler),
        )
        .route(
            "/teams/{team_id}/messages",
            get(list_team_messages_handler),
        )
        .route(
            "/teams/{team_id}/files",
            get(list_team_files_handler),
        )
        .route(
            "/teams/{team_id}/files/{*subpath}",
            get(list_team_files_subdir_handler),
        )
        .route(
            "/teams/{team_id}/file/{*filepath}",
            get(read_team_file_handler),
        )
        .with_state(state)
}
