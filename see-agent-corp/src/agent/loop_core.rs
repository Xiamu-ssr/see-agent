use std::path::PathBuf;
use std::sync::Arc;

use tracing::{info, warn};

use crate::consts::DEFAULT_LLM_MAX_TOKENS;
use crate::eye::Eye;
#[cfg(test)]
use crate::eye::Screenshot;
use crate::session::SessionStore;
use crate::tool::ToolRegistry;
use crate::types::{Config, SessionMessageType, ToolResult};

use super::context::{estimate_tokens, ConversationContext};

/// The main agent execution engine.
///
/// Runs in inbox/ReAct mode: hot-reload prompt → LLM → tool → back to idle.
pub struct AgentLoop {
    brain: Box<dyn crate::brain::Brain>,
    #[allow(dead_code)]
    eye: Arc<dyn Eye>,
    registry: ToolRegistry,
    config: Config,
    #[allow(dead_code)]
    agent_id: String,
    pub(crate) inject_queue: Vec<serde_json::Value>,
    compact_warned: bool,
    // Derived from config
    max_steps: u32,
    /// Directory to save screenshots to disk (path-ref mode).
    screenshots_dir: Option<PathBuf>,
    /// Counter for screenshot file naming.
    screenshot_counter: u32,
    /// Optional session store for persisting messages to disk.
    session_store: Option<SessionStore>,
    /// Screen dimensions for coordinate scaling, updated on each screenshot.
    screen_dims: (u32, u32, u32, u32), // (model_w, model_h, screen_w, screen_h)
    /// Inbox path for real-time steer injection (optional; set by worker).
    inbox_path: Option<PathBuf>,
    /// Cursor path for real-time steer injection (optional; set by worker).
    cursor_path: Option<PathBuf>,
}

impl AgentLoop {
    pub fn new(
        brain: Box<dyn crate::brain::Brain>,
        eye: Arc<dyn Eye>,
        registry: ToolRegistry,
        config: Config,
        agent_id: String,
    ) -> Self {
        let max_steps = config.agent.max_steps;

        Self {
            brain,
            eye,
            registry,
            config,
            agent_id,
            inject_queue: Vec::new(),
            compact_warned: false,
            max_steps,
            screenshots_dir: None,
            screenshot_counter: 0,
            session_store: None,
            screen_dims: (0, 0, 0, 0),
            inbox_path: None,
            cursor_path: None,
        }
    }

    /// Set the directory where screenshots are saved to disk.
    pub fn set_screenshots_dir(&mut self, dir: PathBuf) {
        self.screenshots_dir = Some(dir);
    }

    /// Set a session store for persisting messages to disk.
    pub fn set_session_store(&mut self, store: SessionStore) {
        self.session_store = Some(store);
    }

    /// Set inbox paths for real-time steer injection during reasoning loop.
    pub fn set_inbox_paths(&mut self, inbox: PathBuf, cursor: PathBuf) {
        self.inbox_path = Some(inbox);
        self.cursor_path = Some(cursor);
    }

    /// Hot-reload config and brain (called when config.json changes).
    pub fn hot_reload(&mut self, config: Config, brain: Box<dyn crate::brain::Brain>) {
        self.max_steps = config.agent.max_steps;
        self.config = config;
        self.brain = brain;
    }

    /// Save a screenshot to disk and add it to context as a path reference.
    /// Falls back to inline base64 if no screenshots_dir is set or save fails.
    #[cfg(test)]
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

    /// Save a tool result image to disk (without adding to conversation context).
    fn save_image_to_disk(&mut self, image: &crate::types::ToolResultImage) {
        if let Some(ref dir) = self.screenshots_dir {
            self.screenshot_counter += 1;
            let file_path = dir.join(format!("step_{:03}.webp", self.screenshot_counter));
            let _ = std::fs::create_dir_all(dir);
            if let Ok(bytes) = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &image.base64,
            ) {
                let _ = std::fs::write(&file_path, &bytes);
            }
        }
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
    // Inbox/ReAct (one turn)
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
            let label = match sender {
                "user" => "[用户]",
                "system" | "supervisor" => "[系统]",
                s => &format!("[{s}]"),
            };
            let formatted = format!("{label} {text}");
            ctx.add_user_reply(&formatted, sender, priority);

            // Persist user message to session store (without prefix — prefix is only for LLM context)
            if let Some(ref mut store) = self.session_store {
                let _ = store.append_message(
                    SessionMessageType::UserReply,
                    serde_json::json!({ "content": text, "sender": sender, "priority": priority }),
                );
            }
        }

        let disabled: Vec<String> = self.config.tools.disabled.clone();
        let tools_schema = self.schemas_to_values(&disabled);

        for _step in 0..self.max_steps {
            // Compact check
            self.maybe_compact(ctx).await;

            // Drain inject queue
            for msg in self.inject_queue.drain(..) {
                if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
                    let sender = msg
                        .get("sender")
                        .and_then(|v| v.as_str())
                        .unwrap_or("system");
                    let label = match sender {
                        "user" => "[用户]",
                        "system" | "supervisor" => "[系统]",
                        s => &format!("[{s}]"),
                    };
                    let formatted = format!("{label} {text}");
                    ctx.add_user_reply(&formatted, sender, "steer");
                }
            }

            // Real-time steer injection: drain new steer messages before each LLM call
            if let (Some(inbox), Some(cursor)) = (&self.inbox_path, &self.cursor_path)
                && let Ok(steer_msgs) = crate::supervisor::drain_steer_only(inbox, cursor)
            {
                for msg in &steer_msgs {
                    let sender = &msg.sender;
                    let label = match sender.as_str() {
                        "user" => "[用户]",
                        "system" | "supervisor" => "[系统]",
                        s => &format!("[{s}]"),
                    };
                    let formatted = format!("{label} {}", msg.content);
                    ctx.add_user_reply(&formatted, sender, "steer");

                    if let Some(ref mut store) = self.session_store {
                        let _ = store.append_message(
                            SessionMessageType::UserReply,
                            serde_json::json!({ "content": msg.content, "sender": sender, "priority": "steer" }),
                        );
                    }
                }
                if !steer_msgs.is_empty() {
                    info!(count = steer_msgs.len(), "injected real-time steer messages");
                }
            }

            let llm_messages = ctx.get_messages_for_llm();
            let sys_prompt = llm_messages.first()
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("");
            self.write_last_llm_call(sys_prompt, &tools_schema, &llm_messages);
            let response = match self.brain.chat(&llm_messages, &tools_schema).await {
                Ok(r) => r,
                Err(e) => {
                    warn!("LLM error in run_one_turn: {e}");
                    if let Some(ref mut store) = self.session_store {
                        let _ = store.append_message(
                            SessionMessageType::Error,
                            serde_json::json!({ "error": format!("{e}") }),
                        );
                    }
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
                    info!("finished tool called, returning to idle");
                    return;
                }

                let result = match self.registry.execute(&tc.name, tc.arguments.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("tool {} error: {e}", tc.name);
                        ToolResult::text(format!("Error: {e}"))
                    }
                };

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
                ctx.add_tool_result(&tc.id, &result.text, &ctx_images);

                // Screenshot tool: save image to disk + update screen dims
                if tc.name == "screenshot" && !result.images.is_empty() {
                    // Update screen dims from metadata
                    let m = &result.metadata;
                    if let (Some(w), Some(h), Some(sw), Some(sh)) = (
                        m["width"].as_u64(),
                        m["height"].as_u64(),
                        m["screen_width"].as_u64(),
                        m["screen_height"].as_u64(),
                    ) {
                        self.screen_dims = (w as u32, h as u32, sw as u32, sh as u32);
                    }
                    // Save to disk (without adding another message to context)
                    self.save_image_to_disk(&result.images[0]);
                }

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
        let micro_threshold = (window * self.config.agent.compact.microcompact_ratio) as usize;
        if tokens >= micro_threshold {
            let saved = ctx.apply_microcompact(keep_recent);
            if saved > 0 {
                info!("microcompact saved ~{saved} tokens");
            }
        }

        // Layer 3: Full compact — LLM summarization
        let full_threshold = (window * self.config.agent.compact.full_compact_ratio) as usize;
        let tokens_after = estimate_tokens(&ctx.get_messages());
        if tokens_after < full_threshold {
            return;
        }

        if !self.compact_warned {
            self.compact_warned = true;
            ctx.add_system_hint(
                "[系统提示] 上下文即将达到窗口上限，请立即用 write 工具将重要信息保存到 memory/ 目录，下一轮将执行上下文压缩。",
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

    /// Write last LLM call metadata to session/last_llm_call.json (atomic overwrite).
    fn write_last_llm_call(
        &self,
        system_prompt: &str,
        tools: &[serde_json::Value],
        messages: &[serde_json::Value],
    ) {
        let Some(ref store) = self.session_store else {
            return;
        };
        let path = store.dir().last_llm_call();
        let data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "model": self.config.llm.model,
            "system_prompt": system_prompt,
            "tools": tools,
            "max_tokens": DEFAULT_LLM_MAX_TOKENS,
            "message_count": messages.len(),
            "estimated_tokens": estimate_tokens(messages),
        });
        // Atomic write: write to tmp file then rename
        let tmp_path = path.with_extension("json.tmp");
        if let Ok(content) = serde_json::to_string_pretty(&data)
            && std::fs::write(&tmp_path, content).is_ok()
        {
            let _ = std::fs::rename(&tmp_path, &path);
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
    impl crate::brain::Brain for MockBrain {
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
    async fn run_one_turn_adds_messages() {
        let brain_resp = BrainResponse {
            content: Some("Got it.".to_owned()),
            tool_calls: vec![],
            raw: json!({"role": "assistant", "content": "Got it."}),
        };
        let mut agent = make_loop(vec![brain_resp]);
        let mut ctx = ConversationContext::new("sys", 3, 3, None);

        let msgs = vec![json!({"content": "hello", "sender": "user", "priority": "collect"})];
        agent.run_one_turn(&mut ctx, &msgs, "sys").await;

        // system + user_reply + assistant = 3
        assert_eq!(ctx.len(), 3);
    }

    #[tokio::test]
    async fn run_one_turn_returns_on_finished_tool() {
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
        let mut ctx = ConversationContext::new("sys", 3, 3, None);

        let msgs = vec![json!({"content": "do it", "sender": "user", "priority": "collect"})];
        agent.run_one_turn(&mut ctx, &msgs, "sys").await;

        // Should have returned after finished tool
        assert!(ctx.len() >= 3); // system + user + assistant
    }

    #[test]
    fn save_screenshot_ref_writes_file_and_stores_path_ref() {
        let tmp = tempfile::TempDir::new().unwrap();
        let screenshots_dir = tmp.path().join("screenshots");

        let mut agent = make_loop(vec![]);
        agent.set_screenshots_dir(screenshots_dir.clone());

        let mut ctx = ConversationContext::new("sys", 3, 3, None);

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

        let expected_path = screenshots_dir.join("step_001.webp");
        assert!(expected_path.exists(), "screenshot file should exist on disk");
        assert_eq!(
            std::fs::read(&expected_path).unwrap(),
            fake_bytes,
            "file should contain decoded screenshot bytes"
        );

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

        let mut ctx = ConversationContext::new("sys", 3, 3, None);

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

        let mut ctx = ConversationContext::new("sys", 3, 3, None);

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
        assert_eq!(parts[0]["type"], "image_url");
        let url = parts[0]["image_url"]["url"].as_str().unwrap();
        assert!(url.contains(&b64));
    }
}
