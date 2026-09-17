//! Anthropic Messages API adapter: `POST /v1/messages` with `tools`, SSE streaming, and
//! `GET /v1/models` for the live model list / key validation.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use sentinel_core::{CoreResult, SentinelError};

use super::http;
use super::stream::SseEvent;
use crate::backend::{
    AgentBackend, ChatRequest, ChatResponse, ContentBlock, Role, StopReason, StreamDelta, Usage,
};
use crate::settings::{ModelInfo, ProviderId};

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const API_VERSION: &str = "2023-06-01";
pub const RECOMMENDED_MODEL: &str = "claude-opus-5";
const PROVIDER: ProviderId = ProviderId::Anthropic;

pub struct AnthropicBackend {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl AnthropicBackend {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            http: http::client(Duration::from_secs(180)),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
        }
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
    }
}

/// Every Claude 3+ model supports tool use; legacy text-completion models do not.
pub fn is_tool_capable(model: &str) -> bool {
    model.starts_with("claude-")
        && !model.starts_with("claude-2")
        && !model.starts_with("claude-instant")
}

pub fn build_body(request: &ChatRequest, stream: bool) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .filter_map(|message| {
            let content: Vec<Value> = message
                .content
                .iter()
                .filter_map(|block| block_to_wire(block, &request.model))
                .collect();
            (!content.is_empty()).then(|| {
                json!({
                    "role": match message.role { Role::User => "user", Role::Assistant => "assistant" },
                    "content": content,
                })
            })
        })
        .collect();
    let mut body = json!({
        "model": request.model,
        "max_tokens": request.max_tokens,
        "system": request.system,
        "messages": messages,
        "stream": stream,
    });
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.input_schema}))
            .collect();
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    body
}

fn block_to_wire(block: &ContentBlock, model: &str) -> Option<Value> {
    match block {
        ContentBlock::Text { text } if text.is_empty() => None,
        ContentBlock::Text { text } => Some(json!({"type": "text", "text": text})),
        ContentBlock::ToolUse { id, name, input } => {
            Some(json!({"type": "tool_use", "id": id, "name": name, "input": input}))
        }
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => Some(json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        })),
        // Thinking blocks are bound to the model that produced them.
        ContentBlock::ProviderData {
            provider: ProviderId::Anthropic,
            data,
        } if data["model"] == model => data.get("block").cloned(),
        ContentBlock::ProviderData { .. } => None,
    }
}

pub fn map_stop_reason(raw: &str) -> StopReason {
    match raw {
        "end_turn" => StopReason::EndTurn,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        "refusal" => StopReason::Refusal,
        other => StopReason::Other {
            raw: other.to_owned(),
        },
    }
}

fn opaque(model: &str, block: Value) -> ContentBlock {
    ContentBlock::ProviderData {
        provider: PROVIDER,
        data: json!({"model": model, "block": block}),
    }
}

/// Non-streaming response body → `ChatResponse`.
pub fn parse_message(body: &Value, requested_model: &str) -> CoreResult<ChatResponse> {
    let blocks = body["content"]
        .as_array()
        .ok_or_else(|| http::malformed(PROVIDER, "missing content array"))?;
    let mut content = Vec::new();
    for block in blocks {
        match block["type"].as_str() {
            Some("text") => super::push_text(&mut content, block["text"].as_str().unwrap_or("")),
            Some("tool_use") => content.push(ContentBlock::ToolUse {
                id: block["id"].as_str().unwrap_or_default().to_owned(),
                name: block["name"].as_str().unwrap_or_default().to_owned(),
                input: block["input"].clone(),
            }),
            Some("thinking" | "redacted_thinking") => {
                content.push(opaque(requested_model, block.clone()))
            }
            _ => {}
        }
    }
    Ok(ChatResponse {
        content,
        stop_reason: map_stop_reason(body["stop_reason"].as_str().unwrap_or("end_turn")),
        usage: Usage {
            input_tokens: body["usage"]["input_tokens"].as_u64(),
            output_tokens: body["usage"]["output_tokens"].as_u64(),
        },
    })
}

#[derive(Debug)]
enum Partial {
    Text(String),
    Tool {
        id: String,
        name: String,
        json: String,
    },
    Opaque(Value),
    Skip,
}

/// Accumulates `message_start` … `message_stop` events into a response.
#[derive(Debug, Default)]
pub struct StreamState {
    blocks: Vec<Partial>,
    stop_reason: Option<StopReason>,
    usage: Usage,
}

impl StreamState {
    /// Returns `true` once `message_stop` arrives.
    pub fn on_event(
        &mut self,
        event: &SseEvent,
        deltas: &UnboundedSender<StreamDelta>,
    ) -> CoreResult<bool> {
        let data: Value =
            serde_json::from_str(&event.data).map_err(|e| http::malformed(PROVIDER, e))?;
        let index = data["index"].as_u64().map(|i| i as usize);
        match data["type"].as_str().unwrap_or_default() {
            "message_start" => {
                self.usage.input_tokens = data["message"]["usage"]["input_tokens"].as_u64();
            }
            "content_block_start" => {
                let block = &data["content_block"];
                let partial = match block["type"].as_str() {
                    Some("text") => {
                        let text = block["text"].as_str().unwrap_or_default();
                        super::send_text(deltas, text);
                        Partial::Text(text.to_owned())
                    }
                    Some("tool_use") => {
                        let id = block["id"].as_str().unwrap_or_default().to_owned();
                        let name = block["name"].as_str().unwrap_or_default().to_owned();
                        let _ = deltas.send(StreamDelta::ToolUseStarted {
                            id: id.clone(),
                            name: name.clone(),
                        });
                        Partial::Tool {
                            id,
                            name,
                            json: String::new(),
                        }
                    }
                    Some("thinking" | "redacted_thinking") => Partial::Opaque(block.clone()),
                    _ => Partial::Skip,
                };
                let index = index.unwrap_or(self.blocks.len());
                if self.blocks.len() <= index {
                    self.blocks.resize_with(index + 1, || Partial::Skip);
                }
                self.blocks[index] = partial;
            }
            "content_block_delta" => {
                let delta = &data["delta"];
                match (
                    index.and_then(|i| self.blocks.get_mut(i)),
                    delta["type"].as_str(),
                ) {
                    (Some(Partial::Text(text)), Some("text_delta")) => {
                        let piece = delta["text"].as_str().unwrap_or_default();
                        text.push_str(piece);
                        super::send_text(deltas, piece);
                    }
                    (Some(Partial::Tool { json, .. }), Some("input_json_delta")) => {
                        json.push_str(delta["partial_json"].as_str().unwrap_or_default());
                    }
                    (Some(Partial::Opaque(block)), Some("thinking_delta")) => {
                        let mut thinking =
                            block["thinking"].as_str().unwrap_or_default().to_owned();
                        thinking.push_str(delta["thinking"].as_str().unwrap_or_default());
                        block["thinking"] = json!(thinking);
                    }
                    (Some(Partial::Opaque(block)), Some("signature_delta")) => {
                        block["signature"] = delta["signature"].clone();
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(reason) = data["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(map_stop_reason(reason));
                }
                if let Some(tokens) = data["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = Some(tokens);
                }
            }
            "message_stop" => return Ok(true),
            "error" => return Err(stream_error(&data["error"])),
            _ => {}
        }
        Ok(false)
    }

    pub fn finish(self, requested_model: &str) -> CoreResult<ChatResponse> {
        let Some(stop_reason) = self.stop_reason else {
            return Err(SentinelError::Network {
                detail: "the Anthropic stream ended before the response was complete. Try again."
                    .into(),
            });
        };
        let mut content = Vec::new();
        for partial in self.blocks {
            match partial {
                Partial::Text(text) => super::push_text(&mut content, &text),
                Partial::Tool { id, name, json } => content.push(ContentBlock::ToolUse {
                    id,
                    name,
                    input: super::parse_arguments(&json),
                }),
                Partial::Opaque(block) => content.push(opaque(requested_model, block)),
                Partial::Skip => {}
            }
        }
        Ok(ChatResponse {
            content,
            stop_reason,
            usage: self.usage,
        })
    }
}

fn stream_error(error: &Value) -> SentinelError {
    let kind = error["type"].as_str().unwrap_or("error");
    let message = error["message"].as_str().unwrap_or("stream error");
    let (status, detail) = match kind {
        "overloaded_error" => (
            Some(529),
            format!("Anthropic is overloaded right now ({message}). Try again shortly."),
        ),
        "rate_limit_error" => (
            Some(429),
            format!("Anthropic rate limit reached ({message}). Wait a moment and try again."),
        ),
        _ => (None, format!("{kind}: {message}")),
    };
    SentinelError::Provider {
        provider: PROVIDER.display_name().to_owned(),
        status,
        detail,
    }
}

pub fn parse_models(body: &Value) -> Vec<ModelInfo> {
    body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_owned();
            Some(ModelInfo {
                display_name: m["display_name"].as_str().unwrap_or(&id).to_owned(),
                supports_tools: is_tool_capable(&id),
                context_window: m["max_input_tokens"]
                    .as_u64()
                    .map(|v| v.min(u32::MAX as u64) as u32),
                recommended: id == RECOMMENDED_MODEL,
                id,
            })
        })
        .collect()
}

#[async_trait]
impl AgentBackend for AnthropicBackend {
    fn provider(&self) -> ProviderId {
        PROVIDER
    }

    fn supports_tool_calling(&self, model: &str) -> bool {
        is_tool_capable(model)
    }

    async fn list_models(&self) -> CoreResult<Vec<ModelInfo>> {
        let mut models = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let mut request = self
                .request(Method::GET, "/v1/models")
                .query(&[("limit", "1000")]);
            if let Some(after_id) = &after {
                request = request.query(&[("after_id", after_id)]);
            }
            let body = http::json(PROVIDER, http::send(PROVIDER, request).await?).await?;
            models.extend(parse_models(&body));
            match (body["has_more"].as_bool(), body["last_id"].as_str()) {
                (Some(true), Some(last)) => after = Some(last.to_owned()),
                _ => break,
            }
        }
        Ok(models)
    }

    async fn send_message(&self, request: &ChatRequest) -> CoreResult<ChatResponse> {
        let http_request = self
            .request(Method::POST, "/v1/messages")
            .json(&build_body(request, false));
        let body = http::json(PROVIDER, http::send(PROVIDER, http_request).await?).await?;
        parse_message(&body, &request.model)
    }

    async fn stream_response(
        &self,
        request: &ChatRequest,
        deltas: UnboundedSender<StreamDelta>,
    ) -> CoreResult<ChatResponse> {
        let http_request = self
            .request(Method::POST, "/v1/messages")
            .json(&build_body(request, true));
        let response = http::send(PROVIDER, http_request).await?;
        let mut state = StreamState::default();
        super::drive_sse(PROVIDER, response, |event| state.on_event(&event, &deltas)).await?;
        state.finish(&request.model)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::backend::{ChatMessage, ToolDefinition};
    use crate::providers::test_support::sse_events;

    const STREAM: &str =
        include_str!("../../tests/fixtures/anthropic/stream_text_thinking_tools.sse");

    fn request() -> ChatRequest {
        ChatRequest {
            model: "claude-opus-5".into(),
            system: "sys".into(),
            messages: vec![
                ChatMessage {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "what's eating my CPU?".into(),
                    }],
                },
                ChatMessage {
                    role: Role::Assistant,
                    content: vec![
                        ContentBlock::ProviderData {
                            provider: ProviderId::Anthropic,
                            data: json!({"model": "claude-opus-5", "block": {"type": "thinking", "thinking": "", "signature": "sig"}}),
                        },
                        ContentBlock::ProviderData {
                            provider: ProviderId::Anthropic,
                            data: json!({"model": "claude-sonnet-5", "block": {"type": "thinking", "thinking": "", "signature": "other"}}),
                        },
                        ContentBlock::ProviderData {
                            provider: ProviderId::Gemini,
                            data: json!({"thoughtSignature": "g"}),
                        },
                        ContentBlock::Text {
                            text: String::new(),
                        },
                        ContentBlock::ToolUse {
                            id: "toolu_1".into(),
                            name: "list_processes".into(),
                            input: json!({"limit": 5}),
                        },
                    ],
                },
                ChatMessage {
                    role: Role::User,
                    content: vec![ContentBlock::ToolResult {
                        tool_use_id: "toolu_1".into(),
                        content: "{\"total\":3}".into(),
                        is_error: false,
                    }],
                },
            ],
            tools: vec![ToolDefinition {
                name: "list_processes".into(),
                description: "List processes".into(),
                input_schema: json!({"type": "object", "properties": {}}),
            }],
            max_tokens: 16000,
            temperature: None,
        }
    }

    #[test]
    fn builds_messages_body_with_tools_and_replayed_thinking() {
        let body = build_body(&request(), true);
        assert_eq!(
            body,
            json!({
                "model": "claude-opus-5",
                "max_tokens": 16000,
                "system": "sys",
                "stream": true,
                "messages": [
                    {"role": "user", "content": [{"type": "text", "text": "what's eating my CPU?"}]},
                    {"role": "assistant", "content": [
                        {"type": "thinking", "thinking": "", "signature": "sig"},
                        {"type": "tool_use", "id": "toolu_1", "name": "list_processes", "input": {"limit": 5}}
                    ]},
                    {"role": "user", "content": [
                        {"type": "tool_result", "tool_use_id": "toolu_1", "content": "{\"total\":3}", "is_error": false}
                    ]}
                ],
                "tools": [{"name": "list_processes", "description": "List processes", "input_schema": {"type": "object", "properties": {}}}]
            })
        );
    }

    #[test]
    fn parses_stream_with_thinking_text_and_parallel_tools_at_any_split() {
        for split in [1, 7, 64, STREAM.len()] {
            let (tx, mut rx) = unbounded_channel();
            let mut state = StreamState::default();
            for event in sse_events(STREAM, split) {
                if state.on_event(&event, &tx).unwrap() {
                    break;
                }
            }
            let response = state.finish("claude-opus-5").unwrap();
            assert_eq!(response.stop_reason, StopReason::ToolUse);
            assert_eq!(response.usage.input_tokens, Some(2871));
            assert_eq!(response.usage.output_tokens, Some(112));
            assert!(
                matches!(&response.content[0], ContentBlock::ProviderData { provider: ProviderId::Anthropic, data }
                if data["block"]["signature"].as_str().unwrap().starts_with("EqQB"))
            );
            assert_eq!(
                response.content[1],
                ContentBlock::Text {
                    text: "Checking live CPU and memory first.".into()
                }
            );
            assert_eq!(
                response.content[2],
                ContentBlock::ToolUse {
                    id: "toolu_01A".into(),
                    name: "list_processes".into(),
                    input: json!({"sort_by": "cpu", "limit": 10})
                }
            );
            assert_eq!(
                response.content[3],
                ContentBlock::ToolUse {
                    id: "toolu_01B".into(),
                    name: "get_resource_usage".into(),
                    input: json!({})
                }
            );
            drop(tx);
            let mut text = String::new();
            let mut started = Vec::new();
            while let Ok(delta) = rx.try_recv() {
                match delta {
                    StreamDelta::Text(t) => text.push_str(&t),
                    StreamDelta::ToolUseStarted { name, .. } => started.push(name),
                }
            }
            assert_eq!(text, "Checking live CPU and memory first.");
            assert_eq!(started, ["list_processes", "get_resource_usage"]);
        }
    }

    #[test]
    fn mid_stream_overload_is_a_specific_error() {
        let fixture = include_str!("../../tests/fixtures/anthropic/stream_overloaded.sse");
        let (tx, _rx) = unbounded_channel();
        let mut state = StreamState::default();
        let err = sse_events(fixture, 16)
            .iter()
            .find_map(|event| state.on_event(event, &tx).err())
            .expect("error event");
        assert!(
            matches!(
                err,
                SentinelError::Provider {
                    status: Some(529),
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn truncated_stream_is_a_network_error() {
        let cut = &STREAM[..STREAM.find("event: message_delta").unwrap()];
        let (tx, _rx) = unbounded_channel();
        let mut state = StreamState::default();
        for event in sse_events(cut, 32) {
            state.on_event(&event, &tx).unwrap();
        }
        assert!(matches!(
            state.finish("claude-opus-5"),
            Err(SentinelError::Network { .. })
        ));
    }

    #[test]
    fn parses_non_streaming_message_and_models() {
        let body: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/anthropic/message_text_and_tool.json"
        ))
        .unwrap();
        let response = parse_message(&body, "claude-opus-5").unwrap();
        assert_eq!(response.stop_reason, StopReason::ToolUse);
        assert_eq!(response.content.len(), 3);
        assert!(
            matches!(&response.content[2], ContentBlock::ToolUse { name, input, .. }
            if name == "terminate_process" && input["pid"] == 812)
        );

        let models: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/anthropic/models_list.json"
        ))
        .unwrap();
        let models = parse_models(&models);
        assert_eq!(models.len(), 3);
        assert!(models.iter().all(|m| m.supports_tools));
        assert!(models[0].recommended);
        assert_eq!(models[2].context_window, Some(200_000));
    }

    #[test]
    fn tool_capability_is_per_family() {
        assert!(is_tool_capable("claude-haiku-4-5"));
        assert!(!is_tool_capable("claude-2.1"));
        assert!(!is_tool_capable("gpt-5.6"));
    }
}
