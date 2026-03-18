use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::config::{LlmConfig, McpConfig, SandboxConfig, SkillsConfig, ToolsConfig};

// ---------------------------------------------------------------------------
// AgentDefinition (on-disk agent.json model)
// ---------------------------------------------------------------------------

/// The on-disk `agent.json` representation.
///
/// Contains the agent id plus config overrides. Only non-default
/// sections are written to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<SkillsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,
    /// Extra fields not captured by known sections.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl AgentDefinition {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            llm: None,
            agent: None,
            screen: None,
            tools: None,
            skills: None,
            mcp: None,
            sandbox: None,
            extra: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// AgentStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Running,
    Stopped,
    Error,
}

// ---------------------------------------------------------------------------
// AgentSummary (list / dashboard view)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub emoji: String,
    pub status: AgentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
}

// ---------------------------------------------------------------------------
// AgentDetail (full info for a single agent)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDetail {
    pub id: String,
    pub name: String,
    pub emoji: String,
    pub status: AgentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub sandbox_profile: String,
    #[serde(default)]
    pub has_soul: bool,
    /// Filesystem path to the agent directory.
    #[serde(default)]
    pub location: String,
}
