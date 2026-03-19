use std::path::Path;

use crate::consts::{MAX_FILE_CHARS, MAX_TOTAL_INJECTION_CHARS};
use crate::types::{SkillInfo, TeamMember};

// ---------------------------------------------------------------------------
// Prompt context
// ---------------------------------------------------------------------------

/// All inputs needed to build the system prompt.
pub struct PromptContext<'a> {
    pub agent_dir: &'a Path,
    pub max_steps: u32,
    pub skills: &'a [SkillInfo],
    pub team: Option<TeamContext<'a>>,
}

/// Team context for multi-agent collaboration.
pub struct TeamContext<'a> {
    pub name: &'a str,
    pub my_role: &'a str, // "leader" or "worker"
    pub leader_id: &'a str,
    pub members: &'a [TeamMember],
    pub shared_dir: Option<&'a str>,
}

// ---------------------------------------------------------------------------
// System prompt builder
// ---------------------------------------------------------------------------

/// Build the full system prompt following MentalModel.md ordering:
///
/// 1. IDENTITY.md
/// 2. AGENTS.md
/// 3. SOUL.md
/// 4. memory/MEMORY.md
/// 5. Constraints (max_steps + safety rules)
/// 6. Skills (optional)
/// 7. Team context (optional)
pub fn build_system_prompt(ctx: &PromptContext) -> String {
    let mut sections = Vec::new();

    // 1-4: Agent files
    let files = inject_agent_files(ctx.agent_dir);
    if !files.is_empty() {
        sections.push(files);
    }

    // 5: Constraints
    sections.push(format!(
        "最多执行 {} 步。不要执行危险的 shell 命令。不要访问或泄露密码、密钥等敏感信息。",
        ctx.max_steps
    ));

    // 6: Skills
    let skills_section = build_skills_section(ctx.skills);
    if !skills_section.is_empty() {
        sections.push(skills_section);
    }

    // 7: Team context
    if let Some(ref team) = ctx.team {
        sections.push(build_team_section(team));
    }

    sections.join("\n\n")
}

// ---------------------------------------------------------------------------
// Agent file injection
// ---------------------------------------------------------------------------

/// Read and concatenate agent files in order: IDENTITY.md, AGENTS.md, SOUL.md, memory/MEMORY.md.
///
/// Each file is truncated to MAX_FILE_CHARS; total is capped at MAX_TOTAL_INJECTION_CHARS.
fn inject_agent_files(agent_dir: &Path) -> String {
    let file_names = [
        "IDENTITY.md",
        "AGENTS.md",
        "SOUL.md",
        "memory/MEMORY.md",
    ];

    let mut parts = Vec::new();
    let mut total_len = 0usize;

    for name in &file_names {
        let path = agent_dir.join(name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let truncated = if content.len() > MAX_FILE_CHARS {
                &content[..MAX_FILE_CHARS]
            } else {
                &content
            };
            let remaining = MAX_TOTAL_INJECTION_CHARS.saturating_sub(total_len);
            if remaining == 0 {
                break;
            }
            let to_add = if truncated.len() > remaining {
                &truncated[..remaining]
            } else {
                truncated
            };
            total_len += to_add.len();
            parts.push(to_add.to_owned());
        }
    }

    parts.join("\n\n")
}

// ---------------------------------------------------------------------------
// Skills section
// ---------------------------------------------------------------------------

fn build_skills_section(skills: &[SkillInfo]) -> String {
    let active: Vec<&SkillInfo> = skills.iter().filter(|s| !s.blocked).collect();
    if active.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "## Skills".to_owned(),
        String::new(),
        "以下是你可以使用的 Skills。需要时用 read 工具读取 SKILL.md 获取详细指南。".to_owned(),
        String::new(),
        "<available_skills>".to_owned(),
    ];
    for skill in &active {
        lines.push("  <skill>".to_owned());
        lines.push(format!("    <name>{}</name>", skill.name));
        if !skill.description.is_empty() {
            lines.push(format!("    <description>{}</description>", skill.description));
        }
        lines.push(format!("    <location>{}</location>", skill.path));
        lines.push("  </skill>".to_owned());
    }
    lines.push("</available_skills>".to_owned());
    lines.push(String::new());
    lines.push("使用规则：".to_owned());
    lines.push("- 如果恰好有一个 skill 适用：用 read 工具读取其 SKILL.md，然后按照指南执行".to_owned());
    lines.push("- 如果多个可能适用：选最具体的一个读取".to_owned());
    lines.push("- 如果没有适用的：不要读取任何 SKILL.md".to_owned());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Team context section
// ---------------------------------------------------------------------------

const TEAM_LEADER_TEMPLATE: &str = include_str!("../../../templates/team_leader_prompt.md");
const TEAM_MEMBER_TEMPLATE: &str = include_str!("../../../templates/team_member_prompt.md");

fn build_team_section(team: &TeamContext) -> String {
    let members_str = team
        .members
        .iter()
        .map(|m| {
            let location = if m.endpoint.is_some() { " (远程)" } else { "" };
            format!("- {} ({}){}", m.id, m.role, location)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let template = if team.my_role == "leader" {
        TEAM_LEADER_TEMPLATE
    } else {
        TEAM_MEMBER_TEMPLATE
    };

    let mut section = template
        .replace("{{team_name}}", team.name)
        .replace("{{my_role}}", team.my_role)
        .replace("{{leader_id}}", team.leader_id)
        .replace("{{members}}", &members_str);

    if let Some(shared_path) = team.shared_dir {
        section.push_str(&format!("\n\n## Team Shared Workspace\nFiles in {} are accessible to all team members.\nUse this directory for shared artifacts, notes, and outputs.", shared_path));
    }

    format!("<TEAM_CONTEXT>\n{}\n</TEAM_CONTEXT>", section.trim())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_agent_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("IDENTITY.md"), "I am a test agent.").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "Rule 1: be careful.").unwrap();
        std::fs::write(dir.path().join("SOUL.md"), "I am efficient.").unwrap();
        let mem_dir = dir.path().join("memory");
        std::fs::create_dir_all(&mem_dir).unwrap();
        std::fs::write(mem_dir.join("MEMORY.md"), "User prefers dark mode.").unwrap();
        dir
    }

    #[test]
    fn build_prompt_includes_all_sections() {
        let dir = setup_agent_dir();
        let ctx = PromptContext {
            agent_dir: dir.path(),
            max_steps: 50,
            skills: &[],
            team: None,
        };

        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("I am a test agent."));
        assert!(prompt.contains("Rule 1: be careful."));
        assert!(prompt.contains("I am efficient."));
        assert!(prompt.contains("User prefers dark mode."));
        assert!(prompt.contains("最多执行 50 步"));
    }

    #[test]
    fn build_prompt_with_skills() {
        let dir = setup_agent_dir();
        let skills = vec![
            SkillInfo {
                name: "web_search".to_owned(),
                description: "Search the web".to_owned(),
                body: String::new(),
                path: String::new(),
                requires_bins: vec![],
                requires_env: vec![],
                requires_any_bins: vec![],
                blocked: false,
                block_reason: None,
            },
            SkillInfo {
                name: "blocked_skill".to_owned(),
                description: "This is blocked".to_owned(),
                body: String::new(),
                path: String::new(),
                requires_bins: vec![],
                requires_env: vec![],
                requires_any_bins: vec![],
                blocked: true,
                block_reason: Some("missing binary".to_owned()),
            },
        ];

        let ctx = PromptContext {
            agent_dir: dir.path(),
            max_steps: 30,
            skills: &skills,
            team: None,
        };

        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("<name>web_search</name>"));
        assert!(prompt.contains("<description>Search the web</description>"));
        // Blocked skill should not appear
        assert!(!prompt.contains("blocked_skill"));
    }

    #[test]
    fn build_prompt_with_team() {
        let dir = setup_agent_dir();
        let members = vec![
            TeamMember {
                id: "agent-a".to_owned(),
                role: "leader".to_owned(),
                endpoint: None,
            },
            TeamMember {
                id: "agent-b".to_owned(),
                role: "researcher".to_owned(),
                endpoint: Some("192.168.1.10:28789".to_owned()),
            },
        ];

        let ctx = PromptContext {
            agent_dir: dir.path(),
            max_steps: 50,
            skills: &[],
            team: Some(TeamContext {
                name: "research-team",
                my_role: "leader",
                leader_id: "agent-a",
                members: &members,
                shared_dir: None,
            }),
        };

        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("<TEAM_CONTEXT>"));
        assert!(prompt.contains("research-team"));
        assert!(prompt.contains("agent-b (researcher) (远程)"));
        assert!(prompt.contains("send_message"));
    }

    #[test]
    fn system_prompt_includes_shared_workspace_path() {
        let dir = setup_agent_dir();
        let members = vec![TeamMember {
            id: "agent-a".to_owned(),
            role: "worker".to_owned(),
            endpoint: None,
        }];

        let ctx = PromptContext {
            agent_dir: dir.path(),
            max_steps: 50,
            skills: &[],
            team: Some(TeamContext {
                name: "my-team",
                my_role: "leader",
                leader_id: "agent-a",
                members: &members,
                shared_dir: Some("/workspace/teams/abc123/shared"),
            }),
        };

        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("Team Shared Workspace"));
        assert!(prompt.contains("/workspace/teams/abc123/shared"));
        assert!(prompt.contains("accessible to all team members"));
    }

    #[test]
    fn system_prompt_no_shared_workspace_without_dir() {
        let dir = setup_agent_dir();
        let members = vec![TeamMember {
            id: "agent-a".to_owned(),
            role: "worker".to_owned(),
            endpoint: None,
        }];

        let ctx = PromptContext {
            agent_dir: dir.path(),
            max_steps: 50,
            skills: &[],
            team: Some(TeamContext {
                name: "my-team",
                my_role: "leader",
                leader_id: "agent-a",
                members: &members,
                shared_dir: None,
            }),
        };

        let prompt = build_system_prompt(&ctx);
        assert!(!prompt.contains("Team Shared Workspace"));
    }

    #[test]
    fn system_prompt_includes_team_members() {
        let dir = setup_agent_dir();
        let members = vec![
            TeamMember {
                id: "alice".to_owned(),
                role: "leader".to_owned(),
                endpoint: None,
            },
            TeamMember {
                id: "bob".to_owned(),
                role: "dev".to_owned(),
                endpoint: None,
            },
            TeamMember {
                id: "charlie".to_owned(),
                role: "tester".to_owned(),
                endpoint: Some("10.0.0.5:9000".to_owned()),
            },
        ];

        let ctx = PromptContext {
            agent_dir: dir.path(),
            max_steps: 50,
            skills: &[],
            team: Some(TeamContext {
                name: "dev-team",
                my_role: "leader",
                leader_id: "alice",
                members: &members,
                shared_dir: None,
            }),
        };

        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("alice (leader)"));
        assert!(prompt.contains("bob (dev)"));
        assert!(prompt.contains("charlie (tester) (远程)"));
    }

    #[test]
    fn system_prompt_leader_instructions() {
        let dir = setup_agent_dir();
        let members = vec![TeamMember {
            id: "alice".to_owned(),
            role: "leader".to_owned(),
            endpoint: None,
        }];

        let ctx = PromptContext {
            agent_dir: dir.path(),
            max_steps: 50,
            skills: &[],
            team: Some(TeamContext {
                name: "t1",
                my_role: "leader",
                leader_id: "alice",
                members: &members,
                shared_dir: None,
            }),
        };

        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("create_task"));
        assert!(prompt.contains("assign_task"));
        assert!(prompt.contains("的领导（leader）"));
    }

    #[test]
    fn system_prompt_worker_instructions() {
        let dir = setup_agent_dir();
        let members = vec![
            TeamMember {
                id: "alice".to_owned(),
                role: "leader".to_owned(),
                endpoint: None,
            },
            TeamMember {
                id: "bob".to_owned(),
                role: "dev".to_owned(),
                endpoint: None,
            },
        ];

        let ctx = PromptContext {
            agent_dir: dir.path(),
            max_steps: 50,
            skills: &[],
            team: Some(TeamContext {
                name: "t1",
                my_role: "worker",
                leader_id: "alice",
                members: &members,
                shared_dir: None,
            }),
        };

        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("claim_task"));
        assert!(prompt.contains("complete_task"));
        assert!(prompt.contains("alice")); // leader name in worker instructions
        assert!(!prompt.contains("的领导（leader）"));
    }

    #[test]
    fn inject_files_missing_some() {
        let dir = TempDir::new().unwrap();
        // Only write IDENTITY.md
        std::fs::write(dir.path().join("IDENTITY.md"), "Hello").unwrap();

        let result = inject_agent_files(dir.path());
        assert_eq!(result, "Hello");
    }

    #[test]
    fn inject_files_truncation() {
        let dir = TempDir::new().unwrap();
        // Write a file larger than MAX_FILE_CHARS
        let big = "x".repeat(MAX_FILE_CHARS + 1000);
        std::fs::write(dir.path().join("IDENTITY.md"), &big).unwrap();

        let result = inject_agent_files(dir.path());
        assert_eq!(result.len(), MAX_FILE_CHARS);
    }

    #[test]
    fn skills_section_empty_when_all_blocked() {
        let skills = vec![SkillInfo {
            name: "s1".to_owned(),
            description: "d1".to_owned(),
            body: String::new(),
            path: String::new(),
            requires_bins: vec![],
            requires_env: vec![],
            requires_any_bins: vec![],
            blocked: true,
            block_reason: Some("missing".to_owned()),
        }];

        let result = build_skills_section(&skills);
        assert!(result.is_empty());
    }

    #[test]
    fn skills_section_lazy_load_format() {
        let skills = vec![SkillInfo {
            name: "web_search".to_owned(),
            description: "Search the web".to_owned(),
            body: "Use this skill to search for information online.".to_owned(),
            path: "/path/to/SKILL.md".to_owned(),
            requires_bins: vec![],
            requires_env: vec![],
            requires_any_bins: vec![],
            blocked: false,
            block_reason: None,
        }];

        let result = build_skills_section(&skills);
        assert!(result.contains("<name>web_search</name>"));
        assert!(result.contains("<description>Search the web</description>"));
        assert!(result.contains("<location>/path/to/SKILL.md</location>"));
        // Body should NOT be injected (lazy load)
        assert!(!result.contains("Use this skill to search for information online."));
    }
}
