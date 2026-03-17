use std::path::{Path, PathBuf};

use crate::types::{Config, WorkspaceDir};

/// Placeholder token in .sb profiles replaced with the actual home directory.
const HOME_PLACEHOLDER: &str = "__SAFEHOUSE_REPLACE_ME_WITH_ABSOLUTE_HOME_DIR__";

/// Embedded core sandbox profiles.
const PROFILE_BASE: &str = include_str!("profiles/00-base.sb");
const PROFILE_SYSTEM: &str = include_str!("profiles/10-system-runtime.sb");
const PROFILE_NETWORK: &str = include_str!("profiles/20-network.sb");
const PROFILE_SEE_AGENT: &str = include_str!("profiles/see-agent-base.sb");

/// Generate a combined sandbox profile for an agent.
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
        // Toolchain profiles
        load_optional_profile(&mut parts, dir, "30-toolchains/python.sb");
        load_optional_profile(&mut parts, dir, "30-toolchains/runtime-managers.sb");

        // Shared agent context
        load_optional_profile(&mut parts, dir, "40-shared/agent-common.sb");

        // Core integrations
        load_optional_profile(&mut parts, dir, "50-integrations-core/git.sb");
        load_optional_profile(&mut parts, dir, "50-integrations-core/scm-clis.sb");

        // Optional: macOS GUI + clipboard for screen agents
        load_optional_profile(&mut parts, dir, "55-integrations-optional/macos-gui.sb");
        load_optional_profile(&mut parts, dir, "55-integrations-optional/clipboard.sb");
        load_optional_profile(&mut parts, dir, "55-integrations-optional/shell-init.sb");
    }

    // Combine all fragments
    let mut combined = parts.join("\n\n");

    // Replace HOME placeholder
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/Users/unknown".to_owned());
    combined = combined.replace(HOME_PLACEHOLDER, &home);

    // Add per-agent dynamic rules
    let agent_dir = workspace.agent(agent_id);
    combined.push_str(&format!(
        "\n\n;; Per-agent directory access\n(allow file-read* file-write*\n    (subpath \"{}\")\n)\n",
        agent_dir.path().display()
    ));

    // Add extra_read / extra_write from config
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

    // Write to /tmp
    let output_path = PathBuf::from(format!("/tmp/see-agent-{agent_id}.sb"));
    let _ = std::fs::write(&output_path, &combined);

    output_path
}

/// Try to find the profiles directory relative to the binary or in known locations.
fn find_profiles_dir() -> Option<PathBuf> {
    // Check next to the executable
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent()?.join("profiles");
        if dir.is_dir() {
            return Some(dir);
        }
    }

    // Check in the see crate source (development mode)
    let dev_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sandbox/profiles");
    if dev_path.is_dir() {
        return Some(dev_path);
    }

    None
}

/// Load an optional profile file and store the owned string.
/// Uses a Vec of owned strings to avoid lifetime issues.
fn load_optional_profile(parts: &mut Vec<&str>, _dir: &Path, _relative: &str) {
    // For now, optional profiles from disk are not loaded at runtime.
    // Core profiles are embedded via include_str!().
    // This function exists as a hook for future extension.
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
        assert!(content.contains("test-sandbox")); // agent dir path
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
}
