use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tracing::{info, warn};

use crate::brain::Brain;
use crate::consts::{FULL_COMPACT_RATIO, MAX_CONSECUTIVE_ERRORS, MICROCOMPACT_RATIO};
use crate::eye::{scale_tool_args, Eye, Screenshot};
use crate::session::SessionStore;
use crate::tool::ToolRegistry;
use crate::types::{Config, SessionMessageType, ToolResult};

use super::context::{estimate_tokens, ConversationContext};
use super::detectors::{
    DetectorAction, ErrorTracker, NoProgressDetector, NoScreenshotDetector, RepeatDetector,
};
use super::loop_types::{RunResult, StepCallback, StepEvent, UserInputCallback};

/// Screen tool names that interact with the display.
const SCREEN_TOOLS: &[&str] = &[
    "screenshot",
    "click",
    "type_text",
    "scroll",
    "drag",
    "hotkey",
];

/// The main agent execution engine.
///
/// Supports two modes:
/// - **Mode A** (`run`): Single screen task — screenshot → LLM → tool → loop until `finished`.
/// - **Mode B** (`run_one_turn`): Inbox/ReAct — hot-reload prompt → LLM → tool → back to idle.
pub struct AgentLoop {
    brain: Box<dyn Brain>,
    eye: Arc<dyn Eye>,
    registry: ToolRegistry,
    config: Config,
    on_step: Option<StepCallback>,
    on_user_input: Option<UserInputCallback>,
    #[allow(dead_code)]
    agent_id: String,
    pub(crate) inject_queue: Vec<serde_json::Value>,
    compact_warned: bool,
    // Derived from config
    max_steps: u32,
    max_images: u32,
    /// Directory to save screenshots to disk (path-ref mode).
    /// When set, screenshots are saved as files and referenced by path
    /// instead of being stored inline as base64.
    screenshots_dir: Option<PathBuf>,
    /// Counter for screenshot file naming.
    screenshot_counter: u32,
    /// Optional session store for persisting messages to disk.
    session_store: Option<SessionStore>,
}

impl AgentLoop {
    pub fn new(
        brain: Box<dyn Brain>,
        eye: Arc<dyn Eye>,
        registry: ToolRegistry,
        config: Config,
        agent_id: String,
    ) -> Self {
        let max_steps = config.agent.max_steps;
        let max_images = config.agent.max_images;

        Self {
            brain,
            eye,
            registry,
            config,
            on_step: None,
            on_user_input: None,
            agent_id,
            inject_queue: Vec::new(),
            compact_warned: false,
            max_steps,
            max_images,
            screenshots_dir: None,
            screenshot_counter: 0,
            session_store: None,
        }
    }

    pub fn set_on_step(&mut self, cb: StepCallback) {
        self.on_step = Some(cb);
    }

    pub fn set_on_user_input(&mut self, cb: UserInputCallback) {
        self.on_user_input = Some(cb);
    }

    /// Set the directory where screenshots are saved to disk.
    /// When set, screenshots use path-ref mode (lazy base64 resolution at LLM call time).
    pub fn set_screenshots_dir(&mut self, dir: PathBuf) {
        self.screenshots_dir = Some(dir);
    }

    /// Set a session store for persisting messages to disk.
    pub fn set_session_store(&mut self, store: SessionStore) {
        self.session_store = Some(store);
    }

    /// Save a screenshot to disk and add it to context as a path reference.
    /// Falls back to inline base64 if no screenshots_dir is set or save fails.
    fn save_screenshot_ref(
        &mut self,
        ctx: &mut ConversationContext,
        screenshot: &Screenshot,
    ) {
        if let Some(ref dir) = self.screenshots_dir {
            self.screenshot_counter += 1;
            let file_path = dir.join(format!("step_{:03}.webp", self.screenshot_counter));
            match screenshot.save(&file_path) {
                Ok(()) => {
                    ctx.add_screenshot_ref(
                        file_path,
                        screenshot.detail(),
                        &screenshot.mime_type,
                    );
                    return;
                }
                Err(e) => {
                    warn!("failed to save screenshot to disk: {e}, falling back to inline");
                }
            }
        }
        // Fallback: inline base64
        ctx.add_screenshot(&screenshot.base64, screenshot.detail(), &screenshot.mime_type);
    }

    /// Check if the registry has any screen-interactive tools.
    fn has_screen_tools(&self) -> bool {
        let names = self.registry.names();
        SCREEN_TOOLS
            .iter()
            .any(|t| names.contains(&t.to_string()))
    }

    /// Convert tool schemas to serde_json::Value array for the Brain trait.
    fn schemas_to_values(
        &self,
        disabled: &[String],
    ) -> Vec<serde_json::Value> {
        self.registry
            .get_schemas_filtered(disabled)
            .into_iter()
            .filter_map(|s| serde_json::to_value(s).ok())
            .collect()
    }

    // -----------------------------------------------------------------------
    // Mode A: Single screen task
    // -----------------------------------------------------------------------

    /// Run a single screen task to completion.
    pub async fn run(
        &mut self,
        task: &str,
        system_prompt: &str,
        session_dir: &str,
    ) -> RunResult {
        let t0 = Instant::now();
        let has_screen = self.has_screen_tools();

        // Initial screenshot
        let screenshot = if has_screen {
            match self.eye.capture().await {
                Ok(ss) => Some(ss),
                Err(e) => {
                    warn!("initial capture failed: {e}");
                    None
                }
            }
        } else {
            None
        };

        // Build context
        let mut ctx = ConversationContext::new(system_prompt, self.max_images as usize, None);

        // Add initial user task
        if let Some(ref ss) = screenshot {
            ctx.add_user_task(task, &ss.base64, ss.detail(), &ss.mime_type);
        } else {
            ctx.add_user_task_text_only(task, "user", "collect");
        }

        // Run the core loop
        let result = self
            .run_loop(&mut ctx, screenshot.as_ref(), t0)
            .await;

        RunResult {
            session_id: String::new(),
            task_dir: session_dir.to_owned(),
            ..result
        }
    }

    /// Core loop for Mode A.
    async fn run_loop(
        &mut self,
        ctx: &mut ConversationContext,
        initial_scaled: Option<&Screenshot>,
        t0: Instant,
    ) -> RunResult {
        let mut error_tracker = ErrorTracker::new(MAX_CONSECUTIVE_ERRORS);
        let mut no_progress = NoProgressDetector::new();
        let mut repeat_detector = RepeatDetector::new();
        let mut no_screenshot = NoScreenshotDetector::new();
        let mut final_step = 0u32;

        // Screen dimensions for coordinate scaling
        let (model_w, model_h, screen_w, screen_h) = initial_scaled
            .map(|s| {
                (
                    s.width,
                    s.height,
                    s.screen_width.unwrap_or(s.width),
                    s.screen_height.unwrap_or(s.height),
                )
            })
            .unwrap_or((0, 0, 0, 0));

        let disabled: Vec<String> = self.config.tools.disabled.clone();
        let tools_schema = self.schemas_to_values(&disabled);

        for step in 0..self.max_steps {
            // Compact check
            self.maybe_compact(ctx).await;

            // Drain inject queue
            for msg in self.inject_queue.drain(..) {
                if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
                    let sender = msg
                        .get("sender")
                        .and_then(|v| v.as_str())
                        .unwrap_or("system");
                    ctx.add_user_reply(text, sender, "steer");
                }
            }

            // LLM call
            let messages = ctx.get_messages_for_llm();
            let response = match self.brain.chat(&messages, &tools_schema).await {
                Ok(r) => {
                    error_tracker.success();
                    r
                }
                Err(e) => {
                    warn!("LLM error at step {step}: {e}");
                    if error_tracker.error() {
                        return self.fail_result(
                            &format!("连续 LLM 错误 {} 次", error_tracker.count()),
                            final_step,
                            t0,
                        );
                    }
                    continue;
                }
            };

            // Add assistant message
            ctx.add_assistant(&response.raw);

            // No tool calls → task complete (LLM chose to end)
            if response.tool_calls.is_empty() {
                let summary = response.content.unwrap_or_default();
                return RunResult {
                    summary,
                    task_dir: String::new(),
                    total_steps: step + 1,
                    elapsed_seconds: t0.elapsed().as_secs_f64(),
                    success: true,
                    session_id: String::new(),
                };
            }

            // Execute tool calls serially
            let mut step_had_screenshot = false;
            for tc in &response.tool_calls {
                // Special: finished
                if tc.name == "finished" {
                    let summary = tc
                        .arguments
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Task completed.")
                        .to_owned();
                    ctx.add_tool_result(&tc.id, &summary, &[]);
                    return RunResult {
                        summary,
                        task_dir: String::new(),
                        total_steps: step + 1,
                        elapsed_seconds: t0.elapsed().as_secs_f64(),
                        success: true,
                        session_id: String::new(),
                    };
                }

                // Special: call_user
                if tc.name == "call_user" {
                    let question = tc
                        .arguments
                        .get("question")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned();
                    let reply = if let Some(ref cb) = self.on_user_input {
                        cb(question).await
                    } else {
                        "已处理，请继续".to_owned()
                    };
                    ctx.add_tool_result(&tc.id, &format!("User replied: {reply}"), &[]);
                    ctx.add_user_reply(&reply, "user", "steer");
                    continue;
                }

                // Coordinate scaling
                let mut exec_args = tc.arguments.clone();
                if model_w > 0 && screen_w > 0 {
                    scale_tool_args(
                        &tc.name,
                        &mut exec_args,
                        model_w,
                        model_h,
                        screen_w,
                        screen_h,
                    );
                }

                // Execute tool
                let result = match self.registry.execute(&tc.name, exec_args.clone()).await {
                    Ok(r) => {
                        error_tracker.success();
                        r
                    }
                    Err(e) => {
                        warn!("tool {} error: {e}", tc.name);
                        if error_tracker.error() {
                            return self.fail_result(
                                &format!("连续工具错误 {} 次", error_tracker.count()),
                                step + 1,
                                t0,
                            );
                        }
                        ctx.add_tool_result(&tc.id, &format!("Error: {e}"), &[]);
                        continue;
                    }
                };

                let result_text = result.text.clone();
                // Convert tool result images
                let ctx_images: Vec<super::context::ToolResultImage> = result
                    .images
                    .iter()
                    .map(|img| super::context::ToolResultImage {
                        base64: img.base64.clone(),
                        mime_type: img.mime_type.clone(),
                        detail: img.detail.clone(),
                    })
                    .collect();
                ctx.add_tool_result(&tc.id, &result_text, &ctx_images);
                final_step = step + 1;

                // Detectors — screenshot tool captures new image
                if tc.name == "screenshot"
                    && let Ok(new_ss) = self.eye.capture().await
                {
                    self.save_screenshot_ref(ctx, &new_ss);
                    step_had_screenshot = true;
                    no_screenshot.got_screenshot();

                    let prefix = &new_ss.base64[..std::cmp::min(crate::consts::SCREENSHOT_PREFIX_LEN, new_ss.base64.len())];
                    if let DetectorAction::Warn(msg) = no_progress.check(prefix) {
                        ctx.add_system_hint(&msg);
                    }
                }

                // Check for images in tool result
                if !result.images.is_empty() {
                    step_had_screenshot = true;
                    no_screenshot.got_screenshot();
                    if let Some(first_img) = result.images.first() {
                        let prefix =
                            &first_img.base64[..std::cmp::min(crate::consts::SCREENSHOT_PREFIX_LEN, first_img.base64.len())];
                        if let DetectorAction::Warn(msg) = no_progress.check(prefix) {
                            ctx.add_system_hint(&msg);
                        }
                    }
                }

                // Repeat detector
                match repeat_detector.check(&tc.name, &tc.arguments) {
                    DetectorAction::Abort(msg) => {
                        return self.fail_result(&msg, step + 1, t0);
                    }
                    DetectorAction::Warn(msg) => {
                        ctx.add_system_hint(&msg);
                    }
                    DetectorAction::Ok => {}
                }

                // Fire step callback
                if let Some(ref cb) = self.on_step {
                    let event = StepEvent {
                        step: step + 1,
                        max_steps: self.max_steps,
                        thought: response.content.clone().unwrap_or_default(),
                        tool_name: tc.name.clone(),
                        tool_args: tc.arguments.clone(),
                        tool_result: result_text.clone(),
                        screenshot_path: None,
                        screen_tool_args: if exec_args != tc.arguments {
                            Some(exec_args.clone())
                        } else {
                            None
                        },
                    };
                    cb(event).await;
                }
            }

            // No-screenshot warning (after all tool calls in this step)
            if !step_had_screenshot
                && let DetectorAction::Warn(msg) = no_screenshot.step_without_screenshot()
            {
                ctx.add_system_hint(&msg);
            }
        }

        // Budget exhausted
        self.fail_result("已达最大步数限制", final_step, t0)
    }

    // -----------------------------------------------------------------------
    // Mode B: Inbox/ReAct (one turn)
    // -----------------------------------------------------------------------

    /// Process one batch of inbox messages in ReAct style.
    pub async fn run_one_turn(
        &mut self,
        ctx: &mut ConversationContext,
        messages: &[serde_json::Value],
        system_prompt: &str,
    ) {
        ctx.update_system_prompt(system_prompt);

        for msg in messages {
            let text = msg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let sender = msg
                .get("sender")
                .and_then(|v| v.as_str())
                .unwrap_or("user");
            let priority = msg
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("collect");
            let formatted = if !sender.is_empty() && sender != "user" {
                format!("[{sender}] {text}")
            } else {
                text.to_owned()
            };
            ctx.add_user_reply(&formatted, sender, priority);
        }

        let disabled: Vec<String> = self.config.tools.disabled.clone();
        let tools_schema = self.schemas_to_values(&disabled);

        for _step in 0..self.max_steps {
            // Drain inject queue
            for msg in self.inject_queue.drain(..) {
                if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
                    let sender = msg
                        .get("sender")
                        .and_then(|v| v.as_str())
                        .unwrap_or("system");
                    ctx.add_user_reply(text, sender, "steer");
                }
            }

            let llm_messages = ctx.get_messages_for_llm();
            let response = match self.brain.chat(&llm_messages, &tools_schema).await {
                Ok(r) => r,
                Err(e) => {
                    warn!("LLM error in run_one_turn: {e}");
                    break;
                }
            };

            ctx.add_assistant(&response.raw);

            // Persist assistant message to session store
            if let Some(ref mut store) = self.session_store {
                let _ = store.append_message(SessionMessageType::Assistant, response.raw.clone());
            }

            if response.tool_calls.is_empty() {
                info!("no tool calls, returning to idle");
                break;
            }

            for tc in &response.tool_calls {
                if tc.name == "finished" {
                    info!("finished tool called in mode B, returning to idle");
                    return;
                }

                let result = match self.registry.execute(&tc.name, tc.arguments.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("tool {} error in mode B: {e}", tc.name);
                        ToolResult {
                            text: format!("Error: {e}"),
                            images: vec![],
                        }
                    }
                };

                ctx.add_tool_result(&tc.id, &result.text, &[]);

                // Persist tool result to session store
                if let Some(ref mut store) = self.session_store {
                    let _ = store.append_message(
                        SessionMessageType::ToolResult,
                        serde_json::json!({ "tool": tc.name, "content": result.text }),
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Compaction
    // -----------------------------------------------------------------------

    async fn maybe_compact(&mut self, ctx: &mut ConversationContext) {
        let messages = ctx.get_messages();
        let tokens = estimate_tokens(&messages);
        let window = self.config.agent.compact.context_window as f64;
        let keep_recent = self.config.agent.compact.keep_recent as usize;

        // Layer 2: Microcompact — clear old tool outputs (rules only, no LLM)
        let micro_threshold = (window * MICROCOMPACT_RATIO) as usize;
        if tokens >= micro_threshold {
            let saved = ctx.apply_microcompact(keep_recent);
            if saved > 0 {
                info!("microcompact saved ~{saved} tokens");
            }
        }

        // Layer 3: Full compact — LLM summarization
        let full_threshold = (window * FULL_COMPACT_RATIO) as usize;
        // Re-estimate after microcompact
        let tokens_after = estimate_tokens(&ctx.get_messages());
        if tokens_after < full_threshold {
            return;
        }

        if !self.compact_warned {
            self.compact_warned = true;
            ctx.add_system_hint(
                "[系统提示] 上下文即将达到窗口上限，请立即用 memory_write 保存重要信息，下一轮将执行上下文压缩。",
            );
            return;
        }

        self.compact_warned = false;

        let current_messages = ctx.get_messages();
        let end = current_messages.len().saturating_sub(keep_recent);
        if end <= 1 {
            return;
        }
        let to_summarize = &current_messages[1..end];

        match self.brain.summarize(to_summarize).await {
            Ok(summary) => {
                info!("full compact complete, summary length: {}", summary.len());
                ctx.apply_compaction(&summary, keep_recent);
            }
            Err(e) => {
                warn!("compaction summarize failed: {e}, skipping");
            }
        }
    }

    fn fail_result(&self, reason: &str, steps: u32, t0: Instant) -> RunResult {
        RunResult {
            summary: reason.to_owned(),
            task_dir: String::new(),
            total_steps: steps,
            elapsed_seconds: t0.elapsed().as_secs_f64(),
            success: false,
            session_id: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::{BrainResponse, ToolCallInfo};
    use crate::error::Result;
    use async_trait::async_trait;
    use serde_json::json;

    struct MockBrain {
        responses: std::sync::Mutex<Vec<BrainResponse>>,
    }

    impl MockBrain {
        fn new(responses: Vec<BrainResponse>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl Brain for MockBrain {
        async fn chat(
            &self,
            _messages: &[serde_json::Value],
            _tools: &[serde_json::Value],
        ) -> Result<BrainResponse> {
            let mut resps = self.responses.lock().unwrap();
            if resps.is_empty() {
                Ok(BrainResponse {
                    content: Some("No more responses".to_owned()),
                    tool_calls: vec![],
                    raw: json!({"role": "assistant", "content": "No more responses"}),
                })
            } else {
                Ok(resps.remove(0))
            }
        }

        async fn summarize(&self, _messages: &[serde_json::Value]) -> Result<String> {
            Ok("Summary of conversation.".to_owned())
        }
    }

    struct MockEye;

    #[async_trait]
    impl Eye for MockEye {
        async fn capture(&self) -> Result<crate::eye::Screenshot> {
            Err(crate::error::CorpError::Agent {
                message: "mock eye".to_owned(),
            })
        }
    }

    fn make_loop(responses: Vec<BrainResponse>) -> AgentLoop {
        let brain = Box::new(MockBrain::new(responses));
        let eye: Arc<dyn Eye> = Arc::new(MockEye);
        let registry = ToolRegistry::new();
        let config = Config::default();
        AgentLoop::new(brain, eye, registry, config, "test-agent".to_owned())
    }

    #[tokio::test]
    async fn run_returns_on_no_tool_calls() {
        let brain_resp = BrainResponse {
            content: Some("Task is done.".to_owned()),
            tool_calls: vec![],
            raw: json!({"role": "assistant", "content": "Task is done."}),
        };
        let mut agent = make_loop(vec![brain_resp]);
        let result = agent.run("do something", "sys prompt", "/tmp/test").await;
        assert!(result.success);
        assert_eq!(result.summary, "Task is done.");
        assert_eq!(result.total_steps, 1);
    }

    #[tokio::test]
    async fn run_returns_on_finished_tool() {
        let brain_resp = BrainResponse {
            content: Some("I'll finish.".to_owned()),
            tool_calls: vec![ToolCallInfo {
                id: "call_1".to_owned(),
                name: "finished".to_owned(),
                arguments: json!({"summary": "All done!"}),
            }],
            raw: json!({
                "role": "assistant",
                "content": "I'll finish.",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "finished", "arguments": "{\"summary\":\"All done!\"}"}
                }]
            }),
        };
        let mut agent = make_loop(vec![brain_resp]);
        let result = agent.run("do something", "sys prompt", "/tmp/test").await;
        assert!(result.success);
        assert_eq!(result.summary, "All done!");
    }

    #[tokio::test]
    async fn run_one_turn_adds_messages() {
        let brain_resp = BrainResponse {
            content: Some("Got it.".to_owned()),
            tool_calls: vec![],
            raw: json!({"role": "assistant", "content": "Got it."}),
        };
        let mut agent = make_loop(vec![brain_resp]);
        let mut ctx = ConversationContext::new("sys", 5, None);

        let msgs = vec![json!({"content": "hello", "sender": "user", "priority": "collect"})];
        agent.run_one_turn(&mut ctx, &msgs, "sys").await;

        // system + user_reply + assistant = 3
        assert_eq!(ctx.len(), 3);
    }

    #[test]
    fn save_screenshot_ref_writes_file_and_stores_path_ref() {
        let tmp = tempfile::TempDir::new().unwrap();
        let screenshots_dir = tmp.path().join("screenshots");

        let mut agent = make_loop(vec![]);
        agent.set_screenshots_dir(screenshots_dir.clone());

        let mut ctx = ConversationContext::new("sys", 5, None);

        // Create a mock screenshot with real base64 data
        use base64::Engine;
        let fake_bytes = b"fake-screenshot-data";
        let b64 = base64::engine::general_purpose::STANDARD.encode(fake_bytes);

        let screenshot = Screenshot {
            base64: b64,
            width: 800,
            height: 600,
            scale_factor: 1.0,
            mime_type: "image/webp".to_owned(),
            screen_width: None,
            screen_height: None,
            image_bytes: None,
        };

        agent.save_screenshot_ref(&mut ctx, &screenshot);

        // Verify file was saved to disk
        let expected_path = screenshots_dir.join("step_001.webp");
        assert!(expected_path.exists(), "screenshot file should exist on disk");
        assert_eq!(
            std::fs::read(&expected_path).unwrap(),
            fake_bytes,
            "file should contain decoded screenshot bytes"
        );

        // Verify context stores a path ref (not inline base64)
        assert_eq!(ctx.len(), 2); // system + screenshot
        let msgs = ctx.get_messages();
        let parts = msgs[1]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "image_path_ref");
        assert_eq!(parts[0]["path"], expected_path.to_string_lossy().as_ref());
    }

    #[test]
    fn save_screenshot_ref_increments_counter() {
        let tmp = tempfile::TempDir::new().unwrap();
        let screenshots_dir = tmp.path().join("screenshots");

        let mut agent = make_loop(vec![]);
        agent.set_screenshots_dir(screenshots_dir.clone());

        let mut ctx = ConversationContext::new("sys", 5, None);

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"data");
        let screenshot = Screenshot {
            base64: b64,
            width: 100,
            height: 100,
            scale_factor: 1.0,
            mime_type: "image/webp".to_owned(),
            screen_width: None,
            screen_height: None,
            image_bytes: None,
        };

        agent.save_screenshot_ref(&mut ctx, &screenshot);
        agent.save_screenshot_ref(&mut ctx, &screenshot);

        assert!(screenshots_dir.join("step_001.webp").exists());
        assert!(screenshots_dir.join("step_002.webp").exists());
        assert_eq!(ctx.len(), 3); // system + 2 screenshots
    }

    #[test]
    fn save_screenshot_ref_falls_back_to_inline_without_dir() {
        let mut agent = make_loop(vec![]);
        // No screenshots_dir set — should fall back to inline base64

        let mut ctx = ConversationContext::new("sys", 5, None);

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"data");
        let screenshot = Screenshot {
            base64: b64.clone(),
            width: 100,
            height: 100,
            scale_factor: 1.0,
            mime_type: "image/webp".to_owned(),
            screen_width: None,
            screen_height: None,
            image_bytes: None,
        };

        agent.save_screenshot_ref(&mut ctx, &screenshot);

        assert_eq!(ctx.len(), 2);
        let msgs = ctx.get_messages();
        let parts = msgs[1]["content"].as_array().unwrap();
        // Should be inline image_url, not path_ref
        assert_eq!(parts[0]["type"], "image_url");
        let url = parts[0]["image_url"]["url"].as_str().unwrap();
        assert!(url.contains(&b64));
    }
}
