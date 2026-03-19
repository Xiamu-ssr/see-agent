use serde_json::json;
use tracing::info;

use crate::types::Message;

use super::context::ConversationContext;
use super::loop_core::AgentLoop;

// ---------------------------------------------------------------------------
// AgentRuntime — message dispatch layer on top of AgentLoop
// ---------------------------------------------------------------------------

/// Wraps `AgentLoop` with inbox message dispatch (collect vs steer).
///
/// - **Collect** messages are batched in `pending` and delivered at the start
///   of the next `run_turn()`.
/// - **Steer** messages are pushed into the `AgentLoop.inject_queue` and
///   delivered between tool calls in the current step.
pub struct AgentRuntime {
    pub agent_loop: AgentLoop,
    pub ctx: ConversationContext,
    pending: Vec<Message>,
    system_prompt: String,
}

impl AgentRuntime {
    pub fn new(agent_loop: AgentLoop, system_prompt: String, image_high_count: usize, image_low_count: usize) -> Self {
        let ctx = ConversationContext::new(&system_prompt, image_high_count, image_low_count, None);
        Self {
            agent_loop,
            ctx,
            pending: Vec::new(),
            system_prompt,
        }
    }

    /// Restore an existing context (e.g. from session JSONL).
    pub fn with_context(
        agent_loop: AgentLoop,
        ctx: ConversationContext,
        system_prompt: String,
    ) -> Self {
        Self {
            agent_loop,
            ctx,
            pending: Vec::new(),
            system_prompt,
        }
    }

    /// Update the system prompt (e.g. after hot-reloading IDENTITY.md).
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = prompt;
    }

    // -----------------------------------------------------------------------
    // Message dispatch
    // -----------------------------------------------------------------------

    /// Dispatch a batch of messages into the correct queues.
    pub fn dispatch(&mut self, messages: Vec<Message>) {
        for msg in messages {
            if msg.is_steer() {
                self.inject_steer(&msg);
            } else {
                self.pending.push(msg);
            }
        }
    }

    /// Inject a single steer message into the AgentLoop's inject queue.
    fn inject_steer(&mut self, msg: &Message) {
        let value = json!({
            "content": format!("{} {}", msg.format_prefix(), msg.content),
            "sender": msg.sender,
            "priority": "steer",
        });
        self.agent_loop.inject_queue.push(value);
    }

    /// Check if there are pending collect messages waiting for a turn.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Return the count of pending collect messages.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    // -----------------------------------------------------------------------
    // Run turn
    // -----------------------------------------------------------------------

    /// Execute one Mode-B turn with all pending collect messages.
    ///
    /// Drains `pending` into JSON values and calls `AgentLoop::run_one_turn`.
    pub async fn run_turn(&mut self) {
        if self.pending.is_empty() {
            return;
        }

        let messages: Vec<serde_json::Value> = self
            .pending
            .drain(..)
            .map(|m| {
                json!({
                    "content": format!("{} {}", m.format_prefix(), m.content),
                    "sender": m.sender,
                    "priority": "collect",
                })
            })
            .collect();

        info!(
            "running turn with {} collect message(s)",
            messages.len()
        );

        self.agent_loop
            .run_one_turn(&mut self.ctx, &messages, &self.system_prompt)
            .await;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::{Brain, BrainResponse};
    use crate::error::Result;
    use crate::eye::{Eye, Screenshot};
    use crate::tool::ToolRegistry;
    use crate::types::{Config, MessagePriority};
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::HashMap;

    struct MockBrain;

    #[async_trait]
    impl Brain for MockBrain {
        async fn chat(&self, _: &[serde_json::Value], _: &[serde_json::Value]) -> Result<BrainResponse> {
            Ok(BrainResponse {
                content: Some("OK".into()),
                tool_calls: vec![],
                raw: json!({"role": "assistant", "content": "OK"}),
            })
        }
        async fn summarize(&self, _: &[serde_json::Value]) -> Result<String> {
            Ok("summary".into())
        }
    }

    struct MockEye;

    #[async_trait]
    impl Eye for MockEye {
        async fn capture(&self) -> Result<Screenshot> {
            Err(crate::error::CorpError::Agent {
                message: "mock".into(),
            })
        }
    }

    fn make_msg(content: &str, priority: MessagePriority) -> Message {
        Message {
            msg_id: None,
            sender: "alice".into(),
            content: content.into(),
            priority,
            metadata: HashMap::new(),
            timestamp: "2025-01-01T00:00:00Z".into(),
        }
    }

    fn make_runtime() -> AgentRuntime {
        let brain = Box::new(MockBrain);
        let eye: std::sync::Arc<dyn crate::eye::Eye> = std::sync::Arc::new(MockEye);
        let registry = ToolRegistry::new();
        let config = Config::default();
        let agent_loop = AgentLoop::new(brain, eye, registry, config, "test".into());
        AgentRuntime::new(agent_loop, "system prompt".into(), 3, 3)
    }

    #[test]
    fn dispatch_separates_collect_and_steer() {
        let mut rt = make_runtime();

        rt.dispatch(vec![
            make_msg("hello", MessagePriority::Collect),
            make_msg("urgent", MessagePriority::Steer),
            make_msg("world", MessagePriority::Collect),
        ]);

        assert_eq!(rt.pending_count(), 2);
        assert_eq!(rt.agent_loop.inject_queue.len(), 1);
    }

    #[tokio::test]
    async fn run_turn_drains_pending() {
        let mut rt = make_runtime();

        rt.dispatch(vec![
            make_msg("one", MessagePriority::Collect),
            make_msg("two", MessagePriority::Collect),
        ]);
        assert_eq!(rt.pending_count(), 2);

        rt.run_turn().await;
        assert_eq!(rt.pending_count(), 0);

        // Context should have: system + 2 user_reply + assistant = 4
        assert_eq!(rt.ctx.len(), 4);
    }

    #[test]
    fn no_pending_does_nothing() {
        let rt = make_runtime();
        assert!(!rt.has_pending());
    }
}
