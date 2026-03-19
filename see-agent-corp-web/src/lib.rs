mod api;
mod app;
mod components;
mod layout;
mod pages;

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn main() {
    mount_to_body(app::App);
}

/// Tests for API response type deserialization.
///
/// These verify that the Rust types used in the frontend match the JSON
/// format returned by the backend API. Tests run on the host (not wasm).
#[cfg(test)]
mod tests {
    use serde::Deserialize;

    // Re-declare the response types exactly as used in the page modules.
    // This is intentional: the test verifies the JSON contract, not the
    // Leptos component wiring.

    #[derive(Debug, Clone, Deserialize)]
    #[allow(dead_code)]
    struct AgentSummary {
        id: String,
        name: String,
        emoji: String,
        state: String,
        #[serde(default)]
        team_id: Option<String>,
        #[serde(default)]
        team_name: Option<String>,
        #[serde(default)]
        is_system: bool,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[allow(dead_code)]
    struct DashboardData {
        agents_count: usize,
        agents_running: usize,
        teams_count: usize,
        tools_count: usize,
        skills_count: usize,
        version: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[allow(dead_code)]
    struct ToolInfo {
        name: String,
        description: String,
        disabled: bool,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[allow(dead_code)]
    struct SessionMsg {
        msg_id: u64,
        timestamp: String,
        msg_type: String,
        data: serde_json::Value,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[allow(dead_code)]
    struct AgentDetailData {
        id: String,
        name: String,
        emoji: String,
        state: String,
        tools: Vec<String>,
        skills: Vec<String>,
        has_soul: bool,
        location: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[allow(dead_code)]
    struct FileEntry {
        name: String,
        #[serde(rename = "type")]
        entry_type: String,
        size: u64,
    }

    // -----------------------------------------------------------------------
    // AgentSummary
    // -----------------------------------------------------------------------

    #[test]
    fn agent_summary_deserialize() {
        let json = r#"{
            "id": "alice",
            "name": "Alice",
            "emoji": "A",
            "state": "sleeping",
            "team_id": null
        }"#;
        let agent: AgentSummary = serde_json::from_str(json).unwrap();
        assert_eq!(agent.id, "alice");
        assert_eq!(agent.state, "sleeping");
        assert!(agent.team_id.is_none());
    }

    #[test]
    fn agent_summary_with_team() {
        let json = r#"{
            "id": "bob",
            "name": "Bob",
            "emoji": "B",
            "state": "active",
            "team_id": "team-1"
        }"#;
        let agent: AgentSummary = serde_json::from_str(json).unwrap();
        assert_eq!(agent.team_id, Some("team-1".into()));
    }

    #[test]
    fn agent_summary_list_deserialize() {
        let json = r#"[
            {"id": "a1", "name": "A1", "emoji": "1", "state": "sleeping", "team_id": null},
            {"id": "a2", "name": "A2", "emoji": "2", "state": "active", "team_id": "t1"}
        ]"#;
        let agents: Vec<AgentSummary> = serde_json::from_str(json).unwrap();
        assert_eq!(agents.len(), 2);
    }

    // -----------------------------------------------------------------------
    // DashboardData
    // -----------------------------------------------------------------------

    #[test]
    fn dashboard_response_deserialize() {
        let json = r#"{
            "agents_count": 3,
            "agents_running": 1,
            "teams_count": 2,
            "tools_count": 19,
            "skills_count": 5,
            "version": "0.1.0"
        }"#;
        let d: DashboardData = serde_json::from_str(json).unwrap();
        assert_eq!(d.agents_count, 3);
        assert_eq!(d.agents_running, 1);
        assert_eq!(d.tools_count, 19);
        assert_eq!(d.version, "0.1.0");
    }

    #[test]
    fn dashboard_response_zero_values() {
        let json = r#"{
            "agents_count": 0,
            "agents_running": 0,
            "teams_count": 0,
            "tools_count": 0,
            "skills_count": 0,
            "version": "dev"
        }"#;
        let d: DashboardData = serde_json::from_str(json).unwrap();
        assert_eq!(d.agents_count, 0);
        assert_eq!(d.version, "dev");
    }

    // -----------------------------------------------------------------------
    // ToolInfo
    // -----------------------------------------------------------------------

    #[test]
    fn tool_info_deserialize() {
        let json = r#"{
            "name": "shell",
            "description": "Execute a shell command",
            "disabled": false
        }"#;
        let t: ToolInfo = serde_json::from_str(json).unwrap();
        assert_eq!(t.name, "shell");
        assert!(!t.disabled);
    }

    #[test]
    fn tool_info_disabled() {
        let json = r#"{
            "name": "screenshot",
            "description": "Capture a screenshot",
            "disabled": true
        }"#;
        let t: ToolInfo = serde_json::from_str(json).unwrap();
        assert!(t.disabled);
    }

    #[test]
    fn tool_info_list_deserialize() {
        let json = r#"[
            {"name": "shell", "description": "Execute a shell command", "disabled": false},
            {"name": "screenshot", "description": "Take screenshot", "disabled": true}
        ]"#;
        let tools: Vec<ToolInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools[1].disabled);
    }

    // -----------------------------------------------------------------------
    // SessionMsg
    // -----------------------------------------------------------------------

    #[test]
    fn session_msg_deserialize() {
        let json = r#"{
            "msg_id": 42,
            "timestamp": "2025-01-01T00:00:00Z",
            "msg_type": "assistant",
            "data": {"content": "Hello!"}
        }"#;
        let m: SessionMsg = serde_json::from_str(json).unwrap();
        assert_eq!(m.msg_id, 42);
        assert_eq!(m.msg_type, "assistant");
    }

    // -----------------------------------------------------------------------
    // AgentDetailData
    // -----------------------------------------------------------------------

    #[test]
    fn agent_detail_deserialize() {
        let json = r#"{
            "id": "alice",
            "name": "Alice",
            "emoji": "A",
            "state": "active",
            "tools": ["shell", "screenshot"],
            "skills": [],
            "has_soul": true,
            "location": "/tmp/agents/alice"
        }"#;
        let a: AgentDetailData = serde_json::from_str(json).unwrap();
        assert_eq!(a.id, "alice");
        assert!(a.has_soul);
        assert_eq!(a.tools.len(), 2);
    }

    // -----------------------------------------------------------------------
    // FileEntry
    // -----------------------------------------------------------------------

    #[test]
    fn file_entry_deserialize() {
        let json = r#"{
            "name": "SOUL.md",
            "type": "file",
            "size": 1024
        }"#;
        let f: FileEntry = serde_json::from_str(json).unwrap();
        assert_eq!(f.name, "SOUL.md");
        assert_eq!(f.entry_type, "file");
        assert_eq!(f.size, 1024);
    }

    #[test]
    fn file_entry_directory() {
        let json = r#"{
            "name": "memory",
            "type": "directory",
            "size": 0
        }"#;
        let f: FileEntry = serde_json::from_str(json).unwrap();
        assert_eq!(f.entry_type, "directory");
    }
}
