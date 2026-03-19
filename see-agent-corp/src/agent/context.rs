use std::path::PathBuf;

use serde_json::{json, Value};

use crate::consts::{CHARS_PER_TOKEN, TOKENS_PER_IMAGE};

// ---------------------------------------------------------------------------
// Tool result image (from tool execution)
// ---------------------------------------------------------------------------

/// An image returned by a tool alongside its text result.
#[derive(Debug, Clone)]
pub struct ToolResultImage {
    pub base64: String,
    pub mime_type: String,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// ImageContent
// ---------------------------------------------------------------------------

/// How an image is stored in the conversation context.
#[derive(Debug, Clone)]
pub enum ImageContent {
    /// Path to screenshot file on disk — resolved to base64 only at LLM call time.
    PathRef {
        path: PathBuf,
        detail: String,
        mime_type: String,
    },
    /// Inline base64 data — used only at the LLM call boundary.
    Inline {
        base64: String,
        detail: String,
        mime_type: String,
    },
}

// ---------------------------------------------------------------------------
// ImageAction (for four-level lifecycle in get_messages_for_llm)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageAction {
    KeepHigh,
    Downgrade,
    TextOnly,
}

// ---------------------------------------------------------------------------
// ConversationContext
// ---------------------------------------------------------------------------

/// Manages the in-memory message history for an agent's conversation.
///
/// Messages are stored in OpenAI chat-completion format (`Vec<Value>`).
/// A sliding window limits how many screenshots are included when
/// `get_messages()` is called — older images are replaced with
/// `[Screenshot omitted]`.
pub struct ConversationContext {
    messages: Vec<Value>,
    image_high_count: usize,
    image_low_count: usize,
    on_append: Option<Box<dyn Fn(Value) + Send + Sync>>,
}

impl ConversationContext {
    /// Create a new context with the initial system prompt.
    pub fn new(
        system_prompt: &str,
        image_high_count: usize,
        image_low_count: usize,
        on_append: Option<Box<dyn Fn(Value) + Send + Sync>>,
    ) -> Self {
        let sys_msg = json!({"role": "system", "content": system_prompt});

        if let Some(ref cb) = on_append {
            cb(json!({"type": "system", "content": system_prompt}));
        }

        Self {
            messages: vec![sys_msg],
            image_high_count,
            image_low_count,
            on_append,
        }
    }

    /// Create a context for session restore (no on_append, no initial system message).
    pub fn for_restore(image_high_count: usize, image_low_count: usize) -> Self {
        Self {
            messages: Vec::new(),
            image_high_count,
            image_low_count,
            on_append: None,
        }
    }

    /// Activate the on_append callback (after restore replay is complete).
    pub fn set_on_append(&mut self, cb: Box<dyn Fn(Value) + Send + Sync>) {
        self.on_append = Some(cb);
    }

    /// Direct access to internal messages (for restore replay).
    pub fn push_raw(&mut self, msg: Value) {
        self.messages.push(msg);
    }

    /// Number of messages currently stored.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    // -----------------------------------------------------------------------
    // Mutators
    // -----------------------------------------------------------------------

    /// Add a user task with a screenshot.
    pub fn add_user_task(
        &mut self,
        text: &str,
        screenshot_b64: &str,
        detail: &str,
        mime_type: &str,
    ) {
        let msg = json!({
            "role": "user",
            "content": [
                {"type": "text", "text": text},
                {"type": "image_url", "image_url": {
                    "url": format!("data:{mime_type};base64,{screenshot_b64}"),
                    "detail": detail,
                }}
            ]
        });
        self.messages.push(msg);
        self.fire_append(json!({
            "type": "user_task",
            "text": text,
            "detail": detail,
        }));
    }

    /// Add a text-only user task (no screenshot).
    pub fn add_user_task_text_only(&mut self, text: &str, sender: &str, priority: &str) {
        let msg = json!({"role": "user", "content": text});
        self.messages.push(msg);
        self.fire_append(json!({
            "type": "user_task",
            "text": text,
            "sender": sender,
            "priority": priority,
        }));
    }

    /// Add an assistant response (raw API message).
    pub fn add_assistant(&mut self, raw: &Value) {
        self.messages.push(raw.clone());

        // Persist flattened form
        let content = raw.get("content").cloned();
        let tool_calls: Vec<Value> = raw
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|tc| {
                        json!({
                            "id": tc.get("id"),
                            "name": tc.pointer("/function/name"),
                            "args": tc.pointer("/function/arguments"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut persist = json!({"type": "assistant"});
        if let Some(c) = content {
            persist["content"] = c;
        }
        if !tool_calls.is_empty() {
            persist["tool_calls"] = json!(tool_calls);
        }
        self.fire_append(persist);
    }

    /// Add a tool execution result.
    ///
    /// If `images` is non-empty, each image is also added as a screenshot message.
    pub fn add_tool_result(
        &mut self,
        tool_call_id: &str,
        text: &str,
        images: &[ToolResultImage],
    ) {
        let msg = json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": text,
        });
        self.messages.push(msg);
        self.fire_append(json!({
            "type": "tool_result",
            "tool_call_id": tool_call_id,
            "result": text,
        }));

        for img in images {
            self.add_screenshot(&img.base64, &img.detail, &img.mime_type);
        }
    }

    /// Add a standalone screenshot (e.g. from tool result or periodic capture).
    pub fn add_screenshot(&mut self, b64: &str, detail: &str, mime_type: &str) {
        let msg = json!({
            "role": "user",
            "content": [{
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{mime_type};base64,{b64}"),
                    "detail": detail,
                }
            }]
        });
        self.messages.push(msg);
        self.fire_append(json!({
            "type": "screenshot",
            "detail": detail,
        }));
    }

    /// Store a path reference to a screenshot on disk.
    ///
    /// The image is NOT loaded into memory here — it will be resolved to base64
    /// only when `get_messages_for_llm()` is called.
    pub fn add_screenshot_ref(&mut self, path: PathBuf, detail: &str, mime_type: &str) {
        let msg = json!({
            "role": "user",
            "content": [{
                "type": "image_path_ref",
                "path": path.to_string_lossy(),
                "detail": detail,
                "mime_type": mime_type,
            }]
        });
        self.messages.push(msg);
        self.fire_append(json!({
            "type": "screenshot",
            "detail": detail,
        }));
    }

    /// Add a user reply (from call_user or inbox).
    pub fn add_user_reply(&mut self, text: &str, sender: &str, priority: &str) {
        let msg = json!({"role": "user", "content": text});
        self.messages.push(msg);
        self.fire_append(json!({
            "type": "user_reply",
            "text": text,
            "sender": sender,
            "priority": priority,
        }));
    }

    /// Add a system hint (injected as user role to avoid conflicting with system[0]).
    pub fn add_system_hint(&mut self, text: &str) {
        let msg = json!({"role": "user", "content": text});
        self.messages.push(msg);
        self.fire_append(json!({
            "type": "system_hint",
            "text": text,
        }));
    }

    /// Update the system prompt in-place (hot-reload). Does NOT fire on_append.
    pub fn update_system_prompt(&mut self, new_prompt: &str) {
        if let Some(msg) = self.messages.first_mut()
            && msg.get("role").and_then(|v| v.as_str()) == Some("system")
        {
            msg["content"] = json!(new_prompt);
        }
    }

    // -----------------------------------------------------------------------
    // Retrieval with sliding window
    // -----------------------------------------------------------------------

    /// Get messages for LLM call, with screenshot sliding window applied.
    ///
    /// Returns a new Vec — never mutates internal state.
    pub fn get_messages(&self) -> Vec<Value> {
        // 1. Find all image positions
        let mut image_positions: Vec<(usize, Option<usize>)> = Vec::new();
        for (msg_idx, msg) in self.messages.iter().enumerate() {
            if let Some(parts) = msg.get("content").and_then(|c| c.as_array()) {
                for (part_idx, part) in parts.iter().enumerate() {
                    if part.get("type").and_then(|t| t.as_str()) == Some("image_url") {
                        image_positions.push((msg_idx, Some(part_idx)));
                    }
                }
            }
        }

        // 2. If within limit, return as-is
        let max_images = self.image_high_count + self.image_low_count;
        if image_positions.len() <= max_images {
            return self.messages.clone();
        }

        // 3. Identify positions to drop (oldest first)
        let drop_count = image_positions.len() - max_images;
        let to_drop: std::collections::HashSet<(usize, Option<usize>)> =
            image_positions[..drop_count].iter().copied().collect();

        // Group by message index
        let mut msgs_with_drops: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for &(msg_idx, part_idx) in &to_drop {
            if let Some(pi) = part_idx {
                msgs_with_drops.entry(msg_idx).or_default().push(pi);
            }
        }

        // 4. Rebuild output
        let mut output = Vec::with_capacity(self.messages.len());
        for (idx, msg) in self.messages.iter().enumerate() {
            if let Some(drop_parts) = msgs_with_drops.get(&idx) {
                let drop_set: std::collections::HashSet<usize> =
                    drop_parts.iter().copied().collect();

                if let Some(parts) = msg.get("content").and_then(|c| c.as_array()) {
                    let remaining: Vec<&Value> = parts
                        .iter()
                        .enumerate()
                        .filter(|(pi, _)| !drop_set.contains(pi))
                        .map(|(_, p)| p)
                        .collect();

                    let role = msg.get("role").cloned().unwrap_or(json!("user"));
                    let placeholder = json!({"type": "text", "text": "[Screenshot omitted]"});

                    if remaining.is_empty() {
                        output.push(json!({
                            "role": role,
                            "content": [placeholder],
                        }));
                    } else {
                        let mut new_parts = vec![placeholder];
                        new_parts.extend(remaining.into_iter().cloned());
                        output.push(json!({
                            "role": role,
                            "content": new_parts,
                        }));
                    }
                } else {
                    output.push(msg.clone());
                }
            } else {
                output.push(msg.clone());
            }
        }

        output
    }

    /// Get messages for LLM call, resolving path refs to base64 and applying
    /// the four-level image lifecycle.
    ///
    /// Level 1: Latest image_high_count images — full fidelity (detail: high)
    /// Level 2: Next image_low_count images — low fidelity (detail: low)
    /// Level 3: Older images — replaced with text placeholder
    /// Level 4: Images discarded at full compact (handled by apply_compaction)
    pub fn get_messages_for_llm(&self) -> Vec<Value> {
        // 1. Resolve all image_path_ref entries to image_url
        let resolved: Vec<Value> = self
            .messages
            .iter()
            .map(Self::resolve_path_refs)
            .collect();

        // 2. Find all image positions
        let mut image_positions: Vec<(usize, usize)> = Vec::new();
        for (msg_idx, msg) in resolved.iter().enumerate() {
            if let Some(parts) = msg.get("content").and_then(|c| c.as_array()) {
                for (part_idx, part) in parts.iter().enumerate() {
                    if part.get("type").and_then(|t| t.as_str()) == Some("image_url") {
                        image_positions.push((msg_idx, part_idx));
                    }
                }
            }
        }

        if image_positions.is_empty() {
            return resolved;
        }

        // 3. Classify images by level (newest-first)
        let total = image_positions.len();
        let level1_start = total.saturating_sub(self.image_high_count);
        let level2_start = level1_start.saturating_sub(self.image_low_count);

        // Build action map: (msg_idx, part_idx) -> action
        let mut actions: std::collections::HashMap<(usize, usize), ImageAction> =
            std::collections::HashMap::new();
        for (i, &pos) in image_positions.iter().enumerate() {
            let action = if i >= level1_start {
                ImageAction::KeepHigh
            } else if i >= level2_start {
                ImageAction::Downgrade
            } else {
                ImageAction::TextOnly
            };
            actions.insert(pos, action);
        }

        // 4. Rebuild output with lifecycle applied
        let mut output = Vec::with_capacity(resolved.len());
        for (msg_idx, msg) in resolved.iter().enumerate() {
            if let Some(parts) = msg.get("content").and_then(|c| c.as_array()) {
                let has_actions = parts.iter().enumerate().any(|(pi, _)| {
                    actions.contains_key(&(msg_idx, pi))
                });
                if has_actions {
                    let new_parts: Vec<Value> = parts
                        .iter()
                        .enumerate()
                        .map(|(pi, part)| {
                            match actions.get(&(msg_idx, pi)) {
                                Some(ImageAction::KeepHigh) => part.clone(),
                                Some(ImageAction::Downgrade) => {
                                    let mut p = part.clone();
                                    if let Some(img) = p.get_mut("image_url") {
                                        img["detail"] = json!("low");
                                    }
                                    p
                                }
                                Some(ImageAction::TextOnly) => {
                                    json!({"type": "text", "text": "[Screenshot omitted]"})
                                }
                                None => part.clone(),
                            }
                        })
                        .collect();
                    let role = msg.get("role").cloned().unwrap_or(json!("user"));
                    output.push(json!({"role": role, "content": new_parts}));
                } else {
                    output.push(msg.clone());
                }
            } else {
                output.push(msg.clone());
            }
        }

        output
    }

    /// Resolve image_path_ref entries in a single message to image_url.
    fn resolve_path_refs(msg: &Value) -> Value {
        if let Some(parts) = msg.get("content").and_then(|c| c.as_array()) {
            let new_parts: Vec<Value> = parts
                .iter()
                .map(|part| {
                    if part.get("type").and_then(|t| t.as_str()) == Some("image_path_ref") {
                        let path_str =
                            part.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        let detail =
                            part.get("detail").and_then(|v| v.as_str()).unwrap_or("auto");
                        let mime = part
                            .get("mime_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("image/png");

                        match std::fs::read(path_str) {
                            Ok(bytes) => {
                                use base64::Engine;
                                let b64 =
                                    base64::engine::general_purpose::STANDARD.encode(&bytes);
                                json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": format!("data:{mime};base64,{b64}"),
                                        "detail": detail,
                                    }
                                })
                            }
                            Err(_) => {
                                json!({"type": "text", "text": "[Screenshot file not found]"})
                            }
                        }
                    } else {
                        part.clone()
                    }
                })
                .collect();
            let role = msg.get("role").cloned().unwrap_or(json!("user"));
            json!({"role": role, "content": new_parts})
        } else {
            msg.clone()
        }
    }

    // -----------------------------------------------------------------------
    // Microcompact (Layer 2)
    // -----------------------------------------------------------------------

    /// Clear old tool_result content to save tokens (memory-only, no JSONL change).
    ///
    /// Keeps the most recent `keep_recent` messages untouched.
    /// Returns the estimated tokens saved.
    pub fn apply_microcompact(&mut self, keep_recent: usize) -> usize {
        let safe_boundary = self.messages.len().saturating_sub(keep_recent);
        let mut saved = 0usize;

        for msg in self.messages[..safe_boundary].iter_mut() {
            if msg.get("role").and_then(|r| r.as_str()) == Some("tool")
                && let Some(content) = msg.get("content").and_then(|c| c.as_str())
            {
                let old_tokens = content.len() / CHARS_PER_TOKEN;
                let placeholder = "[tool output cleared — microcompact]";
                let new_tokens = placeholder.len() / CHARS_PER_TOKEN;
                if old_tokens > new_tokens {
                    saved += old_tokens - new_tokens;
                    msg["content"] = serde_json::json!(placeholder);
                }
            }
        }

        if saved > 0 {
            self.fire_append(serde_json::json!({
                "type": "microcompact",
                "tokens_saved": saved,
            }));
        }

        saved
    }

    // -----------------------------------------------------------------------
    // Compaction (Layer 3)
    // -----------------------------------------------------------------------

    /// Apply compaction: keep system prompt + summary + last N messages.
    pub fn apply_compaction(&mut self, summary: &str, keep_recent: usize) {
        let system = self.messages.first().cloned().unwrap_or(json!({}));
        let summary_msg = json!({
            "role": "system",
            "content": format!("[Conversation Summary]\n{summary}"),
        });

        let recent_start = self.messages.len().saturating_sub(keep_recent);
        let recent: Vec<Value> = self.messages[recent_start..].to_vec();

        self.messages = vec![system, summary_msg];
        self.messages.extend(recent);

        self.fire_append(json!({
            "type": "compact",
            "summary": summary,
        }));
    }

    /// Inject a summary at index 1 (used during session restore).
    pub fn inject_summary(&mut self, summary: &str) {
        let summary_msg = json!({
            "role": "system",
            "content": format!("[Conversation Summary]\n{summary}"),
        });
        if self.messages.len() > 1 {
            self.messages.insert(1, summary_msg);
        } else {
            self.messages.push(summary_msg);
        }
    }

    // -----------------------------------------------------------------------
    // Private
    // -----------------------------------------------------------------------

    fn fire_append(&self, data: Value) {
        if let Some(ref cb) = self.on_append {
            cb(data);
        }
    }
}

// ---------------------------------------------------------------------------
// Token estimation (standalone function)
// ---------------------------------------------------------------------------

/// Estimate token count for a list of OpenAI-format messages.
///
/// Rules: text = len/4, image = 765, tool_call arguments = len/4.
pub fn estimate_tokens(messages: &[Value]) -> usize {
    let mut total = 0usize;
    for msg in messages {
        let content = msg.get("content");
        match content {
            Some(Value::String(s)) => {
                total += s.len() / CHARS_PER_TOKEN;
            }
            Some(Value::Array(parts)) => {
                for part in parts {
                    match part.get("type").and_then(|t| t.as_str()) {
                        Some("text") => {
                            if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                                total += t.len() / CHARS_PER_TOKEN;
                            }
                        }
                        Some("image_url") => {
                            total += TOKENS_PER_IMAGE;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        // Tool call arguments
        if let Some(tcs) = msg.get("tool_calls").and_then(|v| v.as_array()) {
            for tc in tcs {
                if let Some(args) = tc.pointer("/function/arguments").and_then(|v| v.as_str()) {
                    total += args.len() / CHARS_PER_TOKEN;
                }
            }
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> ConversationContext {
        ConversationContext::new("You are a test agent.", 3, 3, None)
    }

    #[test]
    fn new_context_has_system_message() {
        let ctx = make_ctx();
        assert_eq!(ctx.len(), 1);
        assert_eq!(ctx.messages[0]["role"], "system");
        assert_eq!(ctx.messages[0]["content"], "You are a test agent.");
    }

    #[test]
    fn add_user_task_text_only() {
        let mut ctx = make_ctx();
        ctx.add_user_task_text_only("Do something", "user", "collect");
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx.messages[1]["role"], "user");
        assert_eq!(ctx.messages[1]["content"], "Do something");
    }

    #[test]
    fn add_assistant_and_tool_result() {
        let mut ctx = make_ctx();
        let raw = json!({
            "role": "assistant",
            "content": "I'll click there.",
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "click", "arguments": "{\"x\":100}"}
            }]
        });
        ctx.add_assistant(&raw);
        ctx.add_tool_result("call_1", "clicked at (100, 200)", &[]);

        assert_eq!(ctx.len(), 3);
        assert_eq!(ctx.messages[1]["role"], "assistant");
        assert_eq!(ctx.messages[2]["role"], "tool");
        assert_eq!(ctx.messages[2]["tool_call_id"], "call_1");
    }

    #[test]
    fn add_system_hint_uses_user_role() {
        let mut ctx = make_ctx();
        ctx.add_system_hint("Screen hasn't changed.");
        assert_eq!(ctx.messages[1]["role"], "user");
    }

    #[test]
    fn update_system_prompt() {
        let mut ctx = make_ctx();
        ctx.update_system_prompt("New prompt.");
        assert_eq!(ctx.messages[0]["content"], "New prompt.");
    }

    #[test]
    fn get_messages_no_pruning() {
        let mut ctx = make_ctx();
        ctx.add_screenshot("abc", "high", "image/webp");
        ctx.add_screenshot("def", "high", "image/webp");

        let msgs = ctx.get_messages();
        // 2 images <= image_high+low(6), no pruning
        assert_eq!(msgs.len(), 3);
    }

    #[test]
    fn get_messages_sliding_window() {
        let mut ctx = ConversationContext::new("sys", 1, 1, None);
        ctx.add_screenshot("img1", "high", "image/webp");
        ctx.add_screenshot("img2", "high", "image/webp");
        ctx.add_screenshot("img3", "high", "image/webp");
        ctx.add_screenshot("img4", "high", "image/webp");

        let msgs = ctx.get_messages();
        assert_eq!(msgs.len(), 5); // system + 4 screenshot messages

        // First 2 should be replaced with [Screenshot omitted]
        let first_content = msgs[1]["content"].as_array().unwrap();
        assert_eq!(first_content[0]["text"], "[Screenshot omitted]");

        let second_content = msgs[2]["content"].as_array().unwrap();
        assert_eq!(second_content[0]["text"], "[Screenshot omitted]");

        // Last 2 should still have image_url
        let third_content = msgs[3]["content"].as_array().unwrap();
        assert_eq!(third_content[0]["type"], "image_url");

        let fourth_content = msgs[4]["content"].as_array().unwrap();
        assert_eq!(fourth_content[0]["type"], "image_url");
    }

    #[test]
    fn get_messages_preserves_mixed_content() {
        let mut ctx = ConversationContext::new("sys", 1, 0, None);
        // User task with text + image
        ctx.add_user_task("Do this", "img_data", "high", "image/webp");
        // Another screenshot
        ctx.add_screenshot("img2", "high", "image/webp");

        let msgs = ctx.get_messages();
        // The first user_task's image should be dropped, but text preserved
        let task_msg = &msgs[1];
        let parts = task_msg["content"].as_array().unwrap();
        // Should have [Screenshot omitted] placeholder + original text
        assert!(parts.iter().any(|p| p["text"] == "[Screenshot omitted]"));
        assert!(parts.iter().any(|p| p["text"] == "Do this"));
    }

    #[test]
    fn apply_compaction() {
        let mut ctx = make_ctx();
        for i in 0..10 {
            ctx.add_user_task_text_only(&format!("msg {i}"), "user", "collect");
        }
        assert_eq!(ctx.len(), 11); // system + 10

        ctx.apply_compaction("Summary of first 6 messages.", 4);

        // system + summary + last 4
        assert_eq!(ctx.len(), 6);
        assert_eq!(ctx.messages[0]["role"], "system");
        assert!(ctx.messages[1]["content"]
            .as_str()
            .unwrap()
            .starts_with("[Conversation Summary]"));
        assert_eq!(ctx.messages[2]["content"], "msg 6");
    }

    #[test]
    fn inject_summary() {
        let mut ctx = make_ctx();
        ctx.add_user_task_text_only("task", "user", "collect");
        ctx.inject_summary("Previous context summary.");

        assert_eq!(ctx.len(), 3);
        assert!(ctx.messages[1]["content"]
            .as_str()
            .unwrap()
            .contains("Previous context summary."));
    }

    #[test]
    fn estimate_tokens_text() {
        let msgs = vec![json!({"role": "user", "content": "x".repeat(100)})];
        assert_eq!(estimate_tokens(&msgs), 25); // 100/4
    }

    #[test]
    fn estimate_tokens_image() {
        let msgs = vec![json!({
            "role": "user",
            "content": [{"type": "image_url", "image_url": {"url": "data:..."}}]
        })];
        assert_eq!(estimate_tokens(&msgs), TOKENS_PER_IMAGE);
    }

    #[test]
    fn estimate_tokens_tool_calls() {
        let msgs = vec![json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "click", "arguments": "x".repeat(40)}
            }]
        })];
        assert_eq!(estimate_tokens(&msgs), 10); // 40/4
    }

    #[test]
    fn on_append_fires() {
        use std::sync::{Arc, Mutex};
        let log = Arc::new(Mutex::new(Vec::<Value>::new()));
        let log2 = log.clone();

        let mut ctx = ConversationContext::new(
            "sys",
            3,
            3,
            Some(Box::new(move |v| {
                log2.lock().unwrap().push(v);
            })),
        );
        ctx.add_user_task_text_only("hello", "user", "collect");
        ctx.add_system_hint("hint");

        let entries = log.lock().unwrap();
        assert_eq!(entries.len(), 3); // system + user_task + system_hint
        assert_eq!(entries[0]["type"], "system");
        assert_eq!(entries[1]["type"], "user_task");
        assert_eq!(entries[2]["type"], "system_hint");
    }

    #[test]
    fn tool_result_with_images() {
        let mut ctx = make_ctx();
        let raw = json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "screenshot", "arguments": "{}"}
            }]
        });
        ctx.add_assistant(&raw);

        let images = vec![ToolResultImage {
            base64: "img_data".to_owned(),
            mime_type: "image/webp".to_owned(),
            detail: "high".to_owned(),
        }];
        ctx.add_tool_result("call_1", "screenshot taken", &images);

        // assistant + tool_result + screenshot = 3 new messages
        assert_eq!(ctx.len(), 4);
        // Last message should be the image
        let last = &ctx.messages[3];
        assert_eq!(last["role"], "user");
        let parts = last["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "image_url");
    }

    // -----------------------------------------------------------------------
    // ImageContent / path-ref tests
    // -----------------------------------------------------------------------

    #[test]
    fn add_screenshot_ref_stores_path_placeholder() {
        let mut ctx = make_ctx();
        ctx.add_screenshot_ref(
            PathBuf::from("/tmp/test.png"),
            "low",
            "image/png",
        );

        assert_eq!(ctx.len(), 2);
        let msg = &ctx.messages[1];
        assert_eq!(msg["role"], "user");
        let parts = msg["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "image_path_ref");
        assert_eq!(parts[0]["path"], "/tmp/test.png");
        assert_eq!(parts[0]["detail"], "low");
        assert_eq!(parts[0]["mime_type"], "image/png");
    }

    #[test]
    fn get_messages_returns_path_refs_unresolved() {
        let mut ctx = make_ctx();
        ctx.add_screenshot_ref(
            PathBuf::from("/tmp/test.png"),
            "low",
            "image/png",
        );

        let msgs = ctx.get_messages();
        // get_messages() should return raw messages — path refs stay as-is
        let parts = msgs[1]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "image_path_ref");
    }

    #[test]
    fn get_messages_for_llm_resolves_path_ref_to_base64() {
        let tmp = tempfile::TempDir::new().unwrap();
        let img_path = tmp.path().join("test.png");
        // Write some fake image bytes
        std::fs::write(&img_path, b"fake-png-data").unwrap();

        let mut ctx = make_ctx();
        ctx.add_screenshot_ref(img_path, "low", "image/png");

        let msgs = ctx.get_messages_for_llm();
        let parts = msgs[1]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "image_url");
        let url = parts[0]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        // Verify the base64 decodes to our fake data
        let b64_part = url.strip_prefix("data:image/png;base64,").unwrap();
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64_part)
            .unwrap();
        assert_eq!(decoded, b"fake-png-data");
    }

    #[test]
    fn get_messages_for_llm_missing_file_becomes_placeholder() {
        let mut ctx = make_ctx();
        ctx.add_screenshot_ref(
            PathBuf::from("/nonexistent/path.png"),
            "low",
            "image/png",
        );

        let msgs = ctx.get_messages_for_llm();
        let parts = msgs[1]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "[Screenshot file not found]");
    }

    #[test]
    fn get_messages_for_llm_four_level_on_path_refs() {
        let tmp = tempfile::TempDir::new().unwrap();

        // 4 images: with image_high=3, image_low=3 → img0=Downgrade, img1-3=KeepHigh
        let mut ctx = ConversationContext::new("sys", 3, 3, None);
        for i in 0..4 {
            let img_path = tmp.path().join(format!("img{i}.png"));
            std::fs::write(&img_path, format!("data-{i}").as_bytes()).unwrap();
            ctx.add_screenshot_ref(img_path, "high", "image/png");
        }

        let msgs = ctx.get_messages_for_llm();
        assert_eq!(msgs.len(), 5); // system + 4 screenshot messages

        // img0 → Downgrade (detail: "low")
        let first_content = msgs[1]["content"].as_array().unwrap();
        assert_eq!(first_content[0]["type"], "image_url");
        assert_eq!(first_content[0]["image_url"]["detail"], "low");

        // img1-3 → KeepHigh (detail remains "high")
        for msg in &msgs[2..=4] {
            let parts = msg["content"].as_array().unwrap();
            assert_eq!(parts[0]["type"], "image_url");
            assert_eq!(parts[0]["image_url"]["detail"], "high");
        }
    }

    // -----------------------------------------------------------------------
    // Microcompact tests
    // -----------------------------------------------------------------------

    #[test]
    fn microcompact_clears_old_tool_results() {
        let mut ctx = make_ctx();
        // Add assistant + tool result with large content
        let raw = json!({
            "role": "assistant",
            "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "shell", "arguments": "{}"}}]
        });
        ctx.add_assistant(&raw);
        ctx.add_tool_result("c1", &"x".repeat(1000), &[]);
        // Add a recent message
        ctx.add_user_task_text_only("recent", "user", "collect");

        let saved = ctx.apply_microcompact(1); // keep last 1 message
        assert!(saved > 0, "should save tokens");
        // Tool result content should be replaced
        let tool_msg = &ctx.messages[2];
        assert_eq!(tool_msg["content"], "[tool output cleared — microcompact]");
    }

    #[test]
    fn microcompact_preserves_recent_messages() {
        let mut ctx = make_ctx();
        let raw = json!({
            "role": "assistant",
            "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "shell", "arguments": "{}"}}]
        });
        ctx.add_assistant(&raw);
        ctx.add_tool_result("c1", &"x".repeat(1000), &[]);

        // keep_recent = 10 — everything is "recent"
        let saved = ctx.apply_microcompact(10);
        assert_eq!(saved, 0, "all messages within keep_recent, nothing cleared");
        let tool_msg = &ctx.messages[2];
        assert_ne!(tool_msg["content"], "[tool output cleared — microcompact]");
    }

    #[test]
    fn microcompact_skips_small_tool_results() {
        let mut ctx = make_ctx();
        let raw = json!({
            "role": "assistant",
            "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "shell", "arguments": "{}"}}]
        });
        ctx.add_assistant(&raw);
        ctx.add_tool_result("c1", "ok", &[]); // tiny content
        ctx.add_user_task_text_only("recent", "user", "collect");

        let saved = ctx.apply_microcompact(1);
        // "ok" (2 chars → 0 tokens) is not bigger than placeholder, so no savings
        assert_eq!(saved, 0);
    }

    // -----------------------------------------------------------------------
    // Four-level image lifecycle tests
    // -----------------------------------------------------------------------

    #[test]
    fn four_level_lifecycle_keep_high_downgrade_text_only() {
        let tmp = tempfile::TempDir::new().unwrap();

        // Create 8 images: 2 → TextOnly, 3 → Downgrade, 3 → KeepHigh
        let mut ctx = ConversationContext::new("sys", 3, 3, None);
        for i in 0..8 {
            let img_path = tmp.path().join(format!("img{i}.png"));
            std::fs::write(&img_path, format!("data-{i}").as_bytes()).unwrap();
            ctx.add_screenshot_ref(img_path, "high", "image/png");
        }

        let msgs = ctx.get_messages_for_llm();
        assert_eq!(msgs.len(), 9); // system + 8

        // Images 0,1 → TextOnly (indices 1,2 in msgs)
        for (i, msg) in msgs[1..=2].iter().enumerate() {
            let parts = msg["content"].as_array().unwrap();
            assert_eq!(
                parts[0]["text"], "[Screenshot omitted]",
                "image {i} should be TextOnly"
            );
        }

        // Images 2,3,4 → Downgrade (indices 3,4,5 in msgs)
        for (i, msg) in msgs[3..=5].iter().enumerate() {
            let parts = msg["content"].as_array().unwrap();
            assert_eq!(parts[0]["type"], "image_url", "image {i}+2 should be image_url");
            assert_eq!(
                parts[0]["image_url"]["detail"], "low",
                "image {i}+2 should be downgraded to low detail"
            );
        }

        // Images 5,6,7 → KeepHigh (indices 6,7,8 in msgs)
        for (i, msg) in msgs[6..=8].iter().enumerate() {
            let parts = msg["content"].as_array().unwrap();
            assert_eq!(parts[0]["type"], "image_url", "image {i}+5 should be image_url");
            assert_eq!(
                parts[0]["image_url"]["detail"], "high",
                "image {i}+5 should keep high detail"
            );
        }
    }

    #[test]
    fn four_level_lifecycle_few_images_all_keep_high() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Only 2 images — both within image_high_count, all KeepHigh
        let mut ctx = ConversationContext::new("sys", 3, 3, None);
        for i in 0..2 {
            let img_path = tmp.path().join(format!("img{i}.png"));
            std::fs::write(&img_path, format!("data-{i}").as_bytes()).unwrap();
            ctx.add_screenshot_ref(img_path, "high", "image/png");
        }

        let msgs = ctx.get_messages_for_llm();
        for msg in &msgs[1..=2] {
            let parts = msg["content"].as_array().unwrap();
            assert_eq!(parts[0]["type"], "image_url");
            assert_eq!(parts[0]["image_url"]["detail"], "high");
        }
    }
}
