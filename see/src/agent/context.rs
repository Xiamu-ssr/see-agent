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
    max_images: usize,
    on_append: Option<Box<dyn Fn(Value) + Send + Sync>>,
}

impl ConversationContext {
    /// Create a new context with the initial system prompt.
    pub fn new(
        system_prompt: &str,
        max_images: usize,
        on_append: Option<Box<dyn Fn(Value) + Send + Sync>>,
    ) -> Self {
        let sys_msg = json!({"role": "system", "content": system_prompt});

        if let Some(ref cb) = on_append {
            cb(json!({"type": "system", "content": system_prompt}));
        }

        Self {
            messages: vec![sys_msg],
            max_images,
            on_append,
        }
    }

    /// Create a context for session restore (no on_append, no initial system message).
    pub fn for_restore(max_images: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_images,
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
        if image_positions.len() <= self.max_images {
            return self.messages.clone();
        }

        // 3. Identify positions to drop (oldest first)
        let drop_count = image_positions.len() - self.max_images;
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

    // -----------------------------------------------------------------------
    // Compaction
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
        ConversationContext::new("You are a test agent.", 3, None)
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
        // 2 images <= max_images(3), no pruning
        assert_eq!(msgs.len(), 3);
    }

    #[test]
    fn get_messages_sliding_window() {
        let mut ctx = ConversationContext::new("sys", 2, None);
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
        let mut ctx = ConversationContext::new("sys", 1, None);
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
            5,
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
}
