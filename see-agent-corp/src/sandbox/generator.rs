use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::types::config::SandboxConfig;
use crate::types::{Config, WorkspaceDir};

/// Placeholder token in .sb profiles replaced with the actual home directory.
const HOME_PLACEHOLDER: &str = "__SAFEHOUSE_REPLACE_ME_WITH_ABSOLUTE_HOME_DIR__";

/// Embedded core sandbox profiles.
const PROFILE_BASE: &str = include_str!("profiles/00-base.sb");
const PROFILE_SYSTEM: &str = include_str!("profiles/10-system-runtime.sb");
const PROFILE_NETWORK: &str = include_str!("profiles/20-network.sb");
const PROFILE_SEE_AGENT: &str = include_str!("profiles/see-agent-corp-base.sb");

// ---------------------------------------------------------------------------
// SandboxProfile — describes what an agent can access
// ---------------------------------------------------------------------------

/// Describes the sandbox permission profile for an agent.
#[derive(Debug, Clone, Serialize)]
pub struct SandboxProfile {
    pub rw_dirs: Vec<String>,
    pub ro_dirs: Vec<String>,
    pub network_outbound: bool,
    pub extra_read: Vec<String>,
    pub extra_write: Vec<String>,
}

/// Build a sandbox profile for an agent based on its role and team membership.
pub fn build_sandbox_profile(
    workspace: &WorkspaceDir,
    agent_id: &str,
    is_system: bool,
    team_id: Option<&str>,
    config: &Config,
    agent_sandbox: Option<&SandboxConfig>,
) -> SandboxProfile {
    let ws_path = workspace.path().to_string_lossy().into_owned();

    let mut rw_dirs = Vec::new();
    let mut ro_dirs = Vec::new();

    if is_system {
        // System agent: read-write entire workspace
        rw_dirs.push(ws_path);
    } else {
        // Normal agent: rw own dir, ro config + skills
        let agent_dir = workspace.agent(agent_id);
        rw_dirs.push(agent_dir.path().to_string_lossy().into_owned());
        ro_dirs.push(workspace.config().to_string_lossy().into_owned());
        ro_dirs.push(workspace.skills().to_string_lossy().into_owned());

        // Team access
        if let Some(tid) = team_id {
            let team_dir = workspace.team(tid);
            // All team members need rw access to shared/, tasklist, and messages
            let shared = team_dir.shared().to_string_lossy().into_owned();
            if std::path::Path::new(&shared).exists() {
                rw_dirs.push(shared);
            }
            let tasklist = team_dir.tasklist().to_string_lossy().into_owned();
            if std::path::Path::new(&tasklist).exists() {
                rw_dirs.push(tasklist);
            }
            let messages = team_dir.messages().to_string_lossy().into_owned();
            if std::path::Path::new(&messages).exists() {
                rw_dirs.push(messages);
            }
            let team_json = team_dir.team_json().to_string_lossy().into_owned();
            if std::path::Path::new(&team_json).exists() {
                ro_dirs.push(team_json);
            }
        }
    }

    // Extra paths from global config
    let mut extra_read: Vec<String> = config.sandbox.extra_read.clone();
    let mut extra_write: Vec<String> = config.sandbox.extra_write.clone();

    // Extra paths from agent-level sandbox config
    if let Some(agent_sb) = agent_sandbox {
        extra_read.extend(agent_sb.extra_read.iter().cloned());
        extra_write.extend(agent_sb.extra_write.iter().cloned());
    }

    SandboxProfile {
        rw_dirs,
        ro_dirs,
        network_outbound: true,
        extra_read,
        extra_write,
    }
}

// ---------------------------------------------------------------------------
// Safehouse CLI integration
// ---------------------------------------------------------------------------

/// Check if `safehouse` binary is available in PATH.
pub fn safehouse_available() -> bool {
    std::process::Command::new("safehouse")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Build command-line arguments for `safehouse` from a SandboxProfile.
///
/// Maps to safehouse CLI: `--add-dirs` (rw), `--add-dirs-ro` (ro), `--enable` (features).
/// Network is allowed by default in safehouse (no flag needed).
pub fn build_safehouse_args(profile: &SandboxProfile) -> Vec<String> {
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/Users/unknown".to_owned());

    let mut args = Vec::new();

    // Read-write directories
    let mut rw_paths: Vec<String> = profile.rw_dirs.iter().map(|d| expand_home(d, &home)).collect();
    rw_paths.extend(profile.extra_write.iter().map(|d| expand_home(d, &home)));
    if !rw_paths.is_empty() {
        args.push("--add-dirs".into());
        args.push(rw_paths.join(":"));
    }

    // Read-only directories
    let mut ro_paths: Vec<String> = profile.ro_dirs.iter().map(|d| expand_home(d, &home)).collect();
    ro_paths.extend(profile.extra_read.iter().map(|d| expand_home(d, &home)));
    if !ro_paths.is_empty() {
        args.push("--add-dirs-ro".into());
        args.push(ro_paths.join(":"));
    }

    // Enable required features
    // - process-control: Worker needs SIGUSR1 signal handling + shell tool spawns subprocesses
    // - shell-init: shell tool may need .zshrc/.bashrc
    args.push("--enable".into());
    args.push("process-control,shell-init".into());

    // Pass through SAC_BIN environment variable so agent can find CLI binary
    args.push("--env-pass".into());
    args.push("SAC_BIN".into());

    // Network is allowed by default in safehouse — no flag needed.

    args.push("--".into());
    args
}

// ---------------------------------------------------------------------------
// Legacy .sb profile generation (kept for backward compat)
// ---------------------------------------------------------------------------

/// Generate a combined sandbox profile for an agent (legacy .sb format).
///
/// Assembles profile fragments, replaces the HOME_DIR placeholder,
/// adds per-agent directory rules, and writes to /tmp.
///
/// Returns the path to the generated .sb file.
pub fn generate_profile(
    workspace: &WorkspaceDir,
    agent_id: &str,
    config: &Config,
) -> PathBuf {
    let mut parts: Vec<&str> = vec![
        PROFILE_BASE,
        PROFILE_SYSTEM,
        PROFILE_NETWORK,
        PROFILE_SEE_AGENT,
    ];

    // Try to load optional profiles from the profiles directory
    let profiles_dir = find_profiles_dir();
    if let Some(dir) = &profiles_dir {
        load_optional_profile(&mut parts, dir, "30-toolchains/python.sb");
        load_optional_profile(&mut parts, dir, "30-toolchains/runtime-managers.sb");
        load_optional_profile(&mut parts, dir, "40-shared/agent-common.sb");
        load_optional_profile(&mut parts, dir, "50-integrations-core/git.sb");
        load_optional_profile(&mut parts, dir, "50-integrations-core/scm-clis.sb");
        load_optional_profile(&mut parts, dir, "55-integrations-optional/macos-gui.sb");
        load_optional_profile(&mut parts, dir, "55-integrations-optional/clipboard.sb");
        load_optional_profile(&mut parts, dir, "55-integrations-optional/shell-init.sb");
    }

    let mut combined = parts.join("\n\n");

    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/Users/unknown".to_owned());
    combined = combined.replace(HOME_PLACEHOLDER, &home);

    let agent_dir = workspace.agent(agent_id);
    combined.push_str(&format!(
        "\n\n;; Per-agent directory access\n(allow file-read* file-write*\n    (subpath \"{}\")\n)\n",
        agent_dir.path().display()
    ));

    if !config.sandbox.extra_read.is_empty() {
        combined.push_str("\n;; Extra read paths from config\n(allow file-read*\n");
        for path in &config.sandbox.extra_read {
            combined.push_str(&format!("    (subpath \"{}\")\n", expand_home(path, &home)));
        }
        combined.push_str(")\n");
    }
    if !config.sandbox.extra_write.is_empty() {
        combined.push_str("\n;; Extra write paths from config\n(allow file-read* file-write*\n");
        for path in &config.sandbox.extra_write {
            combined.push_str(&format!("    (subpath \"{}\")\n", expand_home(path, &home)));
        }
        combined.push_str(")\n");
    }

    let output_path = PathBuf::from(format!("/tmp/see-agent-corp-{agent_id}.sb"));
    let _ = std::fs::write(&output_path, &combined);

    output_path
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_profiles_dir() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent()?.join("profiles");
        if dir.is_dir() {
            return Some(dir);
        }
    }

    let dev_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sandbox/profiles");
    if dev_path.is_dir() {
        return Some(dev_path);
    }

    None
}

fn load_optional_profile(parts: &mut Vec<&str>, _dir: &Path, _relative: &str) {
    let _ = parts;
    let _ = _dir;
    let _ = _relative;
}

/// Expand ~ to the actual home directory.
fn expand_home(path: &str, home: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else {
        path.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_contains_deny_default() {
        let combined = [PROFILE_BASE, PROFILE_SYSTEM].join("\n\n");
        assert!(combined.contains("(deny default)"));
    }

    #[test]
    fn profile_replaces_home() {
        let combined = PROFILE_BASE.replace(HOME_PLACEHOLDER, "/Users/test");
        assert!(combined.contains("/Users/test"));
        assert!(!combined.contains(HOME_PLACEHOLDER));
    }

    #[test]
    fn generate_creates_file() {
        use crate::config::ensure_workspace;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();
        crate::agent::create_agent(&ws, "test-sandbox", None, None).unwrap();

        let config = Config::default();
        let path = generate_profile(&ws, "test-sandbox", &config);

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("(deny default)"));
        assert!(content.contains("test-sandbox"));
        assert!(!content.contains(HOME_PLACEHOLDER));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extra_paths_included() {
        use crate::config::ensure_workspace;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();
        crate::agent::create_agent(&ws, "test-extra", None, None).unwrap();

        let mut config = Config::default();
        config.sandbox.extra_read = vec!["/usr/local/share".into()];
        config.sandbox.extra_write = vec!["~/custom-dir".into()];

        let path = generate_profile(&ws, "test-extra", &config);
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.contains("/usr/local/share"));
        assert!(content.contains("custom-dir"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn build_profile_system_agent() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        let config = Config::default();

        let profile = build_sandbox_profile(&ws, "system", true, None, &config, None);
        assert!(profile.rw_dirs.iter().any(|d| d.contains(tmp.path().to_str().unwrap())));
        assert!(profile.ro_dirs.is_empty());
        assert!(profile.network_outbound);
    }

    #[test]
    fn build_profile_normal_agent() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        let config = Config::default();

        let profile = build_sandbox_profile(&ws, "dev-1", false, None, &config, None);
        assert_eq!(profile.rw_dirs.len(), 1);
        assert!(profile.rw_dirs[0].contains("dev-1"));
        assert!(profile.ro_dirs.iter().any(|d| d.contains("config.json")));
        assert!(profile.ro_dirs.iter().any(|d| d.contains("skills")));
    }

    #[test]
    fn build_profile_team_agent() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        let config = Config::default();

        let profile = build_sandbox_profile(&ws, "dev-1", false, Some("team-x"), &config, None);
        assert!(profile.rw_dirs.iter().any(|d| d.contains("shared")));
        assert!(profile.ro_dirs.iter().any(|d| d.contains("team.json")));
        assert!(profile.ro_dirs.iter().any(|d| d.contains("tasklist.json")));
    }

    #[test]
    fn build_safehouse_args_includes_rw_and_ro() {
        let profile = SandboxProfile {
            rw_dirs: vec!["/tmp/work".into()],
            ro_dirs: vec!["/etc/config".into()],
            network_outbound: true,
            extra_read: vec![],
            extra_write: vec![],
        };
        let args = build_safehouse_args(&profile);
        assert!(args.contains(&"--add-dirs".to_string()));
        assert!(args.contains(&"/tmp/work".to_string()));
        assert!(args.contains(&"--add-dirs-ro".to_string()));
        assert!(args.contains(&"/etc/config".to_string()));
        assert!(args.contains(&"--enable".to_string()));
        assert!(args.contains(&"process-control,shell-init".to_string()));
        assert_eq!(args.last().unwrap(), "--");
    }
}
