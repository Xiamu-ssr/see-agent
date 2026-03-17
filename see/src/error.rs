use thiserror::Error;

#[derive(Error, Debug)]
pub enum SeeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config error: {message}")]
    Config { message: String },

    #[error("Agent error: {message}")]
    Agent { message: String },

    #[error("Team error: {message}")]
    Team { message: String },

    #[error("Tool error: {tool}: {message}")]
    Tool { tool: String, message: String },

    #[error("Tool call parse error: {message}")]
    ToolCallParse { message: String },

    #[error("Brain error: {message}")]
    Brain { message: String },

    #[error("Session error: {message}")]
    Session { message: String },

    #[error("IPC error: {message}")]
    Ipc { message: String },

    #[error("MCP error: {message}")]
    Mcp { message: String },

    #[error("Not found: {what}")]
    NotFound { what: String },
}

pub type Result<T> = std::result::Result<T, SeeError>;
