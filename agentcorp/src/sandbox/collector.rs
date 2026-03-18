use serde::Serialize;

/// A single sandbox violation event from macOS unified logs.
#[derive(Debug, Clone, Serialize)]
pub struct SandboxViolation {
    pub timestamp: String,
    pub operation: String,
    pub path: String,
}

/// Collect sandbox deny events for a given process ID from macOS unified logs.
///
/// Uses `log show` to query sandbox category events for the given PID
/// within the last `minutes` minutes.
pub async fn collect_violations(pid: u32, minutes: u32) -> Vec<SandboxViolation> {
    let predicate = format!(
        "processID == {pid} AND category == \"Sandbox\""
    );
    let last = format!("{minutes}m");

    let output = match tokio::process::Command::new("log")
        .args(["show", "--predicate", &predicate, "--last", &last, "--style", "json"])
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_log_output(&text)
}

/// Parse JSON log output from `log show --style json` and extract sandbox deny events.
fn parse_log_output(text: &str) -> Vec<SandboxViolation> {
    let mut violations = Vec::new();

    // The output is a JSON array of log entries
    let entries: Vec<serde_json::Value> = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return violations,
    };

    for entry in entries {
        let message = entry
            .get("eventMessage")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if !message.contains("deny") {
            continue;
        }

        let timestamp = entry
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        // Extract operation: "deny(1) file-read-data" → "file-read-data"
        let operation = extract_operation(message);
        let path = extract_path(message);

        violations.push(SandboxViolation {
            timestamp,
            operation,
            path,
        });
    }

    violations
}

/// Extract the denied operation from a sandbox log message.
/// e.g. "deny(1) file-read-data" → "file-read-data"
fn extract_operation(message: &str) -> String {
    if let Some(pos) = message.find("deny") {
        let after = &message[pos..];
        // Skip "deny" and optional "(N) "
        let rest = after.trim_start_matches("deny");
        let rest = rest.trim_start_matches(|c: char| c == '(' || c.is_ascii_digit() || c == ')');
        let rest = rest.trim_start();
        // Take until whitespace or end
        rest.split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_owned()
    } else {
        "unknown".to_owned()
    }
}

/// Extract the path from a sandbox log message.
/// Looks for `path "..."` pattern.
fn extract_path(message: &str) -> String {
    if let Some(start) = message.find("path \"") {
        let after = &message[start + 6..];
        if let Some(end) = after.find('"') {
            return after[..end].to_owned();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_operation_basic() {
        assert_eq!(
            extract_operation("deny(1) file-read-data"),
            "file-read-data"
        );
    }

    #[test]
    fn extract_operation_no_parens() {
        assert_eq!(extract_operation("deny file-write-data"), "file-write-data");
    }

    #[test]
    fn extract_path_basic() {
        assert_eq!(
            extract_path("deny(1) file-read-data path \"/usr/local/bin/foo\""),
            "/usr/local/bin/foo"
        );
    }

    #[test]
    fn extract_path_missing() {
        assert_eq!(extract_path("deny(1) network-outbound"), "");
    }

    #[test]
    fn parse_log_output_empty() {
        let violations = parse_log_output("[]");
        assert!(violations.is_empty());
    }

    #[test]
    fn parse_log_output_with_deny() {
        let json = r#"[{"timestamp":"2024-01-01T00:00:00Z","eventMessage":"deny(1) file-read-data path \"/secret\""}]"#;
        let violations = parse_log_output(json);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "file-read-data");
        assert_eq!(violations[0].path, "/secret");
    }

    #[test]
    fn parse_log_output_skips_non_deny() {
        let json = r#"[{"timestamp":"t1","eventMessage":"allow file-read-data"},{"timestamp":"t2","eventMessage":"deny(1) network-outbound"}]"#;
        let violations = parse_log_output(json);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "network-outbound");
    }
}
