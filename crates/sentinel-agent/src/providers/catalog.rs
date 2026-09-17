//! Provider catalog: descriptors shown in settings, suggested models, settings validation, and
//! backend construction. Adding a provider means a `ProviderId` variant, an adapter, and an entry
//! in each `match` here.

use std::sync::Arc;

use sentinel_core::{CoreResult, SentinelError};

use super::anthropic::{self, AnthropicBackend};
use super::gemini::{self, GeminiBackend};
use super::ollama::{self, OllamaBackend};
use super::openai::{self, OpenAiBackend};
use crate::backend::AgentBackend;
use crate::settings::{AgentSettings, ModelInfo, ProviderDescriptor, ProviderId, ProviderKind};

/// Upper bound for output tokens per model response (cloud adapters; Ollama ignores it).
pub const MAX_RESPONSE_TOKENS: u32 = 16_000;
pub const MAX_TOOL_ROUNDS_LIMIT: u32 = 32;

fn model(id: &str, name: &str, context: Option<u32>, recommended: bool) -> ModelInfo {
    ModelInfo {
        id: id.to_owned(),
        display_name: name.to_owned(),
        supports_tools: true,
        context_window: context,
        recommended,
    }
}

/// Shown before the live list loads or when it cannot be fetched. All support tool calling.
pub fn suggested_models(provider: ProviderId) -> Vec<ModelInfo> {
    match provider {
        ProviderId::Ollama => vec![
            model(
                ollama::RECOMMENDED_LOCAL_MODEL,
                "Llama 3.1 8B",
                Some(131_072),
                true,
            ),
            model("qwen3:8b", "Qwen3 8B", None, false),
            model("mistral-nemo:12b", "Mistral Nemo 12B", None, false),
        ],
        ProviderId::OllamaCloud => vec![
            model(
                ollama::RECOMMENDED_CLOUD_MODEL,
                "gpt-oss 120B",
                Some(131_072),
                true,
            ),
            model("gpt-oss:20b", "gpt-oss 20B", Some(131_072), false),
            model("kimi-k2.6", "Kimi K2.6", Some(262_144), false),
            model("glm-5.3", "GLM 5.3", Some(1_048_576), false),
            model(
                "deepseek-v4.1-flash",
                "DeepSeek V4.1 Flash",
                Some(1_048_576),
                false,
            ),
        ],
        ProviderId::Anthropic => vec![
            model(
                anthropic::RECOMMENDED_MODEL,
                "Claude Opus 5",
                Some(1_000_000),
                true,
            ),
            model("claude-sonnet-5", "Claude Sonnet 5", Some(1_000_000), false),
            model("claude-haiku-4-5", "Claude Haiku 4.5", Some(200_000), false),
        ],
        ProviderId::Openai => vec![
            model(
                openai::RECOMMENDED_MODEL,
                "GPT-5.6 Terra",
                Some(1_050_000),
                true,
            ),
            model("gpt-6-astra", "GPT-6 Astra", Some(1_050_000), false),
            model("gpt-5.6-luna", "GPT-5.6 Luna", Some(1_050_000), false),
        ],
        ProviderId::Gemini => vec![
            model(gemini::RECOMMENDED_MODEL, "Gemini 3.8 Flash", None, true),
            model(
                "gemini-3.5-flash-lite",
                "Gemini 3.5 Flash-Lite",
                None,
                false,
            ),
            model("gemini-2.5-pro", "Gemini 2.5 Pro", None, false),
        ],
    }
}

pub fn default_model(provider: ProviderId) -> String {
    suggested_models(provider)
        .into_iter()
        .find(|m| m.recommended)
        .map(|m| m.id)
        .unwrap_or_default()
}

pub fn requires_api_key(provider: ProviderId) -> bool {
    provider != ProviderId::Ollama
}

/// Host part of an `http(s)://host[:port][/path]` URL, without brackets for IPv6.
pub fn url_host(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split('/').next()?;
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if let Some(v6) = authority.strip_prefix('[') {
        return v6.split(']').next();
    }
    authority.split(':').next().filter(|h| !h.is_empty())
}

pub fn is_loopback_url(url: &str) -> bool {
    match url_host(url) {
        Some(host) => {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        }
        None => false,
    }
}

/// Where conversation data goes, in the words the chat badge uses.
pub fn data_notice(provider: ProviderId, settings: &AgentSettings) -> String {
    match provider {
        ProviderId::Ollama if is_loopback_url(&settings.ollama_base_url) => {
            "Runs locally — nothing leaves this machine".to_owned()
        }
        ProviderId::Ollama => format!(
            "Requests are sent to the Ollama server at {}",
            url_host(&settings.ollama_base_url).unwrap_or(&settings.ollama_base_url)
        ),
        ProviderId::OllamaCloud => {
            "Requests are sent to the Ollama Cloud API (ollama.com)".to_owned()
        }
        ProviderId::Anthropic => "Requests are sent to the Anthropic API".to_owned(),
        ProviderId::Openai => "Requests are sent to the OpenAI API".to_owned(),
        ProviderId::Gemini => "Requests are sent to the Gemini API".to_owned(),
    }
}

pub fn descriptor(provider: ProviderId, settings: &AgentSettings) -> ProviderDescriptor {
    let kind = match provider {
        ProviderId::Ollama if !is_loopback_url(&settings.ollama_base_url) => ProviderKind::Cloud,
        other => other.kind(),
    };
    ProviderDescriptor {
        id: provider,
        display_name: provider.display_name().to_owned(),
        kind,
        requires_api_key: requires_api_key(provider),
        suggested_models: suggested_models(provider),
        data_notice: data_notice(provider, settings),
    }
}

pub fn descriptors(settings: &AgentSettings) -> Vec<ProviderDescriptor> {
    ProviderId::ALL
        .iter()
        .map(|p| descriptor(*p, settings))
        .collect()
}

pub fn selected_model(settings: &AgentSettings, provider: ProviderId) -> String {
    settings
        .selected_models
        .get(&provider)
        .filter(|m| !m.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| default_model(provider))
}

/// Validates and normalizes settings from the UI.
pub fn sanitize_settings(mut settings: AgentSettings) -> CoreResult<AgentSettings> {
    let url = settings
        .ollama_base_url
        .trim()
        .trim_end_matches('/')
        .to_owned();
    if !(url.starts_with("http://") || url.starts_with("https://")) || url_host(&url).is_none() {
        return Err(SentinelError::invalid(
            "the Ollama address must be a URL such as http://127.0.0.1:11434",
        ));
    }
    settings.ollama_base_url = url;
    settings.max_tool_rounds = settings.max_tool_rounds.clamp(1, MAX_TOOL_ROUNDS_LIMIT);
    settings.selected_models = settings
        .selected_models
        .into_iter()
        .map(|(p, m)| (p, m.trim().to_owned()))
        .filter(|(_, m)| !m.is_empty())
        .collect();
    Ok(settings)
}

pub fn missing_key_error(provider: ProviderId) -> SentinelError {
    SentinelError::Unavailable {
        feature: format!("{} API key", provider.display_name()),
        reason: format!(
            "No API key is saved for {}. Add one in Settings › AI agent.",
            provider.display_name()
        ),
    }
}

/// Constructs the adapter for `provider`. Cloud providers need a key.
pub fn build_backend(
    provider: ProviderId,
    api_key: Option<String>,
    settings: &AgentSettings,
) -> CoreResult<Arc<dyn AgentBackend>> {
    let key = || {
        api_key
            .clone()
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| missing_key_error(provider))
    };
    Ok(match provider {
        ProviderId::Ollama => Arc::new(OllamaBackend::local(settings.ollama_base_url.clone())),
        ProviderId::OllamaCloud => Arc::new(OllamaBackend::cloud(key()?)),
        ProviderId::Anthropic => Arc::new(AnthropicBackend::new(key()?)),
        ProviderId::Openai => Arc::new(OpenAiBackend::new(key()?)),
        ProviderId::Gemini => Arc::new(GeminiBackend::new(key()?)),
    })
}

/// Backend ready for a turn with `model`: Ollama variants fetch the model's capabilities first,
/// so a missing model or a daemon that is not running fails here with specific guidance.
pub async fn prepare_backend(
    provider: ProviderId,
    api_key: Option<String>,
    settings: &AgentSettings,
    model: &str,
) -> CoreResult<Arc<dyn AgentBackend>> {
    match provider {
        ProviderId::Ollama => {
            let backend = OllamaBackend::local(settings.ollama_base_url.clone());
            backend.refresh_capabilities(model).await?;
            Ok(Arc::new(backend))
        }
        ProviderId::OllamaCloud => {
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| missing_key_error(provider))?;
            let backend = OllamaBackend::cloud(key);
            backend.refresh_capabilities(model).await?;
            Ok(Arc::new(backend))
        }
        _ => build_backend(provider, api_key, settings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_has_a_descriptor_and_a_recommended_tool_model() {
        let settings = AgentSettings::default();
        let all = descriptors(&settings);
        assert_eq!(all.len(), ProviderId::ALL.len());
        for d in &all {
            assert_eq!(
                d.suggested_models.iter().filter(|m| m.recommended).count(),
                1,
                "{:?}",
                d.id
            );
            assert!(d.suggested_models.iter().all(|m| m.supports_tools));
            assert!(!d.data_notice.is_empty());
        }
        let local = &all[0];
        assert_eq!(
            local.data_notice,
            "Runs locally — nothing leaves this machine"
        );
        assert!(!local.requires_api_key);
        assert_eq!(all[4].data_notice, "Requests are sent to the Gemini API");
        assert_eq!(default_model(ProviderId::Ollama), "llama3.1:8b");
    }

    #[test]
    fn remote_ollama_is_not_labelled_local() {
        let settings = AgentSettings {
            ollama_base_url: "http://gpu-box.lan:11434".into(),
            ..Default::default()
        };
        let d = descriptor(ProviderId::Ollama, &settings);
        assert_eq!(d.kind, ProviderKind::Cloud);
        assert_eq!(
            d.data_notice,
            "Requests are sent to the Ollama server at gpu-box.lan"
        );
        assert!(is_loopback_url("http://localhost:11434"));
        assert!(is_loopback_url("http://[::1]:11434/"));
        assert!(!is_loopback_url("https://ollama.com"));
    }

    #[test]
    fn settings_are_validated() {
        let mut settings = AgentSettings {
            ollama_base_url: " http://127.0.0.1:11434/ ".into(),
            max_tool_rounds: 500,
            ..Default::default()
        };
        settings
            .selected_models
            .insert(ProviderId::Gemini, "  ".into());
        let clean = sanitize_settings(settings).unwrap();
        assert_eq!(clean.ollama_base_url, "http://127.0.0.1:11434");
        assert_eq!(clean.max_tool_rounds, MAX_TOOL_ROUNDS_LIMIT);
        assert!(!clean.selected_models.contains_key(&ProviderId::Gemini));
        assert_eq!(
            selected_model(&clean, ProviderId::Gemini),
            "gemini-3.8-flash"
        );
        assert!(
            sanitize_settings(AgentSettings {
                ollama_base_url: "ftp://x".into(),
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn cloud_backends_require_a_key() {
        let settings = AgentSettings::default();
        assert!(build_backend(ProviderId::Ollama, None, &settings).is_ok());
        for provider in [
            ProviderId::OllamaCloud,
            ProviderId::Anthropic,
            ProviderId::Openai,
            ProviderId::Gemini,
        ] {
            let err = build_backend(provider, None, &settings)
                .err()
                .expect("key required");
            assert!(
                matches!(&err, SentinelError::Unavailable { feature, .. } if feature.ends_with("API key"))
            );
            let backend = build_backend(provider, Some("k".into()), &settings).unwrap();
            assert_eq!(backend.provider(), provider);
        }
    }
}
