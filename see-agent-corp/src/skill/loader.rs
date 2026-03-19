use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tracing::{debug, warn};

use crate::types::SkillInfo;

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

/// Parse a SKILL.md file into a `SkillInfo`.
///
/// Expected format:
/// ```text
/// ---
/// name: my_skill
/// description: Does something useful.
/// metadata: {"requires_bins": ["git"]}
/// ---
///
/// ## Usage
/// ...
/// ```
fn parse_skill(path: &Path) -> Option<SkillInfo> {
    let content = std::fs::read_to_string(path).ok()?;

    // Split frontmatter from body
    let content = content.trim_start();
    if !content.starts_with("---") {
        warn!(?path, "SKILL.md missing frontmatter delimiter");
        return None;
    }

    let after_first = &content[3..];
    let end_idx = after_first.find("\n---")?;
    let frontmatter = &after_first[..end_idx];
    let body = after_first[end_idx + 4..].trim().to_owned();

    // Parse key-value pairs from frontmatter
    let mut name = String::new();
    let mut description = String::new();
    let mut metadata_str = String::new();

    for line in frontmatter.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "name" => name = value.to_owned(),
                "description" => description = value.to_owned(),
                "metadata" => metadata_str = value.to_owned(),
                _ => {} // ignore unknown keys
            }
        }
    }

    if name.is_empty() {
        warn!(?path, "SKILL.md missing 'name' in frontmatter");
        return None;
    }

    // Parse metadata for gate requirements
    let (requires_bins, requires_env, requires_any_bins) = parse_metadata(&metadata_str);

    Some(SkillInfo {
        name,
        description,
        body,
        path: path.to_string_lossy().into_owned(),
        requires_bins,
        requires_env,
        requires_any_bins,
        blocked: false,
        block_reason: None,
    })
}

/// Parse the metadata JSON string for gate requirements.
///
/// Supports two formats:
/// - Direct: `{"requires_bins": [...], "requires_env": [...], "requires_any_bins": [...]}`
/// - OpenClaw nested: `{"openclaw": {"requires": {"bins": [...], "env": [...], "anyBins": [...]}}}`
fn parse_metadata(meta: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    if meta.is_empty() {
        return (vec![], vec![], vec![]);
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(meta) else {
        warn!(metadata = meta, "failed to parse skill metadata as JSON");
        return (vec![], vec![], vec![]);
    };

    // Try direct format first
    let requires_bins = extract_string_array(&value, "requires_bins");
    let requires_env = extract_string_array(&value, "requires_env");
    let requires_any_bins = extract_string_array(&value, "requires_any_bins");

    if !requires_bins.is_empty() || !requires_env.is_empty() || !requires_any_bins.is_empty() {
        return (requires_bins, requires_env, requires_any_bins);
    }

    // Try OpenClaw nested format
    if let Some(openclaw) = value.get("openclaw").and_then(|v| v.get("requires")) {
        let bins = extract_string_array(openclaw, "bins");
        let env = extract_string_array(openclaw, "env");
        let any_bins = extract_string_array(openclaw, "anyBins");
        return (bins, env, any_bins);
    }

    (vec![], vec![], vec![])
}

fn extract_string_array(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

/// Resolve the effective skill directories for an agent.
///
/// Built-in defaults (always included):
///   1. `{workspace}/skills/` — global skills
///   2. `{workspace}/agents/{agent_id}/skills/` — agent-specific skills
///
/// Then appends extra dirs from config.skills.dirs and agent.skills.dirs.
pub fn resolve_skill_dirs(
    workspace: &crate::types::WorkspaceDir,
    agent_id: &str,
    config_extra: &[String],
    agent_extra: Option<&[String]>,
) -> Vec<String> {
    let mut dirs = vec![
        workspace.skills().to_string_lossy().into_owned(),
        workspace.agent(agent_id).skills().to_string_lossy().into_owned(),
    ];
    for d in config_extra {
        if !dirs.contains(d) {
            dirs.push(d.clone());
        }
    }
    if let Some(extra) = agent_extra {
        for d in extra {
            if !dirs.contains(d) {
                dirs.push(d.clone());
            }
        }
    }
    dirs
}

/// Load all skills from the given directories.
///
/// Searches each directory recursively for `SKILL.md` files.
/// Deduplicates by name (first-seen wins).
pub fn load_skills(dirs: &[String]) -> Vec<SkillInfo> {
    let mut skills = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for dir in dirs {
        let expanded = expand_tilde(dir);
        let path = Path::new(&expanded);
        if !path.exists() {
            debug!(?path, "skills directory does not exist, skipping");
            continue;
        }

        let mut skill_files: Vec<PathBuf> = Vec::new();
        collect_skill_files(path, &mut skill_files);
        skill_files.sort();

        for skill_file in skill_files {
            if let Some(info) = parse_skill(&skill_file) {
                if seen.contains(&info.name) {
                    warn!(name = %info.name, ?skill_file, "duplicate skill, skipping");
                    continue;
                }
                seen.insert(info.name.clone());
                skills.push(info);
            }
        }
    }

    debug!(count = skills.len(), "loaded skills");
    skills
}

/// Recursively collect all SKILL.md files under a directory.
fn collect_skill_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_skill_files(&path, out);
        } else if path.file_name().and_then(|f| f.to_str()) == Some("SKILL.md") {
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Filter and gate
// ---------------------------------------------------------------------------

/// Remove disabled skills from the list.
pub fn filter_skills(skills: Vec<SkillInfo>, disabled: &[String]) -> Vec<SkillInfo> {
    skills
        .into_iter()
        .filter(|s| !disabled.contains(&s.name))
        .collect()
}

/// Check gate requirements and mark blocked skills.
///
/// Gate checks:
/// - `requires_bins`: All listed binaries must be found in PATH
/// - `requires_env`: All listed env vars must be set
/// - `requires_any_bins`: At least one listed binary must be found
pub fn gate_skills(mut skills: Vec<SkillInfo>) -> Vec<SkillInfo> {
    for skill in &mut skills {
        let mut reasons = Vec::new();

        // Check required binaries
        if !skill.requires_bins.is_empty() {
            let missing: Vec<&str> = skill
                .requires_bins
                .iter()
                .filter(|b| !binary_exists(b))
                .map(|b| b.as_str())
                .collect();
            if !missing.is_empty() {
                reasons.push(format!("missing binaries: {}", missing.join(", ")));
            }
        }

        // Check required env vars
        if !skill.requires_env.is_empty() {
            let missing: Vec<&str> = skill
                .requires_env
                .iter()
                .filter(|e| std::env::var(e).is_err())
                .map(|e| e.as_str())
                .collect();
            if !missing.is_empty() {
                reasons.push(format!("missing env vars: {}", missing.join(", ")));
            }
        }

        // Check any-of binaries
        if !skill.requires_any_bins.is_empty()
            && !skill.requires_any_bins.iter().any(|b| binary_exists(b))
        {
            reasons.push(format!(
                "none of these binaries found: {}",
                skill.requires_any_bins.join(", ")
            ));
        }

        if !reasons.is_empty() {
            skill.blocked = true;
            skill.block_reason = Some(reasons.join("; "));
        }
    }

    skills
}

/// Check if a binary exists in PATH using `which`.
fn binary_exists(name: &str) -> bool {
    // Use std::process::Command to run `which` — portable across macOS/Linux
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Expand `~` at the start of a path to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().into_owned();
    }
    path.to_owned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_skill(dir: &Path, name: &str, content: &str) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn parse_basic_skill() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: test_skill\ndescription: A test skill.\n---\n\n## Usage\nDo stuff.\n",
        )
        .unwrap();

        let info = parse_skill(&path).unwrap();
        assert_eq!(info.name, "test_skill");
        assert_eq!(info.description, "A test skill.");
        assert!(info.body.contains("## Usage"));
        assert!(info.requires_bins.is_empty());
    }

    #[test]
    fn parse_skill_with_metadata() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: gated\ndescription: Needs git.\nmetadata: {\"requires_bins\": [\"git\"]}\n---\n\nBody.",
        )
        .unwrap();

        let info = parse_skill(&path).unwrap();
        assert_eq!(info.requires_bins, vec!["git"]);
    }

    #[test]
    fn parse_skill_openclaw_metadata() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: oc\ndescription: OpenClaw.\nmetadata: {\"openclaw\": {\"requires\": {\"bins\": [\"node\"], \"env\": [\"TOKEN\"]}}}\n---\n\nBody.",
        )
        .unwrap();

        let info = parse_skill(&path).unwrap();
        assert_eq!(info.requires_bins, vec!["node"]);
        assert_eq!(info.requires_env, vec!["TOKEN"]);
    }

    #[test]
    fn parse_skill_missing_name() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("SKILL.md");
        std::fs::write(&path, "---\ndescription: no name\n---\n\nBody.").unwrap();

        assert!(parse_skill(&path).is_none());
    }

    #[test]
    fn parse_skill_no_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("SKILL.md");
        std::fs::write(&path, "Just markdown, no frontmatter.").unwrap();

        assert!(parse_skill(&path).is_none());
    }

    #[test]
    fn load_skills_from_directory() {
        let tmp = TempDir::new().unwrap();
        write_skill(
            tmp.path(),
            "alpha",
            "---\nname: alpha\ndescription: First.\n---\n\nBody A.",
        );
        write_skill(
            tmp.path(),
            "beta",
            "---\nname: beta\ndescription: Second.\n---\n\nBody B.",
        );

        let skills = load_skills(&[tmp.path().to_string_lossy().into_owned()]);
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn load_skills_deduplicates() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();

        write_skill(
            tmp1.path(),
            "dupe",
            "---\nname: same_name\ndescription: First.\n---\n\nBody 1.",
        );
        write_skill(
            tmp2.path(),
            "dupe",
            "---\nname: same_name\ndescription: Second.\n---\n\nBody 2.",
        );

        let skills = load_skills(&[
            tmp1.path().to_string_lossy().into_owned(),
            tmp2.path().to_string_lossy().into_owned(),
        ]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "First.");
    }

    #[test]
    fn load_skills_nonexistent_dir() {
        let skills = load_skills(&["/nonexistent/dir".to_owned()]);
        assert!(skills.is_empty());
    }

    #[test]
    fn filter_disabled_skills() {
        let skills = vec![
            SkillInfo {
                name: "keep".into(),
                description: String::new(),
                body: String::new(),
                path: String::new(),
                requires_bins: vec![],
                requires_env: vec![],
                requires_any_bins: vec![],
                blocked: false,
                block_reason: None,
            },
            SkillInfo {
                name: "remove".into(),
                description: String::new(),
                body: String::new(),
                path: String::new(),
                requires_bins: vec![],
                requires_env: vec![],
                requires_any_bins: vec![],
                blocked: false,
                block_reason: None,
            },
        ];

        let filtered = filter_skills(skills, &["remove".to_owned()]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "keep");
    }

    #[test]
    fn gate_blocks_missing_binary() {
        let skills = vec![SkillInfo {
            name: "gated".into(),
            description: String::new(),
            body: String::new(),
            path: String::new(),
            requires_bins: vec!["_nonexistent_binary_xyzzy_12345".into()],
            requires_env: vec![],
            requires_any_bins: vec![],
            blocked: false,
            block_reason: None,
        }];

        let result = gate_skills(skills);
        assert!(result[0].blocked);
        assert!(result[0]
            .block_reason
            .as_ref()
            .unwrap()
            .contains("missing binaries"));
    }

    #[test]
    fn gate_blocks_missing_env() {
        let skills = vec![SkillInfo {
            name: "env_gated".into(),
            description: String::new(),
            body: String::new(),
            path: String::new(),
            requires_bins: vec![],
            requires_env: vec!["_NONEXISTENT_ENV_VAR_XYZ_99".into()],
            requires_any_bins: vec![],
            blocked: false,
            block_reason: None,
        }];

        let result = gate_skills(skills);
        assert!(result[0].blocked);
        assert!(result[0]
            .block_reason
            .as_ref()
            .unwrap()
            .contains("missing env vars"));
    }

    #[test]
    fn gate_passes_existing_binary() {
        // "ls" should exist on any unix system
        let skills = vec![SkillInfo {
            name: "ok".into(),
            description: String::new(),
            body: String::new(),
            path: String::new(),
            requires_bins: vec!["ls".into()],
            requires_env: vec![],
            requires_any_bins: vec![],
            blocked: false,
            block_reason: None,
        }];

        let result = gate_skills(skills);
        assert!(!result[0].blocked);
    }

    #[test]
    fn gate_any_bins_one_exists() {
        let skills = vec![SkillInfo {
            name: "any".into(),
            description: String::new(),
            body: String::new(),
            path: String::new(),
            requires_bins: vec![],
            requires_env: vec![],
            requires_any_bins: vec![
                "_nonexistent_xyzzy".into(),
                "ls".into(), // this exists
            ],
            blocked: false,
            block_reason: None,
        }];

        let result = gate_skills(skills);
        assert!(!result[0].blocked);
    }

    #[test]
    fn expand_tilde_expands() {
        let result = expand_tilde("~/test");
        assert!(!result.starts_with("~/"));
        assert!(result.ends_with("/test"));
    }

    #[test]
    fn expand_tilde_no_tilde() {
        assert_eq!(expand_tilde("/absolute/path"), "/absolute/path");
        assert_eq!(expand_tilde("relative"), "relative");
    }

    #[test]
    fn binary_exists_finds_ls() {
        assert!(binary_exists("ls"));
    }

    #[test]
    fn binary_exists_rejects_nonexistent() {
        assert!(!binary_exists("_nonexistent_binary_xyzzy_12345"));
    }
}
