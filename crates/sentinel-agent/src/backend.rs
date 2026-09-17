//! Provider-neutral chat + tool-calling interface. Each adapter (Anthropic `tools`, OpenAI `tools`,
//! Gemini `functionDeclarations`, Ollama `/api/chat` tools) translates these types to and from its
//! wire format; nothing above this layer knows which provider is active.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use ts_rs::TS;

use sentinel_core::CoreResult;

use crate::settings::{ModelInfo, ProviderId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        /// JSON-encoded tool output (or error message when `is_error`).
        content: String,
        is_error: bool,
    },
    /// Opaque provider state that must be replayed verbatim to the provider that produced it
    /// (Anthropic thinking blocks, Gemini thought signatures, Ollama thinking text). Adapters for
    /// other providers skip it, so switching providers mid-conversation keeps history valid.
    ProviderData {
        provider: ProviderId,
        data: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatMessage {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

/// JSON Schema (draft 2020-12 subset: no `$ref`, no `oneOf`) so every provider accepts it verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Refusal,
    Other { raw: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamDelta {
    Text(String),
    ToolUseStarted { id: String, name: String },
}

#[async_trait]
pub trait AgentBackend: Send + Sync {
    fn provider(&self) -> ProviderId;

    fn supports_tool_calling(&self, model: &str) -> bool;

    /// Live model list. Doubles as the lightweight credential check for key validation.
    async fn list_models(&self) -> CoreResult<Vec<ModelInfo>>;

    async fn validate_credentials(&self) -> CoreResult<()> {
        self.list_models().await.map(|_| ())
    }

    async fn send_message(&self, request: &ChatRequest) -> CoreResult<ChatResponse>;

    /// Streams deltas to `deltas` and resolves to the fully assembled response.
    async fn stream_response(
        &self,
        request: &ChatRequest,
        deltas: UnboundedSender<StreamDelta>,
    ) -> CoreResult<ChatResponse>;
}
