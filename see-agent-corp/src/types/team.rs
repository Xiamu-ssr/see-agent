use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// TeamStatus (matches MentalModel.md section 三 team.json "status")
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamStatus {
    Created,
    Running,
    Stopped,
}

// ---------------------------------------------------------------------------
// TeamMember
// ---------------------------------------------------------------------------

/// A member entry in team.json.
///
/// `endpoint` = None means the agent is local (same machine).
/// `endpoint` = Some(addr) means the agent is on a remote node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub id: String,
    pub role: String,
    /// Remote node address. None = local agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

// ---------------------------------------------------------------------------
// TeamSummary (list / dashboard view)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSummary {
    pub id: String,
    pub name: String,
    pub status: TeamStatus,
    pub members: Vec<TeamMember>,
    pub leader: String,
}

// ---------------------------------------------------------------------------
// TeamDefinition (full team.json on disk)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDefinition {
    pub id: String,
    pub name: String,
    pub members: Vec<TeamMember>,
    /// Agent id of the team leader.
    pub leader: String,
    pub status: TeamStatus,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}
