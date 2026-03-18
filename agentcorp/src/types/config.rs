use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::consts;

// ---------------------------------------------------------------------------
// Top-level Config (matches MentalModel.md section 五 config.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct Config {
    #[serde(default)]
    pub node: NodeConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub agent: AgentBehaviorConfig,
    #[serde(default)]
    pub screen: ScreenConfig,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub sandbox: SandboxConfig,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// NodeConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct NodeConfig {
    /// Machine identifier. Defaults to hostname at runtime.
    #[serde(default)]
    pub id: String,
    /// Listen address. Empty string = local-only, no remote connections.
    #[serde(default)]
    pub listen: String,
}

// ---------------------------------------------------------------------------
// LlmConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmConfig {
    #[serde(default = "default_llm_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
}

fn default_llm_base_url() -> String {
    consts::DEFAULT_LLM_BASE_URL.to_owned()
}

fn default_llm_model() -> String {
    consts::DEFAULT_LLM_MODEL.to_owned()
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            base_url: default_llm_base_url(),
            api_key: String::new(),
            model: default_llm_model(),
        }
    }
}

// ---------------------------------------------------------------------------
// AgentBehaviorConfig + CompactConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentBehaviorConfig {
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
    #[serde(default)]
    pub compact: CompactConfig,
}

fn default_max_steps() -> u32 {
    consts::DEFAULT_MAX_STEPS
}

impl Default for AgentBehaviorConfig {
    fn default() -> Self {
        Self {
            max_steps: default_max_steps(),
            compact: CompactConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompactConfig {
    #[serde(default = "default_context_window")]
    pub context_window: u64,
    #[serde(default = "default_target_ratio")]
    pub target_ratio: f64,
    #[serde(default = "default_keep_recent")]
    pub keep_recent: u32,
    /// Empty string = use the main model.
    #[serde(default)]
    pub summary_model: String,
}

fn default_context_window() -> u64 {
    consts::DEFAULT_CONTEXT_WINDOW
}

fn default_target_ratio() -> f64 {
    consts::DEFAULT_COMPACT_TARGET_RATIO
}

fn default_keep_recent() -> u32 {
    consts::DEFAULT_COMPACT_KEEP_RECENT
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            context_window: default_context_window(),
            target_ratio: default_target_ratio(),
            keep_recent: default_keep_recent(),
            summary_model: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// ScreenConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScalingMatch {
    #[default]
    AspectRatio,
    PixelCount,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScreenConfig {
    #[serde(default = "default_max_images")]
    pub max_images: u32,
    #[serde(default = "default_screenshot_interval_ms")]
    pub screenshot_interval_ms: u64,
    #[serde(default = "default_tool_delay_ms")]
    pub tool_delay_ms: u64,
    #[serde(default = "default_scaling_enabled")]
    pub scaling_enabled: bool,
    #[serde(default)]
    pub scaling_match: ScalingMatch,
    #[serde(default = "default_show_overlay")]
    pub show_overlay: bool,
}

fn default_max_images() -> u32 {
    consts::DEFAULT_MAX_CONTEXT_IMAGES
}

fn default_screenshot_interval_ms() -> u64 {
    consts::DEFAULT_SCREENSHOT_INTERVAL_MS
}

fn default_tool_delay_ms() -> u64 {
    consts::DEFAULT_TOOL_DELAY_MS
}

fn default_scaling_enabled() -> bool {
    true
}

fn default_show_overlay() -> bool {
    true
}

impl Default for ScreenConfig {
    fn default() -> Self {
        Self {
            max_images: default_max_images(),
            screenshot_interval_ms: default_screenshot_interval_ms(),
            tool_delay_ms: default_tool_delay_ms(),
            scaling_enabled: default_scaling_enabled(),
            scaling_match: ScalingMatch::default(),
            show_overlay: default_show_overlay(),
        }
    }
}

// ---------------------------------------------------------------------------
// SkillsConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillsConfig {
    #[serde(default = "default_skills_dirs")]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub disabled: Vec<String>,
}

fn default_skills_dirs() -> Vec<String> {
    vec![consts::DEFAULT_SKILLS_DIR.to_owned()]
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            dirs: default_skills_dirs(),
            disabled: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// McpConfig + McpServerConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpServerType {
    #[default]
    Stdio,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServerConfig {
    #[serde(default, rename = "type")]
    pub server_type: McpServerType,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub disabled: Vec<String>,
}

// ---------------------------------------------------------------------------
// ToolsConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct ToolsConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
}

// ---------------------------------------------------------------------------
// SandboxConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SandboxConfig {
    #[serde(default = "default_sandbox_profile")]
    pub profile: String,
    #[serde(default)]
    pub extra_read: Vec<String>,
    #[serde(default)]
    pub extra_write: Vec<String>,
}

fn default_sandbox_profile() -> String {
    consts::DEFAULT_SANDBOX_PROFILE.to_owned()
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            profile: default_sandbox_profile(),
            extra_read: Vec::new(),
            extra_write: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// WebConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebConfig {
    #[serde(default = "default_web_language")]
    pub language: String,
}

fn default_web_language() -> String {
    consts::DEFAULT_WEB_LANGUAGE.to_owned()
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            language: default_web_language(),
        }
    }
}
