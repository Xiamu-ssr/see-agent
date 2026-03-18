use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::server::AppState;

#[derive(Serialize)]
struct LogEntry {
    time: String,
    level: String,
    message: String,
}

async fn get_logs_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<LogEntry>>, StatusCode> {
    // Read the most recent log file from workspace
    let ws = state.workspace();
    let log_dir = ws.logs();

    if !log_dir.exists() {
        return Ok(Json(vec![]));
    }

    // Find latest log file
    let mut entries: Vec<LogEntry> = Vec::new();
    if let Ok(dir) = std::fs::read_dir(&log_dir) {
        let mut files: Vec<_> = dir.flatten().map(|e| e.path()).collect();
        files.sort();

        if let Some(latest) = files.last()
            && let Ok(content) = std::fs::read_to_string(latest)
        {
            for line in content.lines().rev().take(see::consts::LOG_TAIL_LINES) {
                entries.push(LogEntry {
                    time: String::new(),
                    level: "info".into(),
                    message: line.to_owned(),
                });
            }
        }
    }

    entries.reverse();
    Ok(Json(entries))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/logs", get(get_logs_handler))
        .with_state(state)
}
