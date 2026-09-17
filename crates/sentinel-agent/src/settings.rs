use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use sentinel_core::model::TimestampMs;

/// OS keychain service name. One entry per provider, account = [`ProviderId::keychain_account`].
pub const KEYCHAIN_SERVICE: &str = "com.rayanjain.sentinel";

/// Adding a provider = new variant + one `AgentBackend` adapter + descriptor entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProviderId {
    /// Local Ollama daemon; nothing leaves the machine.
    Ollama,
    /// Ollama-hosted models at ollama.com (API key). Same wire format as local Ollama.
    OllamaCloud,
    Anthropic,
    Openai,
    Gemini,
}

impl ProviderId {
    pub const ALL: [ProviderId; 5] = [
        Self::Ollama,
        Self::OllamaCloud,
        Self::Anthropic,
        Self::Openai,
        Self::Gemini,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama (local)",
            Self::OllamaCloud => "Ollama Cloud",
            Self::Anthropic => "Anthropic",
            Self::Openai => "OpenAI",
            Self::Gemini => "Google Gemini",
        }
    }

    pub fn keychain_account(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OllamaCloud => "ollama-cloud",
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::Gemini => "gemini",
        }
    }

    pub fn kind(self) -> ProviderKind {
        match self {
            Self::Ollama => ProviderKind::Local,
            _ => ProviderKind::Cloud,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProviderKind {
    /// Nothing leaves the machine.
    Local,
    /// Conversation and tool results are sent to the provider's API.
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub supports_tools: bool,
    pub context_window: Option<u32>,
    pub recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub display_name: String,
    pub kind: ProviderKind,
    pub requires_api_key: bool,
    /// Shown before the live model list loads (or when offline).
    pub suggested_models: Vec<ModelInfo>,
    /// Chat badge copy, e.g. "Requests sent to Anthropic API".
    pub data_notice: String,
}

/// Non-secret settings, persisted as JSON in the app config dir. API keys live only in the keychain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentSettings {
    pub active_provider: ProviderId,
    /// Model chosen per provider, so switching providers restores the previous choice.
    pub selected_models: BTreeMap<ProviderId, String>,
    pub ollama_base_url: String,
    /// Upper bound on model↔tool round-trips per user message.
    pub max_tool_rounds: u32,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            active_provider: ProviderId::Ollama,
            selected_models: BTreeMap::from([(ProviderId::Ollama, "llama3.1:8b".to_owned())]),
            ollama_base_url: "http://127.0.0.1:11434".to_owned(),
            max_tool_rounds: 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum KeyStatus {
    Missing,
    /// No key needed (local Ollama).
    NotRequired,
    /// Stored, but validation could not reach the provider (offline).
    Unverified {
        message: String,
    },
    Valid {
        checked_at_ms: TimestampMs,
    },
    /// Provider rejected the key; it was not stored.
    Invalid {
        message: String,
        checked_at_ms: TimestampMs,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderStatus {
    pub id: ProviderId,
    pub key: KeyStatus,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OllamaStatus {
    pub installed: bool,
    pub running: bool,
    pub version: Option<String>,
    pub base_url: String,
    /// Locally pulled models, with tool-calling support flagged.
    pub models: Vec<ModelInfo>,
}
