//! OpenAI Chat Completions adapter: `POST {base}/chat/completions` with `tools`, SSE streaming
//! with `delta.tool_calls` fragments, and `GET {base}/models` for the model list / key validation.
//! The base URL is configurable so OpenAI-compatible providers only need a descriptor entry.

use std::collections::BTreeMap;
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

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const RECOMMENDED_MODEL: &str = "gpt-5.6-terra";

pub struct OpenAiBackend {
    http: reqwest::Client,
    provider: ProviderId,
    base_url: String,
    api_key: String,
}

impl OpenAiBackend {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(ProviderId::Openai, api_key, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(
        provider: ProviderId,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            http: http::client(Duration::from_secs(180)),
            provider,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
        }
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base_url))
            .bearer_auth(&self.api_key)
    }
}

const NON_CHAT_MARKERS: &[&str] = &[
    "embedding",
    "tts",
    "whisper",
    "transcribe",
    "dall-e",
    "gpt-image",
    "moderation",
    "realtime",
    "audio",
    "instruct",
    "davinci",
    "babbage",
    "sora",
];

/// Chat models listed by `/models` (the endpoint also returns embeddings, speech, images, …).
pub fn is_chat_model(model: &str) -> bool {
    let id = model.to_ascii_lowercase();
    let family = ["gpt-", "chatgpt-", "o1", "o3", "o4"]
        .iter()
        .any(|p| id.starts_with(p));
    family && !NON_CHAT_MARKERS.iter().any(|m| id.contains(m))
}

/// GPT-4-class and later chat models and the o-series support function tools; the earliest
/// o1 previews did not.
pub fn is_tool_capable(model: &str) -> bool {
    let id = model.to_ascii_lowercase();
    is_chat_model(&id) && !id.starts_with("o1-mini") && !id.starts_with("o1-preview")
}

pub fn build_body(request: &ChatRequest, stream: bool) -> Value {
    let mut messages = vec![json!({"role": "system", "content": request.system})];
    for message in &request.messages {
        match message.role {
            Role::User => {
                let mut text = String::new();
                for block in &message.content {
                    match block {
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } => messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": content,
                        })),
                        ContentBlock::Text { text: t } => text.push_str(t),
                        _ => {}
                    }
                }
                if !text.is_empty() {
                    messages.push(json!({"role": "user", "content": text}));
                }
            }
            Role::Assistant => {
                let mut text = String::new();
                let mut calls = Vec::new();
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text: t } => text.push_str(t),
                        ContentBlock::ToolUse { id, name, input } => calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": match input {
                                    Value::String(raw) => raw.clone(),
                                    other => other.to_string(),
                                },
                            },
                        })),
                        _ => {}
                    }
                }
                if text.is_empty() && calls.is_empty() {
                    continue;
                }
                let mut wire = json!({
                    "role": "assistant",
                    "content": if text.is_empty() { Value::Null } else { json!(text) },
                });
                if !calls.is_empty() {
                    wire["tool_calls"] = json!(calls);
                }
                messages.push(wire);
            }
        }
    }
    let mut body = json!({
        "model": request.model,
        "messages": messages,
        "max_completion_tokens": request.max_tokens,
        "stream": stream,
    });
    if stream {
        body["stream_options"] = json!({"include_usage": true});
    }
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {"name": t.name, "description": t.description, "parameters": t.input_schema},
                })
            })
            .collect();
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    body
}

pub fn map_finish_reason(raw: &str) -> StopReason {
    match raw {
        "stop" => StopReason::EndTurn,
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "content_filter" => StopReason::Refusal,
        other => StopReason::Other {
            raw: other.to_owned(),
        },
    }
}

fn usage_of(value: &Value) -> Usage {
    Usage {
        input_tokens: value["prompt_tokens"].as_u64(),
        output_tokens: value["completion_tokens"].as_u64(),
    }
}

fn assemble(
    text: String,
    refusal: String,
    calls: Vec<(String, String, String)>,
    finish: &str,
    usage: Usage,
) -> ChatResponse {
    let mut content = Vec::new();
    super::push_text(&mut content, &text);
    if text.is_empty() {
        super::push_text(&mut content, &refusal);
    }
    let has_calls = !calls.is_empty();
    for (id, name, args) in calls {
        content.push(ContentBlock::ToolUse {
            id: if id.is_empty() {
                super::new_call_id()
            } else {
                id
            },
            name,
            input: super::parse_arguments(&args),
        });
    }
    let stop_reason = if has_calls {
        StopReason::ToolUse
    } else if !refusal.is_empty() && text.is_empty() {
        StopReason::Refusal
    } else {
        map_finish_reason(finish)
    };
    ChatResponse {
        content,
        stop_reason,
        usage,
    }
}

pub fn parse_completion(body: &Value) -> CoreResult<ChatResponse> {
    let choice = &body["choices"][0];
    if choice.is_null() {
        return Err(http::malformed(
            ProviderId::Openai,
            "response has no choices",
        ));
    }
    let message = &choice["message"];
    let calls = message["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|call| {
            (
                call["id"].as_str().unwrap_or_default().to_owned(),
                call["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                call["function"]["arguments"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect();
    Ok(assemble(
        message["content"].as_str().unwrap_or_default().to_owned(),
        message["refusal"].as_str().unwrap_or_default().to_owned(),
        calls,
        choice["finish_reason"].as_str().unwrap_or("stop"),
        usage_of(&body["usage"]),
    ))
}

#[derive(Debug, Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug)]
pub struct StreamState {
    provider: ProviderId,
    text: String,
    refusal: String,
    calls: BTreeMap<u64, PartialCall>,
    finish: Option<String>,
    usage: Usage,
}

impl StreamState {
    pub fn new(provider: ProviderId) -> Self {
        Self {
            provider,
            text: String::new(),
            refusal: String::new(),
            calls: BTreeMap::new(),
            finish: None,
            usage: Usage::default(),
        }
    }

    /// Returns `true` at `[DONE]`.
    pub fn on_event(
        &mut self,
        event: &SseEvent,
        deltas: &UnboundedSender<StreamDelta>,
    ) -> CoreResult<bool> {
        if event.data.trim() == "[DONE]" {
            return Ok(true);
        }
        let data: Value =
            serde_json::from_str(&event.data).map_err(|e| http::malformed(self.provider, e))?;
        if data["error"].is_object() {
            return Err(SentinelError::Provider {
                provider: self.provider.display_name().to_owned(),
                status: None,
                detail: http::error_message(&event.data),
            });
        }
        if data["usage"].is_object() {
            self.usage = usage_of(&data["usage"]);
        }
        for choice in data["choices"].as_array().into_iter().flatten() {
            if choice["index"].as_u64().unwrap_or(0) != 0 {
                continue;
            }
            let delta = &choice["delta"];
            if let Some(piece) = delta["content"].as_str() {
                self.text.push_str(piece);
                super::send_text(deltas, piece);
            }
            if let Some(piece) = delta["refusal"].as_str() {
                self.refusal.push_str(piece);
            }
            for (position, fragment) in delta["tool_calls"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                let index = fragment["index"].as_u64().unwrap_or(position as u64);
                let call = self.calls.entry(index).or_default();
                if let Some(id) = fragment["id"].as_str().filter(|s| !s.is_empty()) {
                    call.id = id.to_owned();
                }
                if let Some(name) = fragment["function"]["name"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                {
                    if call.name.is_empty() {
                        let _ = deltas.send(StreamDelta::ToolUseStarted {
                            id: call.id.clone(),
                            name: name.to_owned(),
                        });
                    }
                    call.name = name.to_owned();
                }
                if let Some(args) = fragment["function"]["arguments"].as_str() {
                    call.arguments.push_str(args);
                }
            }
            if let Some(reason) = choice["finish_reason"].as_str() {
                self.finish = Some(reason.to_owned());
            }
        }
        Ok(false)
    }

    pub fn finish(self) -> CoreResult<ChatResponse> {
        let Some(finish) = self.finish else {
            return Err(SentinelError::Network {
                detail: format!(
                    "the {} stream ended before the response was complete. Try again.",
                    self.provider.display_name()
                ),
            });
        };
        let calls = self
            .calls
            .into_values()
            .map(|c| (c.id, c.name, c.arguments))
            .collect();
        Ok(assemble(
            self.text,
            self.refusal,
            calls,
            &finish,
            self.usage,
        ))
    }
}

pub fn parse_models(body: &Value) -> Vec<ModelInfo> {
    let mut rows: Vec<(i64, ModelInfo)> = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m["id"].as_str()?;
            is_chat_model(id).then(|| {
                (
                    m["created"].as_i64().unwrap_or(0),
                    ModelInfo {
                        id: id.to_owned(),
                        display_name: id.to_owned(),
                        supports_tools: is_tool_capable(id),
                        context_window: None,
                        recommended: id == RECOMMENDED_MODEL,
                    },
                )
            })
        })
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    rows.into_iter().map(|(_, m)| m).collect()
}

#[async_trait]
impl AgentBackend for OpenAiBackend {
    fn provider(&self) -> ProviderId {
        self.provider
    }

    fn supports_tool_calling(&self, model: &str) -> bool {
        is_tool_capable(model)
    }

    async fn list_models(&self) -> CoreResult<Vec<ModelInfo>> {
        let response = http::send(self.provider, self.request(Method::GET, "/models")).await?;
        Ok(parse_models(&http::json(self.provider, response).await?))
    }

    async fn send_message(&self, request: &ChatRequest) -> CoreResult<ChatResponse> {
        let http_request = self
            .request(Method::POST, "/chat/completions")
            .json(&build_body(request, false));
        let body = http::json(
            self.provider,
            http::send(self.provider, http_request).await?,
        )
        .await?;
        parse_completion(&body)
    }

    async fn stream_response(
        &self,
        request: &ChatRequest,
        deltas: UnboundedSender<StreamDelta>,
    ) -> CoreResult<ChatResponse> {
        let http_request = self
            .request(Method::POST, "/chat/completions")
            .json(&build_body(request, true));
        let response = http::send(self.provider, http_request).await?;
        let mut state = StreamState::new(self.provider);
        super::drive_sse(self.provider, response, |event| {
            state.on_event(&event, &deltas)
        })
        .await?;
        state.finish()
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::backend::{ChatMessage, ToolDefinition};
    use crate::providers::test_support::sse_events;

    const STREAM: &str = include_str!("../../tests/fixtures/openai/stream_text_and_tools.sse");

    #[test]
    fn builds_chat_body_with_tool_round_trip() {
        let request = ChatRequest {
            model: "gpt-5.6-terra".into(),
            system: "sys".into(),
            messages: vec![
                ChatMessage {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "kill whatever's using port 5432".into(),
                    }],
                },
                ChatMessage {
                    role: Role::Assistant,
                    content: vec![
                        ContentBlock::ProviderData {
                            provider: ProviderId::Anthropic,
                            data: json!({}),
                        },
                        ContentBlock::ToolUse {
                            id: "call_port".into(),
                            name: "find_port_owner".into(),
                            input: json!({"port": 5432}),
                        },
                    ],
                },
                ChatMessage {
                    role: Role::User,
                    content: vec![ContentBlock::ToolResult {
                        tool_use_id: "call_port".into(),
                        content: "{\"pid\":812}".into(),
                        is_error: false,
                    }],
                },
            ],
            tools: vec![ToolDefinition {
                name: "find_port_owner".into(),
                description: "Who owns a port".into(),
                input_schema: json!({"type": "object"}),
            }],
            max_tokens: 4096,
            temperature: None,
        };
        assert_eq!(
            build_body(&request, true),
            json!({
                "model": "gpt-5.6-terra",
                "max_completion_tokens": 4096,
                "stream": true,
                "stream_options": {"include_usage": true},
                "messages": [
                    {"role": "system", "content": "sys"},
                    {"role": "user", "content": "kill whatever's using port 5432"},
                    {"role": "assistant", "content": null, "tool_calls": [
                        {"id": "call_port", "type": "function", "function": {"name": "find_port_owner", "arguments": "{\"port\":5432}"}}
                    ]},
                    {"role": "tool", "tool_call_id": "call_port", "content": "{\"pid\":812}"}
                ],
                "tools": [{"type": "function", "function": {"name": "find_port_owner", "description": "Who owns a port", "parameters": {"type": "object"}}}]
            })
        );
    }

    #[test]
    fn parses_stream_with_fragmented_parallel_tool_calls() {
        for split in [1, 5, 50, STREAM.len()] {
            let (tx, mut rx) = unbounded_channel();
            let mut state = StreamState::new(ProviderId::Openai);
            for event in sse_events(STREAM, split) {
                if state.on_event(&event, &tx).unwrap() {
                    break;
                }
            }
            let response = state.finish().unwrap();
            assert_eq!(response.stop_reason, StopReason::ToolUse);
            assert_eq!(response.usage.input_tokens, Some(2410));
            assert_eq!(
                response.content,
                vec![
                    ContentBlock::Text {
                        text: "Looking at who owns port 5432.".into()
                    },
                    ContentBlock::ToolUse {
                        id: "call_port".into(),
                        name: "find_port_owner".into(),
                        input: json!({"port": 5432})
                    },
                    ContentBlock::ToolUse {
                        id: "call_sockets".into(),
                        name: "list_network_sockets".into(),
                        input: json!({"state": "listen"})
                    },
                ]
            );
            drop(tx);
            let started = std::iter::from_fn(|| rx.try_recv().ok())
                .filter(|d| matches!(d, StreamDelta::ToolUseStarted { .. }))
                .count();
            assert_eq!(started, 2);
        }
    }

    #[test]
    fn stream_error_payload_and_truncation_are_errors() {
        let fixture = include_str!("../../tests/fixtures/openai/stream_error.sse");
        let (tx, _rx) = unbounded_channel();
        let mut state = StreamState::new(ProviderId::Openai);
        let err = sse_events(fixture, 9)
            .iter()
            .find_map(|e| state.on_event(e, &tx).err())
            .expect("error");
        assert!(err.to_string().contains("server had an error"), "{err}");

        let mut state = StreamState::new(ProviderId::Openai);
        let cut = &STREAM[..STREAM.find("\"finish_reason\":\"tool_calls\"").unwrap() - 40];
        let cut = &cut[..cut.rfind("\n\n").unwrap()];
        for event in sse_events(cut, 64) {
            state.on_event(&event, &tx).unwrap();
        }
        assert!(matches!(state.finish(), Err(SentinelError::Network { .. })));
    }

    #[test]
    fn parses_completion_with_invalid_arguments_preserved() {
        let body: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/openai/completion_tool_calls.json"
        ))
        .unwrap();
        let response = parse_completion(&body).unwrap();
        assert_eq!(response.stop_reason, StopReason::ToolUse);
        assert!(
            matches!(&response.content[1], ContentBlock::ToolUse { name, input, .. }
            if name == "move_to_trash" && input["paths"][0] == "/Users/me/Library/Caches/com.old.app")
        );
        assert!(
            matches!(&response.content[2], ContentBlock::ToolUse { input: Value::String(raw), .. }
            if raw == "{not json")
        );
    }

    #[test]
    fn model_list_keeps_chat_models_only() {
        let body: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/openai/models_list.json"))
                .unwrap();
        let ids: Vec<String> = parse_models(&body).into_iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            ["gpt-6-astra", "gpt-5.6-luna", "gpt-5.6-terra", "o4-mini"]
        );
    }
}
