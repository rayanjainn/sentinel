//! Google Gemini adapter: `models/{model}:streamGenerateContent?alt=sse` with
//! `tools[].functionDeclarations`, `functionCall` / `functionResponse` parts, thought-signature
//! replay, and `GET models` for the model list / key validation. The key travels in the
//! `x-goog-api-key` header, never the URL.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Map, Value, json};
use tokio::sync::mpsc::UnboundedSender;

use sentinel_core::{CoreResult, SentinelError};

use super::http;
use super::stream::SseEvent;
use crate::backend::{
    AgentBackend, ChatMessage, ChatRequest, ChatResponse, ContentBlock, Role, StopReason,
    StreamDelta, Usage,
};
use crate::settings::{ModelInfo, ProviderId};

pub const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
pub const RECOMMENDED_MODEL: &str = "gemini-3.8-flash";
/// Documented validator bypass for function calls that did not originate from Gemini.
const FOREIGN_CALL_SIGNATURE: &str = "skip_thought_signature_validator";
const PROVIDER: ProviderId = ProviderId::Gemini;

pub struct GeminiBackend {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl GeminiBackend {
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
            .header("x-goog-api-key", &self.api_key)
    }
}

const NON_TOOL_MARKERS: &[&str] = &["embedding", "aqa", "image", "tts", "native-audio", "veo"];

pub fn is_tool_capable(model: &str) -> bool {
    let id = model.trim_start_matches("models/");
    id.starts_with("gemini-") && !NON_TOOL_MARKERS.iter().any(|m| id.contains(m))
}

/// Reduces a JSON Schema to the OpenAPI subset `functionDeclarations.parameters` accepts.
pub fn sanitize_schema(schema: &Value) -> Value {
    let Some(object) = schema.as_object() else {
        return schema.clone();
    };
    let mut out = Map::new();
    for (key, value) in object {
        match key.as_str() {
            "type" => {
                if let Some(t) = value.as_str() {
                    out.insert(key.clone(), json!(t.to_ascii_uppercase()));
                }
            }
            "description" | "required" | "enum" | "minimum" | "maximum" | "minItems"
            | "maxItems" | "nullable" => {
                out.insert(key.clone(), value.clone());
            }
            "items" => {
                out.insert(key.clone(), sanitize_schema(value));
            }
            "properties" => {
                let props = value
                    .as_object()
                    .map(|p| {
                        p.iter()
                            .map(|(name, s)| (name.clone(), sanitize_schema(s)))
                            .collect::<Map<_, _>>()
                    })
                    .unwrap_or_default();
                out.insert(key.clone(), Value::Object(props));
            }
            _ => {}
        }
    }
    Value::Object(out)
}

/// Gemini metadata stored next to a `ToolUse`: the API's own call id and thought signature.
fn call_meta<'a>(message: &'a ChatMessage, tool_use_id: &str) -> Option<&'a Value> {
    message.content.iter().find_map(|block| match block {
        ContentBlock::ProviderData {
            provider: ProviderId::Gemini,
            data,
        } if data["toolUseId"] == tool_use_id => Some(data),
        _ => None,
    })
}

fn api_call_id(messages: &[ChatMessage], tool_use_id: &str) -> Option<String> {
    messages
        .iter()
        .find_map(|m| call_meta(m, tool_use_id))
        .and_then(|meta| meta["callId"].as_str().map(str::to_owned))
}

pub fn build_body(request: &ChatRequest) -> Value {
    let mut contents = Vec::new();
    for message in &request.messages {
        let mut parts = Vec::new();
        let from_gemini = message.content.iter().any(|b| {
            matches!(
                b,
                ContentBlock::ProviderData {
                    provider: ProviderId::Gemini,
                    ..
                }
            )
        });
        let mut first_call = true;
        for block in &message.content {
            match block {
                ContentBlock::Text { text } if !text.is_empty() => {
                    parts.push(json!({"text": text}))
                }
                ContentBlock::ToolUse { id, name, input } => {
                    let meta = call_meta(message, id);
                    let mut call = json!({
                        "name": name,
                        "args": if input.is_object() { input.clone() } else { json!({}) },
                    });
                    if let Some(call_id) = meta.and_then(|m| m["callId"].as_str()) {
                        call["id"] = json!(call_id);
                    }
                    let mut part = json!({"functionCall": call});
                    if let Some(signature) = meta.and_then(|m| m["thoughtSignature"].as_str()) {
                        part["thoughtSignature"] = json!(signature);
                    } else if first_call && !from_gemini {
                        part["thoughtSignature"] = json!(FOREIGN_CALL_SIGNATURE);
                    }
                    first_call = false;
                    parts.push(part);
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let name = super::tool_name_for(&request.messages, tool_use_id)
                        .unwrap_or("unknown_tool");
                    let response = if *is_error {
                        json!({"error": content})
                    } else {
                        json!({"output": serde_json::from_str::<Value>(content).unwrap_or_else(|_| json!(content))})
                    };
                    let mut result = json!({"name": name, "response": response});
                    if let Some(call_id) = api_call_id(&request.messages, tool_use_id) {
                        result["id"] = json!(call_id);
                    }
                    parts.push(json!({"functionResponse": result}));
                }
                _ => {}
            }
        }
        if !parts.is_empty() {
            contents.push(json!({
                "role": match message.role { Role::User => "user", Role::Assistant => "model" },
                "parts": parts,
            }));
        }
    }
    let mut generation = json!({"maxOutputTokens": request.max_tokens});
    if let Some(temperature) = request.temperature {
        generation["temperature"] = json!(temperature);
    }
    let mut body = json!({
        "systemInstruction": {"parts": [{"text": request.system}]},
        "contents": contents,
        "generationConfig": generation,
    });
    if !request.tools.is_empty() {
        let declarations: Vec<Value> = request
            .tools
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "parameters": sanitize_schema(&t.input_schema)}))
            .collect();
        body["tools"] = json!([{"functionDeclarations": declarations}]);
        body["toolConfig"] = json!({"functionCallingConfig": {"mode": "AUTO"}});
    }
    body
}

pub fn map_finish_reason(raw: &str) -> StopReason {
    match raw {
        "STOP" => StopReason::EndTurn,
        "MAX_TOKENS" => StopReason::MaxTokens,
        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" | "IMAGE_SAFETY" => {
            StopReason::Refusal
        }
        other => StopReason::Other {
            raw: other.to_owned(),
        },
    }
}

/// Accumulates `GenerateContentResponse` chunks. Each SSE event is a complete response object
/// whose parts extend the previous ones.
#[derive(Debug, Default)]
pub struct StreamState {
    content: Vec<ContentBlock>,
    finish: Option<String>,
    usage: Usage,
}

impl StreamState {
    pub fn on_event(
        &mut self,
        event: &SseEvent,
        deltas: &UnboundedSender<StreamDelta>,
    ) -> CoreResult<bool> {
        let data: Value =
            serde_json::from_str(&event.data).map_err(|e| http::malformed(PROVIDER, e))?;
        self.on_response(&data, deltas)?;
        Ok(false)
    }

    pub fn on_response(
        &mut self,
        data: &Value,
        deltas: &UnboundedSender<StreamDelta>,
    ) -> CoreResult<()> {
        if data["error"].is_object() {
            return Err(SentinelError::Provider {
                provider: PROVIDER.display_name().to_owned(),
                status: data["error"]["code"].as_u64().map(|c| c as u16),
                detail: http::error_message(&data.to_string()),
            });
        }
        if let Some(meta) = data["usageMetadata"].as_object() {
            self.usage.input_tokens = meta.get("promptTokenCount").and_then(Value::as_u64);
            let output = ["candidatesTokenCount", "thoughtsTokenCount"]
                .iter()
                .filter_map(|k| meta.get(*k).and_then(Value::as_u64))
                .reduce(|a, b| a + b);
            if output.is_some() {
                self.usage.output_tokens = output;
            }
        }
        if let Some(reason) = data["promptFeedback"]["blockReason"].as_str() {
            super::push_text(
                &mut self.content,
                &format!("Gemini blocked this request ({reason})."),
            );
            self.finish = Some("SAFETY".to_owned());
            return Ok(());
        }
        let candidate = &data["candidates"][0];
        for part in candidate["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(call) = part.get("functionCall") {
                let id = super::new_call_id();
                let name = call["name"].as_str().unwrap_or_default().to_owned();
                let mut meta = json!({"toolUseId": id});
                if let Some(call_id) = call["id"].as_str() {
                    meta["callId"] = json!(call_id);
                }
                if let Some(signature) = part["thoughtSignature"].as_str() {
                    meta["thoughtSignature"] = json!(signature);
                }
                let _ = deltas.send(StreamDelta::ToolUseStarted {
                    id: id.clone(),
                    name: name.clone(),
                });
                self.content.push(ContentBlock::ProviderData {
                    provider: PROVIDER,
                    data: meta,
                });
                self.content.push(ContentBlock::ToolUse {
                    id,
                    name,
                    input: if call["args"].is_object() {
                        call["args"].clone()
                    } else {
                        json!({})
                    },
                });
            } else if part["thought"].as_bool() != Some(true)
                && let Some(text) = part["text"].as_str()
            {
                super::push_text(&mut self.content, text);
                super::send_text(deltas, text);
            }
        }
        if let Some(reason) = candidate["finishReason"].as_str() {
            self.finish = Some(reason.to_owned());
        }
        Ok(())
    }

    pub fn finish(self) -> CoreResult<ChatResponse> {
        let Some(finish) = self.finish else {
            return Err(SentinelError::Network {
                detail: "the Gemini stream ended before the response was complete. Try again."
                    .into(),
            });
        };
        let has_calls = self
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }));
        Ok(ChatResponse {
            stop_reason: if has_calls {
                StopReason::ToolUse
            } else {
                map_finish_reason(&finish)
            },
            content: self.content,
            usage: self.usage,
        })
    }
}

pub fn parse_models(body: &Value) -> Vec<ModelInfo> {
    body["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m["name"].as_str()?.trim_start_matches("models/").to_owned();
            let generates = m["supportedGenerationMethods"]
                .as_array()
                .is_some_and(|methods| methods.iter().any(|x| x == "generateContent"));
            (generates && id.starts_with("gemini-")).then(|| ModelInfo {
                display_name: m["displayName"].as_str().unwrap_or(&id).to_owned(),
                supports_tools: is_tool_capable(&id),
                context_window: m["inputTokenLimit"]
                    .as_u64()
                    .map(|v| v.min(u32::MAX as u64) as u32),
                recommended: id == RECOMMENDED_MODEL,
                id,
            })
        })
        .collect()
}

#[async_trait]
impl AgentBackend for GeminiBackend {
    fn provider(&self) -> ProviderId {
        PROVIDER
    }

    fn supports_tool_calling(&self, model: &str) -> bool {
        is_tool_capable(model)
    }

    async fn list_models(&self) -> CoreResult<Vec<ModelInfo>> {
        let mut models = Vec::new();
        let mut page_token = String::new();
        loop {
            let mut request = self
                .request(Method::GET, "/models")
                .query(&[("pageSize", "1000")]);
            if !page_token.is_empty() {
                request = request.query(&[("pageToken", page_token.as_str())]);
            }
            let body = http::json(PROVIDER, http::send(PROVIDER, request).await?).await?;
            models.extend(parse_models(&body));
            match body["nextPageToken"].as_str() {
                Some(token) if !token.is_empty() => page_token = token.to_owned(),
                _ => break,
            }
        }
        Ok(models)
    }

    async fn send_message(&self, request: &ChatRequest) -> CoreResult<ChatResponse> {
        let path = format!("/models/{}:generateContent", request.model);
        let http_request = self.request(Method::POST, &path).json(&build_body(request));
        let body = http::json(PROVIDER, http::send(PROVIDER, http_request).await?).await?;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut state = StreamState::default();
        state.on_response(&body, &tx)?;
        state.finish()
    }

    async fn stream_response(
        &self,
        request: &ChatRequest,
        deltas: UnboundedSender<StreamDelta>,
    ) -> CoreResult<ChatResponse> {
        let path = format!("/models/{}:streamGenerateContent", request.model);
        let http_request = self
            .request(Method::POST, &path)
            .query(&[("alt", "sse")])
            .json(&build_body(request));
        let response = http::send(PROVIDER, http_request).await?;
        let mut state = StreamState::default();
        super::drive_sse(PROVIDER, response, |event| state.on_event(&event, &deltas)).await?;
        state.finish()
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::backend::ToolDefinition;
    use crate::providers::test_support::sse_events;

    const STREAM: &str = include_str!("../../tests/fixtures/gemini/stream_text_and_calls.sse");

    fn parse_stream(fixture: &str, split: usize) -> CoreResult<ChatResponse> {
        let (tx, _rx) = unbounded_channel();
        let mut state = StreamState::default();
        for event in sse_events(fixture, split) {
            state.on_event(&event, &tx)?;
        }
        state.finish()
    }

    #[test]
    fn sanitizes_schema_to_openapi_subset() {
        let schema = json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "pids": {"type": "array", "items": {"type": "integer", "minimum": 1, "default": 3}, "maxItems": 5},
                "mode": {"type": "string", "enum": ["a", "b"], "description": "Mode"}
            },
            "required": ["pids"]
        });
        assert_eq!(
            sanitize_schema(&schema),
            json!({
                "type": "OBJECT",
                "properties": {
                    "pids": {"type": "ARRAY", "items": {"type": "INTEGER", "minimum": 1}, "maxItems": 5},
                    "mode": {"type": "STRING", "enum": ["a", "b"], "description": "Mode"}
                },
                "required": ["pids"]
            })
        );
    }

    #[test]
    fn stream_parse_then_replay_preserves_signature_and_call_ids() {
        for split in [1, 13, STREAM.len()] {
            let response = parse_stream(STREAM, split).unwrap();
            assert_eq!(response.stop_reason, StopReason::ToolUse);
            assert_eq!(response.usage.output_tokens, Some(161));
            let calls: Vec<(&str, &Value)> = response
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { name, input, .. } => Some((name.as_str(), input)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                calls,
                [
                    ("list_processes", &json!({"sort_by": "cpu", "limit": 10})),
                    ("get_resource_usage", &json!({}))
                ]
            );
            assert_eq!(
                response.content[0],
                ContentBlock::Text {
                    text: "I'll look at live CPU usage first.".into()
                }
            );

            let first_id = response
                .content
                .iter()
                .find_map(|b| match b {
                    ContentBlock::ToolUse { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .unwrap();
            let request = ChatRequest {
                model: RECOMMENDED_MODEL.into(),
                system: "sys".into(),
                messages: vec![
                    ChatMessage {
                        role: Role::User,
                        content: vec![ContentBlock::Text {
                            text: "cpu?".into(),
                        }],
                    },
                    ChatMessage {
                        role: Role::Assistant,
                        content: response.content.clone(),
                    },
                    ChatMessage {
                        role: Role::User,
                        content: vec![ContentBlock::ToolResult {
                            tool_use_id: first_id,
                            content: "{\"total\":412}".into(),
                            is_error: false,
                        }],
                    },
                ],
                tools: vec![ToolDefinition {
                    name: "list_processes".into(),
                    description: "d".into(),
                    input_schema: json!({"type": "object", "properties": {}}),
                }],
                max_tokens: 8192,
                temperature: None,
            };
            let body = build_body(&request);
            let model_parts = &body["contents"][1]["parts"];
            assert_eq!(
                model_parts[0],
                json!({"text": "I'll look at live CPU usage first."})
            );
            assert_eq!(
                model_parts[1],
                json!({"functionCall": {"id": "fc_7a1", "name": "list_processes", "args": {"sort_by": "cpu", "limit": 10}}, "thoughtSignature": "CiQBVKhc7xSig0001"})
            );
            assert_eq!(
                model_parts[2],
                json!({"functionCall": {"id": "fc_7a2", "name": "get_resource_usage", "args": {}}})
            );
            assert_eq!(
                body["contents"][2],
                json!({"role": "user", "parts": [{"functionResponse": {"id": "fc_7a1", "name": "list_processes", "response": {"output": {"total": 412}}}}]})
            );
            assert_eq!(
                body["systemInstruction"],
                json!({"parts": [{"text": "sys"}]})
            );
            assert_eq!(
                body["tools"][0]["functionDeclarations"][0]["parameters"]["type"],
                "OBJECT"
            );
        }
    }

    #[test]
    fn foreign_calls_get_validator_bypass() {
        let request = ChatRequest {
            model: RECOMMENDED_MODEL.into(),
            system: String::new(),
            messages: vec![ChatMessage {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "toolu_1".into(),
                    name: "list_volumes".into(),
                    input: json!({}),
                }],
            }],
            tools: vec![],
            max_tokens: 10,
            temperature: None,
        };
        let body = build_body(&request);
        assert_eq!(
            body["contents"][0]["parts"][0]["thoughtSignature"],
            FOREIGN_CALL_SIGNATURE
        );
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn blocked_prompt_is_a_refusal_and_truncation_is_an_error() {
        let blocked = parse_stream(
            include_str!("../../tests/fixtures/gemini/stream_blocked.sse"),
            8,
        )
        .unwrap();
        assert_eq!(blocked.stop_reason, StopReason::Refusal);
        let cut = &STREAM[..STREAM.find("\"finishReason\"").unwrap()];
        let cut = &cut[..cut.rfind("\n\n").unwrap()];
        assert!(matches!(
            parse_stream(cut, 64),
            Err(SentinelError::Network { .. })
        ));
    }

    #[test]
    fn parses_non_streaming_response_and_models() {
        let body: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/gemini/response_function_call.json"
        ))
        .unwrap();
        let (tx, _rx) = unbounded_channel();
        let mut state = StreamState::default();
        state.on_response(&body, &tx).unwrap();
        let response = state.finish().unwrap();
        assert_eq!(response.stop_reason, StopReason::ToolUse);
        assert!(
            response
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ProviderData { data, .. }
            if data["thoughtSignature"] == "CiQBSigNoId" && data.get("callId").is_none()))
        );

        let models: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/gemini/models_list.json"))
                .unwrap();
        let models = parse_models(&models);
        let ids: Vec<(&str, bool)> = models
            .iter()
            .map(|m| (m.id.as_str(), m.supports_tools))
            .collect();
        assert_eq!(
            ids,
            [
                ("gemini-3.8-flash", true),
                ("gemini-3.5-flash-lite", true),
                ("gemini-3.1-flash-image", false)
            ]
        );
        assert!(models[0].recommended);
    }

    #[test]
    fn api_error_payload_maps_to_provider_error() {
        let body: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/gemini/error_invalid_key.json"
        ))
        .unwrap();
        let (tx, _rx) = unbounded_channel();
        let err = StreamState::default().on_response(&body, &tx).unwrap_err();
        assert!(matches!(
            err,
            SentinelError::Provider {
                status: Some(400),
                ..
            }
        ));
    }
}
