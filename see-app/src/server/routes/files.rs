use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::server::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct FileEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String,
    size: u64,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// List files in an agent's directory.
async fn list_agent_files_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<FileEntry>>, StatusCode> {
    let ws = state.workspace();
    let agent_dir = ws.agent(&agent_id);

    if !agent_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let entries = list_dir_entries(agent_dir.path()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(entries))
}

/// List files in a subdirectory of an agent's directory.
async fn list_agent_subdir_handler(
    State(state): State<AppState>,
    Path((agent_id, subpath)): Path<(String, String)>,
) -> Result<Json<Vec<FileEntry>>, StatusCode> {
    let ws = state.workspace();
    let agent_dir = ws.agent(&agent_id);

    if !agent_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Prevent path traversal
    let target = agent_dir.path().join(&subpath);
    if !target.starts_with(agent_dir.path()) {
        return Err(StatusCode::FORBIDDEN);
    }

    if !target.is_dir() {
        return Err(StatusCode::NOT_FOUND);
    }

    let entries = list_dir_entries(&target).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(entries))
}

/// Read a file's contents from an agent's directory.
async fn read_agent_file_handler(
    State(state): State<AppState>,
    Path((agent_id, filepath)): Path<(String, String)>,
) -> Result<String, StatusCode> {
    let ws = state.workspace();
    let agent_dir = ws.agent(&agent_id);

    if !agent_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let target = agent_dir.path().join(&filepath);
    if !target.starts_with(agent_dir.path()) {
        return Err(StatusCode::FORBIDDEN);
    }

    if !target.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Limit file size to avoid serving huge files
    let meta = std::fs::metadata(&target).map_err(|_| StatusCode::NOT_FOUND)?;
    if meta.len() > see::consts::MAX_FILE_CHARS as u64 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    std::fs::read_to_string(&target).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ---------------------------------------------------------------------------
// Write handler
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct WriteFileRequest {
    content: String,
}

#[derive(Serialize)]
struct WriteFileResponse {
    status: String,
}

async fn write_agent_file_handler(
    State(state): State<AppState>,
    Path((agent_id, filepath)): Path<(String, String)>,
    Json(req): Json<WriteFileRequest>,
) -> Result<Json<WriteFileResponse>, StatusCode> {
    let ws = state.workspace();
    let agent_dir = ws.agent(&agent_id);

    if !agent_dir.path().exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let target = agent_dir.path().join(&filepath);
    if !target.starts_with(agent_dir.path()) {
        return Err(StatusCode::FORBIDDEN);
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    std::fs::write(&target, &req.content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(WriteFileResponse {
        status: "saved".into(),
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn list_dir_entries(path: &std::path::Path) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().to_string();

        let entry_type = if meta.is_dir() {
            "directory"
        } else {
            "file"
        };

        entries.push(FileEntry {
            name,
            entry_type: entry_type.to_owned(),
            size: meta.len(),
        });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agents/{agent_id}/files", get(list_agent_files_handler))
        .route(
            "/agents/{agent_id}/files/{*subpath}",
            get(list_agent_subdir_handler),
        )
        .route(
            "/agents/{agent_id}/file/{*filepath}",
            get(read_agent_file_handler).put(write_agent_file_handler),
        )
        .with_state(state)
}
