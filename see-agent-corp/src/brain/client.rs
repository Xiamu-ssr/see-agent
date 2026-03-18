use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::consts::{DEFAULT_LLM_MAX_TOKENS, DEFAULT_SUMMARIZE_MAX_TOKENS};
use crate::error::{Result, CorpError};
use crate::types::LlmConfig;

use super::response::{BrainResponse, ToolCallInfo};

// ---------------------------------------------------------------------------
// Brain trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait Brain: Send + Sync {
    /// Send a chat completion request.
    ///
    /// `messages`: OpenAI-format message array.
    /// `tools`: OpenAI function-calling tool schemas.
    async fn chat(
        &self,
        messages: &[serde_json::Value],
        tools: &[serde_json::Value],
    ) -> Result<BrainResponse>;

    /// Summarize a conversation for context compaction.
    async fn summarize(&self, messages: &[serde_json::Value]) -> Result<String>;
}

// ---------------------------------------------------------------------------
// OpenAI-compatible Brain implementation
// ---------------------------------------------------------------------------

pub struct OpenAiBrain {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiBrain {
    pub fn new(config: &LlmConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
        }
    }

    /// Build with an explicit model override (for summarize with a different model).
    pub fn with_model(config: &LlmConfig, model: &str) -> Self {
        let mut brain = Self::new(config);
        brain.model = model.to_owned();
        brain
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }
}

#[async_trait]
impl Brain for OpenAiBrain {
    async fn chat(
        &self,
        messages: &[serde_json::Value],
        tools: &[serde_json::Value],
    ) -> Result<BrainResponse> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": DEFAULT_LLM_MAX_TOKENS,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(tools);
        }

        let resp = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| CorpError::Brain {
                message: format!("HTTP error: {e}"),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(CorpError::Brain {
                message: format!("API returned {status}: {text}"),
            });
        }

        let json: ApiResponse = resp.json().await.map_err(|e| CorpError::Brain {
            message: format!("response parse error: {e}"),
        })?;

        parse_response(json)
    }

    async fn summarize(&self, messages: &[serde_json::Value]) -> Result<String> {
        let system = serde_json::json!({
            "role": "system",
            "content": concat!(
                "你是一个对话摘要助手。请将以下对话压缩成简洁的摘要，",
                "保留关键操作步骤、发现的问题和解决方案。",
                "不要遗漏重要的文件路径、URL、错误信息。",
                "使用中文回复。"
            ),
        });

        let user = serde_json::json!({
            "role": "user",
            "content": format_messages_for_summary(messages),
        });

        let body = serde_json::json!({
            "model": self.model,
            "messages": [system, user],
            "max_tokens": DEFAULT_SUMMARIZE_MAX_TOKENS,
        });

        let resp = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| CorpError::Brain {
                message: format!("summarize HTTP error: {e}"),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(CorpError::Brain {
                message: format!("summarize API returned {status}: {text}"),
            });
        }

        let json: ApiResponse = resp.json().await.map_err(|e| CorpError::Brain {
            message: format!("summarize parse error: {e}"),
        })?;

        let br = parse_response(json)?;
        Ok(br.content.unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// OpenAI API response types (private)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ApiResponse {
    choices: Vec<ApiChoice>,
}

#[derive(Deserialize)]
struct ApiChoice {
    message: ApiMessage,
}

#[derive(Deserialize, Serialize, Clone)]
struct ApiMessage {
    #[serde(default)]
    role: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Deserialize, Serialize, Clone)]
struct ApiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ApiFunction,
}

#[derive(Deserialize, Serialize, Clone)]
struct ApiFunction {
    name: String,
    arguments: String,
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

fn parse_response(resp: ApiResponse) -> Result<BrainResponse> {
    let choice = resp.choices.into_iter().next().ok_or(CorpError::Brain {
        message: "empty choices array".to_owned(),
    })?;

    let msg = choice.message;
    let content = msg.content.clone();

    let tool_calls = msg
        .tool_calls
        .as_ref()
        .map(|tcs| {
            tcs.iter()
                .filter(|tc| tc.call_type == "function")
                .map(|tc| {
                    let arguments: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or_else(|e| {
                            warn!(
                                "failed to parse tool call arguments for {}: {e}",
                                tc.function.name
                            );
                            serde_json::json!({})
                        });
                    ToolCallInfo {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let raw = serde_json::to_value(&msg).unwrap_or(serde_json::json!({}));

    Ok(BrainResponse {
        content,
        tool_calls,
        raw,
    })
}

/// Format messages into a text block for summarization.
fn format_messages_for_summary(messages: &[serde_json::Value]) -> String {
    use crate::consts::MAX_SUMMARIZE_MSG_CHARS;

    let mut parts = Vec::new();
    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("?");
        let content = msg
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("[non-text]");
        let truncated = if content.len() > MAX_SUMMARIZE_MSG_CHARS {
            format!("{}...", &content[..MAX_SUMMARIZE_MSG_CHARS])
        } else {
            content.to_owned()
        };
        parts.push(format!("[{role}] {truncated}"));
    }
    parts.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_text_only() {
        let json: ApiResponse = serde_json::from_value(serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello!",
                }
            }]
        }))
        .unwrap();

        let br = parse_response(json).unwrap();
        assert_eq!(br.content.as_deref(), Some("Hello!"));
        assert!(br.tool_calls.is_empty());
    }

    #[test]
    fn parse_response_with_tool_calls() {
        let json: ApiResponse = serde_json::from_value(serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "click",
                            "arguments": "{\"x\": 100, \"y\": 200}"
                        }
                    }]
                }
            }]
        }))
        .unwrap();

        let br = parse_response(json).unwrap();
        assert!(br.content.is_none());
        assert_eq!(br.tool_calls.len(), 1);
        assert_eq!(br.tool_calls[0].name, "click");
        assert_eq!(br.tool_calls[0].arguments["x"], 100);
    }

    #[test]
    fn parse_response_invalid_tool_args() {
        let json: ApiResponse = serde_json::from_value(serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_bad",
                        "type": "function",
                        "function": {
                            "name": "click",
                            "arguments": "not valid json"
                        }
                    }]
                }
            }]
        }))
        .unwrap();

        let br = parse_response(json).unwrap();
        assert_eq!(br.tool_calls.len(), 1);
        // Falls back to empty object
        assert_eq!(br.tool_calls[0].arguments, serde_json::json!({}));
    }

    #[test]
    fn parse_response_empty_choices() {
        let json: ApiResponse = serde_json::from_value(serde_json::json!({
            "choices": []
        }))
        .unwrap();

        assert!(parse_response(json).is_err());
    }

    #[test]
    fn parse_response_mixed_content_and_tools() {
        let json: ApiResponse = serde_json::from_value(serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "I'll click there.",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "click",
                            "arguments": "{\"x\": 50, \"y\": 50}"
                        }
                    }, {
                        "id": "call_2",
                        "type": "function",
                        "function": {
                            "name": "screenshot",
                            "arguments": "{}"
                        }
                    }]
                }
            }]
        }))
        .unwrap();

        let br = parse_response(json).unwrap();
        assert_eq!(br.content.as_deref(), Some("I'll click there."));
        assert_eq!(br.tool_calls.len(), 2);
        assert_eq!(br.tool_calls[0].name, "click");
        assert_eq!(br.tool_calls[1].name, "screenshot");
    }

    #[test]
    fn format_summary_truncation() {
        let long_msg = serde_json::json!({
            "role": "user",
            "content": "x".repeat(1000),
        });
        let result = format_messages_for_summary(&[long_msg]);
        // Should be truncated to MAX_SUMMARIZE_MSG_CHARS + "..." + prefix
        assert!(result.len() < 1000);
        assert!(result.contains("..."));
    }

    #[test]
    fn openai_brain_constructs() {
        let config = LlmConfig {
            base_url: "https://api.example.com/v1/".to_owned(),
            api_key: "sk-test".to_owned(),
            model: "gpt-4o".to_owned(),
        };
        let brain = OpenAiBrain::new(&config);
        // Trailing slash stripped
        assert_eq!(brain.base_url, "https://api.example.com/v1");
        assert_eq!(brain.model, "gpt-4o");
    }

    #[test]
    fn openai_brain_with_model_override() {
        let config = LlmConfig::default();
        let brain = OpenAiBrain::with_model(&config, "gpt-4o-mini");
        assert_eq!(brain.model, "gpt-4o-mini");
    }

    #[test]
    fn raw_preserves_message_shape() {
        let json: ApiResponse = serde_json::from_value(serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "hi",
                    "tool_calls": [{
                        "id": "call_x",
                        "type": "function",
                        "function": {
                            "name": "finished",
                            "arguments": "{\"summary\": \"done\"}"
                        }
                    }]
                }
            }]
        }))
        .unwrap();

        let br = parse_response(json).unwrap();
        // raw should have role, content, tool_calls
        assert_eq!(br.raw["role"], "assistant");
        assert!(br.raw.get("tool_calls").is_some());
    }
}
