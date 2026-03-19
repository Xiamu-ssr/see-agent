use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::server::AppState;

#[derive(Serialize, Clone)]
struct LogEntry {
    source: String,
    message: String,
}

/// Read the last N lines from a file.
fn tail_lines(path: &std::path::Path, n: usize) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .rev()
        .take(n)
        .map(|l| l.to_owned())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

async fn get_logs_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<LogEntry>>, StatusCode> {
    let ws = state.workspace();
    let tail = see_agent_corp::consts::LOG_TAIL_LINES;
    let mut entries: Vec<LogEntry> = Vec::new();

    // 1. Server log
    let server_log = ws.server_log();
    for line in tail_lines(&server_log, tail) {
        entries.push(LogEntry {
            source: "server".into(),
            message: line,
        });
    }

    // 2. Agent worker logs
    let agents_dir = ws.agents();
    if let Ok(dir) = std::fs::read_dir(&agents_dir) {
        for entry in dir.flatten() {
            let worker_log = entry.path().join("worker.log");
            if worker_log.exists() {
                let agent_id = entry
                    .file_name()
                    .to_string_lossy()
                    .to_string();
                for line in tail_lines(&worker_log, tail) {
                    entries.push(LogEntry {
                        source: format!("agent:{agent_id}"),
                        message: line,
                    });
                }
            }
        }
    }

    Ok(Json(entries))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/logs", get(get_logs_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_lines_reads_last_n() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.log");
        std::fs::write(&path, "a\nb\nc\nd\ne\n").unwrap();
        let lines = tail_lines(&path, 3);
        assert_eq!(lines, vec!["c", "d", "e"]);
    }

    #[test]
    fn tail_lines_missing_file() {
        let lines = tail_lines(std::path::Path::new("/nonexistent"), 10);
        assert!(lines.is_empty());
    }
}
