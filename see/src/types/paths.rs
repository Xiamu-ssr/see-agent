use std::path::{Path, PathBuf};

/// Root workspace directory ~/.see-agent/
#[derive(Debug, Clone)]
pub struct WorkspaceDir(PathBuf);

impl WorkspaceDir {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn config(&self) -> PathBuf {
        self.0.join("config.json")
    }

    pub fn agents(&self) -> PathBuf {
        self.0.join("agents")
    }

    pub fn teams(&self) -> PathBuf {
        self.0.join("teams")
    }

    pub fn skills(&self) -> PathBuf {
        self.0.join("skills")
    }

    pub fn logs(&self) -> PathBuf {
        self.0.join("logs")
    }

    pub fn agent(&self, id: &str) -> AgentDir {
        AgentDir(self.agents().join(id))
    }

    pub fn team(&self, id: &str) -> TeamDir {
        TeamDir(self.teams().join(id))
    }

    pub fn system_agent(&self) -> AgentDir {
        self.agent("system")
    }

    pub fn server_pid(&self) -> PathBuf {
        self.0.join("server.pid")
    }

    pub fn server_log(&self) -> PathBuf {
        self.0.join("server.log")
    }
}

/// Single agent directory ~/.see-agent/agents/{id}/
#[derive(Debug, Clone)]
pub struct AgentDir(PathBuf);

impl AgentDir {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn agent_json(&self) -> PathBuf {
        self.0.join("agent.json")
    }

    pub fn identity_md(&self) -> PathBuf {
        self.0.join("IDENTITY.md")
    }

    pub fn soul_md(&self) -> PathBuf {
        self.0.join("SOUL.md")
    }

    pub fn agents_md(&self) -> PathBuf {
        self.0.join("AGENTS.md")
    }

    pub fn inbox(&self) -> PathBuf {
        self.0.join("inbox.jsonl")
    }

    pub fn inbox_cursor(&self) -> PathBuf {
        self.0.join("inbox_cursor.json")
    }

    pub fn memory_dir(&self) -> PathBuf {
        self.0.join("memory")
    }

    pub fn memory_md(&self) -> PathBuf {
        self.0.join("memory").join("MEMORY.md")
    }

    pub fn session(&self) -> SessionDir {
        SessionDir(self.0.join("session"))
    }
}

/// Session directory inside an agent
#[derive(Debug, Clone)]
pub struct SessionDir(PathBuf);

impl SessionDir {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn meta(&self) -> PathBuf {
        self.0.join("meta.json")
    }

    pub fn messages(&self) -> PathBuf {
        self.0.join("messages.jsonl")
    }

    pub fn session_log(&self) -> PathBuf {
        self.0.join("session.log")
    }

    pub fn system_prompt_log(&self) -> PathBuf {
        self.0.join("system_prompt_log.md")
    }

    pub fn screenshots(&self) -> PathBuf {
        self.0.join("screenshots")
    }

    /// Screenshot file for a given step number
    pub fn screenshot_file(&self, step: u32) -> PathBuf {
        self.0
            .join("screenshots")
            .join(format!("step_{step:03}.webp"))
    }
}

/// Team directory ~/.see-agent/teams/{id}/
#[derive(Debug, Clone)]
pub struct TeamDir(PathBuf);

impl TeamDir {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn team_json(&self) -> PathBuf {
        self.0.join("team.json")
    }

    pub fn messages(&self) -> PathBuf {
        self.0.join("messages.jsonl")
    }

    pub fn tasklist(&self) -> PathBuf {
        self.0.join("tasklist.json")
    }

    pub fn shared(&self) -> PathBuf {
        self.0.join("shared")
    }
}
