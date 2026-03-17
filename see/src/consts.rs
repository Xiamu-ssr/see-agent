/// Agent loop safety limits
pub const MAX_CONSECUTIVE_ERRORS: usize = 3;
pub const NO_PROGRESS_LIMIT: usize = 3;
pub const REPEAT_WARN_LIMIT: usize = 3;
pub const REPEAT_ABORT_LIMIT: usize = 5;
pub const MAX_STEPS_WITHOUT_SCREENSHOT: usize = 5;

/// Config defaults
pub const DEFAULT_MAX_STEPS: usize = 50;
pub const DEFAULT_CONTEXT_WINDOW: usize = 200_000;
pub const DEFAULT_COMPACT_TARGET_RATIO: f64 = 0.75;
pub const DEFAULT_COMPACT_KEEP_RECENT: usize = 8;
pub const DEFAULT_MAX_CONTEXT_IMAGES: usize = 5;
pub const DEFAULT_SCREENSHOT_INTERVAL_MS: u64 = 800;
pub const DEFAULT_TOOL_DELAY_MS: u64 = 200;
pub const DEFAULT_LLM_MAX_TOKENS: u32 = 4096;
pub const DEFAULT_SUMMARIZE_MAX_TOKENS: u32 = 2048;

/// File injection limits
pub const MAX_FILE_CHARS: usize = 20_000;
pub const MAX_TOTAL_INJECTION_CHARS: usize = 100_000;
pub const MAX_SUMMARIZE_MSG_CHARS: usize = 500;

/// Worker
pub const WORKER_HEARTBEAT_SECS: u64 = 300;
pub const WORKER_SIGNAL_TIMEOUT_SECS: u64 = 5;

/// Screen scaling target resolutions
pub const SCALE_TARGETS: [(u32, u32); 3] = [(1024, 768), (1280, 800), (1366, 768)];
pub const ASPECT_TOLERANCE: f64 = 0.05;

/// Coordinate rounding for repeat detection
pub const COORDINATE_ROUND_TO: i32 = 10;

/// Token estimation
pub const CHARS_PER_TOKEN: usize = 4;
pub const TOKENS_PER_IMAGE: usize = 765;

/// Server
pub const DEFAULT_SERVER_PORT: u16 = 28789;

/// Version
pub const VERSION: &str = "0.1.0";
