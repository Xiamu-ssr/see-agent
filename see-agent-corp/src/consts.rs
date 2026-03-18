/// Agent loop safety limits
pub const MAX_CONSECUTIVE_ERRORS: usize = 3;
pub const NO_PROGRESS_LIMIT: usize = 3;
pub const REPEAT_WARN_LIMIT: usize = 3;
pub const REPEAT_ABORT_LIMIT: usize = 5;
pub const MAX_STEPS_WITHOUT_SCREENSHOT: usize = 5;

/// Config defaults — numeric
pub const DEFAULT_MAX_STEPS: u32 = 50;
pub const DEFAULT_CONTEXT_WINDOW: u64 = 200_000;
pub const DEFAULT_COMPACT_TARGET_RATIO: f64 = 0.75;
pub const DEFAULT_COMPACT_KEEP_RECENT: u32 = 8;
pub const DEFAULT_MAX_CONTEXT_IMAGES: u32 = 5;
pub const DEFAULT_LLM_MAX_TOKENS: u32 = 4096;
pub const DEFAULT_SUMMARIZE_MAX_TOKENS: u32 = 2048;

/// Config defaults — strings
pub const DEFAULT_LLM_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_LLM_MODEL: &str = "gpt-4o";
pub const DEFAULT_SKILLS_DIR: &str = "~/.see-agent-corp/skills";
pub const DEFAULT_SANDBOX_PROFILE: &str = "default";
pub const DEFAULT_WEB_LANGUAGE: &str = "zh";

/// File injection limits
pub const MAX_FILE_CHARS: usize = 20_000;
pub const MAX_TOTAL_INJECTION_CHARS: usize = 100_000;
pub const MAX_SUMMARIZE_MSG_CHARS: usize = 500;

/// Worker
pub const WORKER_HEARTBEAT_SECS: u64 = 300;
pub const WORKER_SIGNAL_TIMEOUT_SECS: u64 = 5;
pub const WORKER_SHUTDOWN_TIMEOUT_SECS: u64 = 120;
pub const WORKER_WAKE_CHANNEL_SIZE: usize = 16;

/// Screen
pub const SCALE_TARGETS: [(u32, u32); 3] = [(1024, 768), (1280, 800), (1366, 768)];
pub const ASPECT_TOLERANCE: f64 = 0.05;
pub const VISION_LOW_DETAIL_MAX_DIM: u32 = 1024;
pub const SCREENSHOT_PREFIX_LEN: usize = 1000;

/// Coordinate rounding for repeat detection
pub const COORDINATE_ROUND_TO: i32 = 10;

/// Token estimation
pub const CHARS_PER_TOKEN: usize = 4;
pub const TOKENS_PER_IMAGE: usize = 765;

/// Server / Daemon
pub const DEFAULT_SERVER_PORT: u16 = 28789;
pub const DAEMON_STARTUP_WAIT_MS: u64 = 500;
pub const DAEMON_STOP_POLL_MS: u64 = 100;
pub const DAEMON_STOP_MAX_POLLS: usize = 1200;
pub const DAEMON_RESTART_WAIT_MS: u64 = 200;
pub const LOG_TAIL_LINES: usize = 200;

/// Workspace
pub const WORKSPACE_DIR_NAME: &str = ".see-agent-corp";

/// Environment collection
pub const ENVIRONMENT_TIMEOUT_SECS: u64 = 5;
pub const MAX_INSTALLED_APPS_LIST: usize = 40;

/// MCP
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// Tool execution
pub const SHELL_OUTPUT_MAX_CHARS: usize = 30_000;
pub const SHELL_TIMEOUT_SECS: u64 = 30;
pub const MEMORY_SEARCH_LIMIT: usize = 10;
pub const DEFAULT_WAIT_SECS: f64 = 2.0;
pub const DEFAULT_SCROLL_AMOUNT: i32 = 3;
pub const READ_MAX_FILE_CHARS: usize = 50_000;

/// Compression thresholds
pub const MICROCOMPACT_RATIO: f64 = 0.30;
pub const FULL_COMPACT_RATIO: f64 = 0.95;

/// Image lifecycle levels
pub const IMAGE_LEVEL1_COUNT: usize = 3;
pub const IMAGE_LEVEL2_COUNT: usize = 3;

/// Version
pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_heartbeat_secs_is_300() {
        assert_eq!(WORKER_HEARTBEAT_SECS, 300);
    }

    #[test]
    fn worker_shutdown_timeout_is_120() {
        assert_eq!(WORKER_SHUTDOWN_TIMEOUT_SECS, 120);
    }

    #[test]
    fn daemon_stop_polls_cover_shutdown_window() {
        let shutdown_window_ms = WORKER_SHUTDOWN_TIMEOUT_SECS * 1000;
        let poll_window_ms = DAEMON_STOP_MAX_POLLS as u64 * DAEMON_STOP_POLL_MS;
        assert!(
            poll_window_ms >= shutdown_window_ms,
            "daemon stop poll window ({poll_window_ms}ms) must cover shutdown timeout ({shutdown_window_ms}ms)"
        );
    }
}
