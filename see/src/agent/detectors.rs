use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::consts::{
    COORDINATE_ROUND_TO, MAX_STEPS_WITHOUT_SCREENSHOT, NO_PROGRESS_LIMIT, REPEAT_ABORT_LIMIT,
    REPEAT_WARN_LIMIT,
};

// ---------------------------------------------------------------------------
// Detector actions
// ---------------------------------------------------------------------------

/// What the loop should do in response to a detector check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectorAction {
    /// Nothing to do.
    Ok,
    /// Inject a warning hint into the conversation.
    Warn(String),
    /// Abort the task immediately.
    Abort(String),
}

// ---------------------------------------------------------------------------
// No-progress detector
// ---------------------------------------------------------------------------

/// Detects when the screen hasn't changed for N consecutive steps.
pub struct NoProgressDetector {
    last_hash: Option<u64>,
    count: usize,
}

impl NoProgressDetector {
    pub fn new() -> Self {
        Self {
            last_hash: None,
            count: 0,
        }
    }

    /// Call after a tool returns a screenshot. `screenshot_prefix` is the
    /// first ~1000 chars of the base64 image data.
    pub fn check(&mut self, screenshot_prefix: &str) -> DetectorAction {
        let hash = hash_str(screenshot_prefix);

        if self.last_hash == Some(hash) {
            self.count += 1;
            if self.count >= NO_PROGRESS_LIMIT {
                let msg = format!(
                    "[系统提示] 屏幕已经连续 {} 步没有变化。请重新分析当前状态，尝试不同的策略。",
                    self.count
                );
                self.count = 0;
                return DetectorAction::Warn(msg);
            }
        } else {
            self.count = 0;
        }
        self.last_hash = Some(hash);
        DetectorAction::Ok
    }

    /// Call when a tool does NOT return a screenshot (resets nothing).
    pub fn no_screenshot(&self) -> DetectorAction {
        DetectorAction::Ok
    }
}

// ---------------------------------------------------------------------------
// Repeated action detector
// ---------------------------------------------------------------------------

/// Detects when the agent performs the same action repeatedly.
pub struct RepeatDetector {
    last_key: Option<String>,
    count: usize,
}

impl RepeatDetector {
    pub fn new() -> Self {
        Self {
            last_key: None,
            count: 0,
        }
    }

    /// Call after each tool execution. Returns the recommended action.
    pub fn check(&mut self, tool_name: &str, args: &serde_json::Value) -> DetectorAction {
        let key = action_key(tool_name, args);

        if self.last_key.as_deref() == Some(&key) {
            self.count += 1;
            if self.count >= REPEAT_ABORT_LIMIT {
                return DetectorAction::Abort(format!(
                    "重复执行相同操作 {} 次，任务中止。",
                    self.count
                ));
            }
            if self.count >= REPEAT_WARN_LIMIT {
                return DetectorAction::Warn(format!(
                    "[系统提示] 你已经重复执行相同操作 {} 次。请尝试完全不同的方法。",
                    self.count
                ));
            }
        } else {
            self.count = 1;
        }
        self.last_key = Some(key);
        DetectorAction::Ok
    }
}

// ---------------------------------------------------------------------------
// No-screenshot warning
// ---------------------------------------------------------------------------

/// Warns when the agent hasn't taken a screenshot for N steps.
pub struct NoScreenshotDetector {
    steps_since: usize,
}

impl NoScreenshotDetector {
    pub fn new() -> Self {
        Self { steps_since: 0 }
    }

    /// Call when a tool returns images.
    pub fn got_screenshot(&mut self) {
        self.steps_since = 0;
    }

    /// Call after each step. If no screenshot was returned, increments counter.
    pub fn step_without_screenshot(&mut self) -> DetectorAction {
        self.steps_since += 1;
        if self.steps_since >= MAX_STEPS_WITHOUT_SCREENSHOT {
            self.steps_since = 0;
            DetectorAction::Warn(
                "[系统提示] 你已经连续多步没有查看屏幕。建议使用 screenshot 工具确认当前状态。"
                    .to_owned(),
            )
        } else {
            DetectorAction::Ok
        }
    }
}

// ---------------------------------------------------------------------------
// Consecutive error tracker
// ---------------------------------------------------------------------------

/// Tracks consecutive errors and triggers abort at threshold.
pub struct ErrorTracker {
    count: usize,
    max: usize,
}

impl ErrorTracker {
    pub fn new(max: usize) -> Self {
        Self { count: 0, max }
    }

    /// Record a success — resets the counter.
    pub fn success(&mut self) {
        self.count = 0;
    }

    /// Record an error. Returns true if the error limit has been reached.
    pub fn error(&mut self) -> bool {
        self.count += 1;
        self.count >= self.max
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Normalize a tool call into a comparable key.
///
/// Coordinates are rounded to the nearest COORDINATE_ROUND_TO (10).
fn action_key(tool_name: &str, args: &serde_json::Value) -> String {
    let mut normalized = args.clone();
    if let Some(obj) = normalized.as_object_mut() {
        for key in ["x", "y", "start_x", "start_y", "end_x", "end_y"] {
            if let Some(v) = obj.get(key).and_then(|v| v.as_i64()) {
                let rounded = (v as i32 / COORDINATE_ROUND_TO) * COORDINATE_ROUND_TO;
                obj.insert(key.to_owned(), serde_json::json!(rounded));
            }
        }
    }
    // Sort keys for deterministic comparison
    format!("{tool_name}:{normalized}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn no_progress_triggers_after_limit() {
        let mut d = NoProgressDetector::new();
        // 1st call: sets hash, count=0
        assert_eq!(d.check("same_image"), DetectorAction::Ok);
        // 2nd: match, count=1
        assert_eq!(d.check("same_image"), DetectorAction::Ok);
        // 3rd: match, count=2
        assert_eq!(d.check("same_image"), DetectorAction::Ok);
        // 4th: match, count=3 >= NO_PROGRESS_LIMIT → Warn
        match d.check("same_image") {
            DetectorAction::Warn(msg) => assert!(msg.contains("没有变化")),
            other => panic!("expected Warn, got {other:?}"),
        }
        // Counter reset, so next identical is Ok
        assert_eq!(d.check("same_image"), DetectorAction::Ok);
    }

    #[test]
    fn no_progress_resets_on_change() {
        let mut d = NoProgressDetector::new();
        d.check("img_a");
        d.check("img_a");
        // Change resets
        assert_eq!(d.check("img_b"), DetectorAction::Ok);
        assert_eq!(d.check("img_b"), DetectorAction::Ok);
        // Only 2 repeats, not 3
        assert_eq!(d.check("img_c"), DetectorAction::Ok);
    }

    #[test]
    fn repeat_warns_then_aborts() {
        let mut d = RepeatDetector::new();
        let args = json!({"x": 100, "y": 200});

        assert_eq!(d.check("click", &args), DetectorAction::Ok); // count=1
        assert_eq!(d.check("click", &args), DetectorAction::Ok); // count=2
        match d.check("click", &args) {
            // count=3
            DetectorAction::Warn(msg) => assert!(msg.contains("重复")),
            other => panic!("expected Warn, got {other:?}"),
        }
        assert_eq!(d.check("click", &args), DetectorAction::Warn(_warn_msg())); // count=4
        match d.check("click", &args) {
            // count=5
            DetectorAction::Abort(msg) => assert!(msg.contains("中止")),
            other => panic!("expected Abort, got {other:?}"),
        }
    }

    fn _warn_msg() -> String {
        format!(
            "[系统提示] 你已经重复执行相同操作 {} 次。请尝试完全不同的方法。",
            4
        )
    }

    #[test]
    fn repeat_resets_on_different_action() {
        let mut d = RepeatDetector::new();
        d.check("click", &json!({"x": 100, "y": 200}));
        d.check("click", &json!({"x": 100, "y": 200}));
        // Different action resets
        assert_eq!(
            d.check("click", &json!({"x": 300, "y": 400})),
            DetectorAction::Ok
        );
    }

    #[test]
    fn repeat_coordinate_rounding() {
        let mut d = RepeatDetector::new();
        // These should be treated as the same action (both round to 100, 200)
        d.check("click", &json!({"x": 103, "y": 207}));
        d.check("click", &json!({"x": 105, "y": 204}));
        // count=2, next should be 3 => warn
        match d.check("click", &json!({"x": 101, "y": 209})) {
            DetectorAction::Warn(_) => {}
            other => panic!("expected Warn, got {other:?}"),
        }
    }

    #[test]
    fn no_screenshot_warns_after_limit() {
        let mut d = NoScreenshotDetector::new();
        for _ in 0..4 {
            assert_eq!(d.step_without_screenshot(), DetectorAction::Ok);
        }
        // 5th step triggers
        match d.step_without_screenshot() {
            DetectorAction::Warn(msg) => assert!(msg.contains("screenshot")),
            other => panic!("expected Warn, got {other:?}"),
        }
    }

    #[test]
    fn no_screenshot_resets_on_image() {
        let mut d = NoScreenshotDetector::new();
        d.step_without_screenshot();
        d.step_without_screenshot();
        d.got_screenshot();
        // Counter reset, need 5 more steps
        for _ in 0..4 {
            assert_eq!(d.step_without_screenshot(), DetectorAction::Ok);
        }
    }

    #[test]
    fn error_tracker() {
        let mut t = ErrorTracker::new(3);
        assert!(!t.error());
        assert!(!t.error());
        assert!(t.error()); // 3rd error hits limit
        t.success();
        assert_eq!(t.count(), 0);
    }

    #[test]
    fn action_key_normalization() {
        let k1 = action_key("click", &json!({"x": 103, "y": 207}));
        let k2 = action_key("click", &json!({"x": 105, "y": 204}));
        assert_eq!(k1, k2); // both round to x=100, y=200
    }

    #[test]
    fn action_key_different_tools() {
        let k1 = action_key("click", &json!({"x": 100}));
        let k2 = action_key("scroll", &json!({"x": 100}));
        assert_ne!(k1, k2);
    }
}
