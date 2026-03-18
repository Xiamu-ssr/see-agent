use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub body: String,
    pub path: String,
    #[serde(default)]
    pub requires_bins: Vec<String>,
    #[serde(default)]
    pub requires_env: Vec<String>,
    #[serde(default)]
    pub requires_any_bins: Vec<String>,
    #[serde(default)]
    pub blocked: bool,
    pub block_reason: Option<String>,
}
