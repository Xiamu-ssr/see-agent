use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::server::AppState;

#[derive(Serialize, Clone)]
struct LogEntry {
    time: String,
    level: String,
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

/// Strip ANSI escape codes from a string.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we find the terminating letter (@ through ~)
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() || c2 == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse a tracing-formatted log line into (time, level, message).
/// Expected format: `2026-03-19T07:43:30.494Z  INFO some message`
/// Falls back to empty time/level if parsing fails.
fn parse_log_line(line: &str) -> (String, String, String) {
    let cleaned = strip_ansi(line);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return (String::new(), String::new(), String::new());
    }

    // Look for a timestamp-like prefix (starts with digit, contains T)
    if let Some(space_idx) = trimmed.find("  ") {
        let maybe_time = &trimmed[..space_idx];
        let rest = trimmed[space_idx..].trim_start();
        // Check if it looks like a timestamp
        if maybe_time.len() > 10 && maybe_time.contains('T') {
            // Next token is the level
            if let Some(level_end) = rest.find(' ') {
                let level = &rest[..level_end];
                let msg = rest[level_end..].trim_start();
                return (maybe_time.to_string(), level.to_string(), msg.to_string());
            }
            return (maybe_time.to_string(), rest.to_string(), String::new());
        }
    }

    // Fallback: no parsing
    (String::new(), String::new(), trimmed.to_string())
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
        let (time, level, message) = parse_log_line(&line);
        if !message.is_empty() {
            entries.push(LogEntry {
                time,
                level,
                source: "server".into(),
                message,
            });
        }
    }

    // 2. Agent worker logs
    let agents_dir = ws.agents();
    if let Ok(dir) = std::fs::read_dir(&agents_dir) {
        for entry in dir.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let agent_id = entry.file_name().to_string_lossy().to_string();
            let worker_log = ws.agent(&agent_id).worker_log();
            if worker_log.exists() {
                for line in tail_lines(&worker_log, tail) {
                    let (time, level, message) = parse_log_line(&line);
                    if !message.is_empty() {
                        entries.push(LogEntry {
                            time,
                            level,
                            source: format!("agent:{agent_id}"),
                            message,
                        });
                    }
                }
            }
        }
    }

    // Sort by time descending (most recent first)
    entries.sort_by(|a, b| b.time.cmp(&a.time));

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

    #[test]
    fn parse_log_line_tracing_format() {
        let line = "2026-03-19T07:43:30.494806Z  INFO 🚀 Starting trunk";
        let (time, level, msg) = parse_log_line(line);
        assert_eq!(time, "2026-03-19T07:43:30.494806Z");
        assert_eq!(level, "INFO");
        assert!(msg.contains("Starting trunk"));
    }

    #[test]
    fn parse_log_line_with_target() {
        let line = "2026-03-19T07:43:30.494806Z  INFO see_agent_corp::server: Starting server";
        let (time, level, msg) = parse_log_line(line);
        assert_eq!(time, "2026-03-19T07:43:30.494806Z");
        assert_eq!(level, "INFO");
        assert!(msg.contains("see_agent_corp::server: Starting server"));
    }

    #[test]
    fn parse_log_line_with_ansi() {
        let line = "\x1b[2m2026-03-19T07:43:30.494806Z\x1b[0m  \x1b[32mINFO\x1b[0m Starting server";
        let (time, level, msg) = parse_log_line(line);
        assert_eq!(time, "2026-03-19T07:43:30.494806Z");
        assert_eq!(level, "INFO");
        assert!(msg.contains("Starting server"));
    }

    #[test]
    fn parse_log_line_plain() {
        let line = "Just a plain message";
        let (time, level, msg) = parse_log_line(line);
        assert!(time.is_empty());
        assert!(level.is_empty());
        assert_eq!(msg, "Just a plain message");
    }

    #[test]
    fn strip_ansi_removes_escape_codes() {
        assert_eq!(strip_ansi("\x1b[31mERROR\x1b[0m"), "ERROR");
        assert_eq!(strip_ansi("no codes"), "no codes");
    }
}
