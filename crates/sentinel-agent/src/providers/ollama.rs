//! Ollama adapter for both the local daemon and Ollama Cloud (`https://ollama.com`, bearer key):
//! `POST /api/chat` with `tools` and NDJSON streaming, `GET /api/tags` + `POST /api/show` for
//! models and tool capability, `GET /api/version` for daemon detection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use sentinel_core::{CoreResult, SentinelError};

use super::http;
use crate::backend::{
    AgentBackend, ChatRequest, ChatResponse, ContentBlock, Role, StopReason, StreamDelta, Usage,
};
use crate::settings::{ModelInfo, ProviderId};

pub const DEFAULT_LOCAL_URL: &str = "http://127.0.0.1:11434";
pub const CLOUD_URL: &str = "https://ollama.com";
pub const RECOMMENDED_LOCAL_MODEL: &str = "llama3.1:8b";
pub const RECOMMENDED_CLOUD_MODEL: &str = "gpt-oss:120b";
/// Local context window requested per chat. The system prompt plus 21 tool schemas take ~3-5k
/// tokens, beyond Ollama's small default; 16k leaves room for several tool rounds while keeping an
/// 8B model's KV cache small enough for 16 GB machines.
const LOCAL_NUM_CTX: u64 = 16_384;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelCapabilities {
    pub tools: bool,
    pub context_length: Option<u64>,
}

pub struct OllamaBackend {
    http: reqwest::Client,
    provider: ProviderId,
    base_url: String,
    api_key: Option<String>,
    capabilities: Mutex<HashMap<String, ModelCapabilities>>,
}

impl OllamaBackend {
    pub fn local(base_url: impl Into<String>) -> Self {
        Self::build(ProviderId::Ollama, base_url.into(), None)
    }

    pub fn cloud(api_key: impl Into<String>) -> Self {
        Self::cloud_with_base_url(api_key, CLOUD_URL)
    }

    pub fn cloud_with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self::build(
            ProviderId::OllamaCloud,
            base_url.into(),
            Some(api_key.into()),
        )
    }

    fn build(provider: ProviderId, base_url: String, api_key: Option<String>) -> Self {
        Self {
            // Local models can take tens of seconds to load before the first chunk.
            http: http::client(Duration::from_secs(300)),
            provider,
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key,
            capabilities: Mutex::new(HashMap::new()),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let builder = self
            .http
            .request(method, format!("{}{path}", self.base_url));
        match &self.api_key {
            Some(key) => builder.bearer_auth(key),
            None => builder,
        }
    }

    fn is_local(&self) -> bool {
        self.provider == ProviderId::Ollama
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> CoreResult<reqwest::Response> {
        http::send(self.provider, request)
            .await
            .map_err(|err| self.explain(err))
    }

    /// Rewrites generic failures into Ollama-specific guidance.
    fn explain(&self, err: SentinelError) -> SentinelError {
        match err {
            SentinelError::Network { .. } if self.is_local() => SentinelError::Unavailable {
                feature: "Ollama".into(),
                reason: format!(
                    "Ollama is not running at {}. Start the Ollama app or run `ollama serve`.",
                    self.base_url
                ),
            },
            SentinelError::Provider {
                status: Some(404),
                detail,
                provider,
            } if self.is_local() && detail.contains("not found") => {
                let model = detail
                    .split('\'')
                    .nth(1)
                    .unwrap_or(RECOMMENDED_LOCAL_MODEL)
                    .to_owned();
                SentinelError::Provider {
                    provider,
                    status: Some(404),
                    detail: format!(
                        "The model {model} is not installed. Run `ollama pull {model}` and try again."
                    ),
                }
            }
            other => other,
        }
    }

    pub async fn version(&self) -> CoreResult<String> {
        let response = self.send(self.request(Method::GET, "/api/version")).await?;
        let body = http::json(self.provider, response).await?;
        Ok(body["version"].as_str().unwrap_or_default().to_owned())
    }

    /// `POST /api/show` capabilities, cached so `supports_tool_calling` can answer synchronously.
    pub async fn refresh_capabilities(&self, model: &str) -> CoreResult<ModelCapabilities> {
        let response = self
            .send(
                self.request(Method::POST, "/api/show")
                    .json(&json!({"model": model})),
            )
            .await?;
        let capabilities = parse_show(&http::json(self.provider, response).await?);
        self.cache(model, capabilities.clone());
        Ok(capabilities)
    }

    fn cache(&self, model: &str, capabilities: ModelCapabilities) {
        if let Ok(mut cache) = self.capabilities.lock() {
            cache.insert(model.to_owned(), capabilities);
        }
    }

    fn recommended(&self) -> &'static str {
        if self.is_local() {
            RECOMMENDED_LOCAL_MODEL
        } else {
            RECOMMENDED_CLOUD_MODEL
        }
    }
}

pub fn parse_show(body: &Value) -> ModelCapabilities {
    let tools = body["capabilities"]
        .as_array()
        .is_some_and(|caps| caps.iter().any(|c| c == "tools"));
    let context_length = body["model_info"].as_object().and_then(|info| {
        info.iter()
            .find(|(k, _)| k.ends_with(".context_length"))
            .and_then(|(_, v)| v.as_u64())
    });
    ModelCapabilities {
        tools,
        context_length,
    }
}

/// `/api/tags` rows. Recent daemons include `capabilities`; `None` means "ask `/api/show`".
pub fn parse_tags(body: &Value) -> Vec<(String, Option<ModelCapabilities>)> {
    body["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let name = m["name"].as_str()?.to_owned();
            let caps = m["capabilities"].as_array().map(|caps| ModelCapabilities {
                tools: caps.iter().any(|c| c == "tools"),
                context_length: m["details"]["context_length"].as_u64(),
            });
            Some((name, caps))
        })
        .collect()
}

pub fn build_body(request: &ChatRequest, local: bool, context_length: Option<u64>) -> Value {
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
                            "tool_name": super::tool_name_for(&request.messages, tool_use_id).unwrap_or("unknown_tool"),
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
                let mut thinking = String::new();
                let mut calls = Vec::new();
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text: t } => text.push_str(t),
                        ContentBlock::ToolUse { id, name, input } => calls.push(json!({
                            "id": id,
                            "function": {
                                "index": calls.len(),
                                "name": name,
                                "arguments": if input.is_object() { input.clone() } else { json!({}) },
                            },
                        })),
                        ContentBlock::ProviderData {
                            provider: ProviderId::Ollama | ProviderId::OllamaCloud,
                            data,
                        } => thinking.push_str(data["thinking"].as_str().unwrap_or_default()),
                        _ => {}
                    }
                }
                if text.is_empty() && calls.is_empty() {
                    continue;
                }
                let mut wire = json!({"role": "assistant", "content": text});
                if !thinking.is_empty() {
                    wire["thinking"] = json!(thinking);
                }
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
        "stream": true,
    });
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.input_schema}}))
            .collect();
    }
    let mut options = serde_json::Map::new();
    if local {
        let ctx = context_length.map_or(LOCAL_NUM_CTX, |c| c.min(LOCAL_NUM_CTX));
        options.insert("num_ctx".into(), json!(ctx));
    }
    if let Some(temperature) = request.temperature {
        options.insert("temperature".into(), json!(temperature));
    }
    if !options.is_empty() {
        body["options"] = Value::Object(options);
    }
    body
}

/// Accumulates `/api/chat` NDJSON chunks. Tool calls arrive whole, possibly across chunks.
#[derive(Debug)]
pub struct StreamState {
    provider: ProviderId,
    text: String,
    thinking: String,
    calls: Vec<(String, String, Value)>,
    done_reason: Option<String>,
    usage: Usage,
}

impl StreamState {
    pub fn new(provider: ProviderId) -> Self {
        Self {
            provider,
            text: String::new(),
            thinking: String::new(),
            calls: Vec::new(),
            done_reason: None,
            usage: Usage::default(),
        }
    }

    /// Returns `true` on the final `done` chunk.
    pub fn on_value(
        &mut self,
        value: &Value,
        deltas: &UnboundedSender<StreamDelta>,
    ) -> CoreResult<bool> {
        if let Some(error) = value["error"].as_str() {
            return Err(SentinelError::Provider {
                provider: self.provider.display_name().to_owned(),
                status: None,
                detail: http::truncate(error),
            });
        }
        let message = &value["message"];
        if let Some(piece) = message["content"].as_str() {
            self.text.push_str(piece);
            super::send_text(deltas, piece);
        }
        if let Some(piece) = message["thinking"].as_str() {
            self.thinking.push_str(piece);
        }
        for call in message["tool_calls"].as_array().into_iter().flatten() {
            let id = call["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(super::new_call_id);
            let name = call["function"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let arguments = match &call["function"]["arguments"] {
                Value::String(raw) => super::parse_arguments(raw),
                Value::Null => json!({}),
                other => other.clone(),
            };
            let _ = deltas.send(StreamDelta::ToolUseStarted {
                id: id.clone(),
                name: name.clone(),
            });
            self.calls.push((id, name, arguments));
        }
        if value["done"].as_bool() == Some(true) {
            self.done_reason = Some(value["done_reason"].as_str().unwrap_or("stop").to_owned());
            self.usage = Usage {
                input_tokens: value["prompt_eval_count"].as_u64(),
                output_tokens: value["eval_count"].as_u64(),
            };
            return Ok(true);
        }
        Ok(false)
    }

    pub fn finish(self) -> CoreResult<ChatResponse> {
        let Some(done_reason) = self.done_reason else {
            return Err(SentinelError::Network {
                detail: format!(
                    "the {} stream ended before the response was complete. Try again.",
                    self.provider.display_name()
                ),
            });
        };
        let mut content = Vec::new();
        if !self.thinking.is_empty() {
            content.push(ContentBlock::ProviderData {
                provider: self.provider,
                data: json!({"thinking": self.thinking}),
            });
        }
        super::push_text(&mut content, &self.text);
        let has_calls = !self.calls.is_empty();
        for (id, name, input) in self.calls {
            content.push(ContentBlock::ToolUse { id, name, input });
        }
        let stop_reason = match done_reason.as_str() {
            _ if has_calls => StopReason::ToolUse,
            "stop" => StopReason::EndTurn,
            "length" => StopReason::MaxTokens,
            other => StopReason::Other {
                raw: other.to_owned(),
            },
        };
        Ok(ChatResponse {
            content,
            stop_reason,
            usage: self.usage,
        })
    }
}

#[async_trait]
impl AgentBackend for OllamaBackend {
    fn provider(&self) -> ProviderId {
        self.provider
    }

    /// Answers from `/api/show` capabilities gathered by `list_models` / `refresh_capabilities`.
    /// Unknown models report `false` rather than guessing.
    fn supports_tool_calling(&self, model: &str) -> bool {
        self.capabilities
            .lock()
            .ok()
            .and_then(|cache| cache.get(model).map(|c| c.tools))
            .unwrap_or(false)
    }

    async fn list_models(&self) -> CoreResult<Vec<ModelInfo>> {
        let response = self.send(self.request(Method::GET, "/api/tags")).await?;
        let rows = parse_tags(&http::json(self.provider, response).await?);
        let mut models = Vec::with_capacity(rows.len());
        for (name, caps) in rows {
            let caps = match caps {
                Some(caps) => {
                    self.cache(&name, caps.clone());
                    caps
                }
                None => self.refresh_capabilities(&name).await.unwrap_or_default(),
            };
            models.push(ModelInfo {
                display_name: name.clone(),
                supports_tools: caps.tools,
                context_window: caps.context_length.map(|c| c.min(u32::MAX as u64) as u32),
                recommended: name == self.recommended(),
                id: name,
            });
        }
        Ok(models)
    }

    async fn validate_credentials(&self) -> CoreResult<()> {
        if self.is_local() {
            return self.version().await.map(|_| ());
        }
        // `/api/tags` is public on ollama.com; `/api/me` requires a valid key.
        self.send(self.request(Method::POST, "/api/me"))
            .await
            .map(|_| ())
    }

    async fn send_message(&self, request: &ChatRequest) -> CoreResult<ChatResponse> {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        self.stream_response(request, tx).await
    }

    async fn stream_response(
        &self,
        request: &ChatRequest,
        deltas: UnboundedSender<StreamDelta>,
    ) -> CoreResult<ChatResponse> {
        let context_length = self
            .capabilities
            .lock()
            .ok()
            .and_then(|cache| cache.get(&request.model).and_then(|c| c.context_length));
        let body = build_body(request, self.is_local(), context_length);
        let response = self
            .send(self.request(Method::POST, "/api/chat").json(&body))
            .await?;
        let mut state = StreamState::new(self.provider);
        super::drive_ndjson(self.provider, response, |value| {
            state.on_value(&value, &deltas)
        })
        .await
        .map_err(|err| self.explain(err))?;
        state.finish()
    }
}

/// Whether the Ollama binary is installed, independent of whether the daemon runs.
pub fn find_installation() -> Option<PathBuf> {
    let binary = if cfg!(windows) {
        "ollama.exe"
    } else {
        "ollama"
    };
    let from_path = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(binary))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let mut known: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        known.push("/Applications/Ollama.app/Contents/Resources/ollama".into());
        known.push("/opt/homebrew/bin/ollama".into());
        known.push("/usr/local/bin/ollama".into());
        if let Some(home) = &home {
            known.push(home.join("Applications/Ollama.app/Contents/Resources/ollama"));
        }
    } else if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            known.push(
                PathBuf::from(local)
                    .join("Programs")
                    .join("Ollama")
                    .join("ollama.exe"),
            );
        }
    } else {
        known.push("/usr/local/bin/ollama".into());
        known.push("/usr/bin/ollama".into());
        if let Some(home) = &home {
            known.push(home.join(".local/bin/ollama"));
        }
    }
    from_path
        .into_iter()
        .chain(known)
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::backend::{ChatMessage, ToolDefinition};
    use crate::providers::test_support::ndjson_values;

    fn parse(fixture: &str, provider: ProviderId, split: usize) -> CoreResult<ChatResponse> {
        let (tx, _rx) = unbounded_channel();
        let mut state = StreamState::new(provider);
        for value in ndjson_values(fixture, split) {
            if state.on_value(&value, &tx)? {
                break;
            }
        }
        state.finish()
    }

    #[test]
    fn parses_recorded_local_tool_call_stream() {
        let fixture = include_str!("../../tests/fixtures/ollama/stream_tool_call.ndjson");
        for split in [1, 17, fixture.len()] {
            let response = parse(fixture, ProviderId::Ollama, split).unwrap();
            assert_eq!(response.stop_reason, StopReason::ToolUse);
            assert_eq!(response.usage.input_tokens, Some(232));
            assert_eq!(
                response.content,
                vec![ContentBlock::ToolUse {
                    id: "call_fiwoix9b".into(),
                    name: "list_processes".into(),
                    input: json!({"limit": "5", "sort_by": "cpu"}),
                }]
            );
        }
    }

    #[test]
    fn parses_recorded_text_stream() {
        let fixture = include_str!("../../tests/fixtures/ollama/stream_text.ndjson");
        let (tx, mut rx) = unbounded_channel();
        let mut state = StreamState::new(ProviderId::Ollama);
        for value in ndjson_values(fixture, 11) {
            state.on_value(&value, &tx).unwrap();
        }
        let response = state.finish().unwrap();
        drop(tx);
        let streamed: String = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|d| match d {
                StreamDelta::Text(t) => Some(t),
                StreamDelta::ToolUseStarted { .. } => None,
            })
            .collect();
        assert!(
            matches!(&response.content[..], [ContentBlock::Text { text }] if *text == streamed && text.starts_with("It depends"))
        );
        assert!(matches!(
            response.stop_reason,
            StopReason::EndTurn | StopReason::MaxTokens
        ));
    }

    #[test]
    fn thinking_and_parallel_calls_round_trip_into_history() {
        let fixture = include_str!("../../tests/fixtures/ollama/stream_thinking_tool_call.ndjson");
        let response = parse(fixture, ProviderId::OllamaCloud, 23).unwrap();
        assert!(
            matches!(&response.content[0], ContentBlock::ProviderData { data, .. }
            if data["thinking"] == "User wants port 5432 owner. Call find_port_owner.")
        );
        let ids: Vec<String> = response
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(ids.len(), 2);

        let request = ChatRequest {
            model: RECOMMENDED_CLOUD_MODEL.into(),
            system: "sys".into(),
            messages: vec![
                ChatMessage {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "port 5432?".into(),
                    }],
                },
                ChatMessage {
                    role: Role::Assistant,
                    content: response.content.clone(),
                },
                ChatMessage {
                    role: Role::User,
                    content: vec![ContentBlock::ToolResult {
                        tool_use_id: ids[0].clone(),
                        content: "{\"pid\":812}".into(),
                        is_error: false,
                    }],
                },
            ],
            tools: vec![ToolDefinition {
                name: "find_port_owner".into(),
                description: "d".into(),
                input_schema: json!({"type": "object"}),
            }],
            max_tokens: 1000,
            temperature: None,
        };
        let body = build_body(&request, false, None);
        assert!(
            body.get("options").is_none(),
            "cloud requests leave num_ctx to the host"
        );
        let assistant = &body["messages"][2];
        assert_eq!(
            assistant["thinking"],
            "User wants port 5432 owner. Call find_port_owner."
        );
        assert_eq!(
            assistant["content"],
            "Checking which process listens on 5432."
        );
        assert_eq!(
            assistant["tool_calls"][0]["function"],
            json!({"index": 0, "name": "find_port_owner", "arguments": {"port": 5432}})
        );
        assert_eq!(
            body["messages"][3],
            json!({"role": "tool", "tool_name": "find_port_owner", "tool_call_id": ids[0], "content": "{\"pid\":812}"})
        );
        assert_eq!(body["tools"][0]["type"], "function");

        let local = build_body(&request, true, Some(8192));
        assert_eq!(local["options"]["num_ctx"], 8192);
        assert_eq!(
            build_body(&request, true, None)["options"]["num_ctx"],
            LOCAL_NUM_CTX
        );
    }

    #[test]
    fn mid_stream_error_and_truncation() {
        let fixture = include_str!("../../tests/fixtures/ollama/stream_error.ndjson");
        let err = parse(fixture, ProviderId::Ollama, 5).unwrap_err();
        assert!(err.to_string().contains("unexpected EOF"), "{err}");
        let cut = include_str!("../../tests/fixtures/ollama/stream_text.ndjson")
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(matches!(
            parse(&cut, ProviderId::Ollama, 64),
            Err(SentinelError::Network { .. })
        ));
    }

    #[test]
    fn capabilities_from_show_and_tags() {
        let show: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/ollama/show_llama31.json"
        ))
        .unwrap();
        assert_eq!(
            parse_show(&show),
            ModelCapabilities {
                tools: true,
                context_length: Some(131_072)
            }
        );
        let tags: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/ollama/tags_local.json"))
                .unwrap();
        let rows = parse_tags(&tags);
        assert_eq!(rows[0].0, "llama3.1:8b");
        assert_eq!(rows[0].1.as_ref().map(|c| c.tools), Some(true));
        assert_eq!(rows[1].1.as_ref().map(|c| c.tools), Some(false));
        assert_eq!(
            parse_tags(&json!({"models": [{"name": "old:1b"}]}))[0].1,
            None
        );
    }

    #[test]
    fn unknown_models_do_not_claim_tool_support() {
        let backend = OllamaBackend::local(DEFAULT_LOCAL_URL);
        assert!(!backend.supports_tool_calling("llama3.1:8b"));
        backend.cache(
            "llama3.1:8b",
            ModelCapabilities {
                tools: true,
                context_length: None,
            },
        );
        assert!(backend.supports_tool_calling("llama3.1:8b"));
    }

    #[test]
    fn explains_local_failures() {
        let backend = OllamaBackend::local("http://127.0.0.1:9");
        let unreachable = backend.explain(SentinelError::Network {
            detail: "refused".into(),
        });
        assert!(
            matches!(&unreachable, SentinelError::Unavailable { feature, .. } if feature == "Ollama")
        );
        let missing = backend.explain(http::status_error(
            ProviderId::Ollama,
            reqwest::StatusCode::NOT_FOUND,
            include_str!("../../tests/fixtures/ollama/error_model_not_found.json"),
        ));
        assert!(
            missing.to_string().contains("ollama pull nope:1b"),
            "{missing}"
        );
    }
}
