//! Shared HTTP plumbing: client construction, response/transport error mapping with actionable
//! detail, and chunked body reading. Keys only ever travel in headers, never in URLs or errors.

use std::time::Duration;

use reqwest::{RequestBuilder, Response, StatusCode};
use serde_json::Value;

use sentinel_core::{CoreResult, SentinelError};

use crate::settings::ProviderId;

const MAX_ERROR_DETAIL: usize = 400;

/// `read_timeout` bounds the gap between streamed chunks (model load + first token for local models).
pub fn client(read_timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(read_timeout)
        .user_agent(concat!("Sentinel/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub async fn send(provider: ProviderId, request: RequestBuilder) -> CoreResult<Response> {
    let response = request
        .send()
        .await
        .map_err(|e| transport_error(provider, e))?;
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    Err(status_error(provider, status, &body))
}

pub async fn json(provider: ProviderId, response: Response) -> CoreResult<Value> {
    let bytes = response
        .bytes()
        .await
        .map_err(|e| transport_error(provider, e))?;
    serde_json::from_slice(&bytes).map_err(|e| malformed(provider, e))
}

pub async fn next_chunk(
    provider: ProviderId,
    response: &mut Response,
) -> CoreResult<Option<bytes::Bytes>> {
    response
        .chunk()
        .await
        .map_err(|e| transport_error(provider, e))
}

pub fn malformed(provider: ProviderId, detail: impl std::fmt::Display) -> SentinelError {
    SentinelError::Provider {
        provider: provider.display_name().to_owned(),
        status: None,
        detail: format!(
            "unexpected response format: {}",
            truncate(&detail.to_string())
        ),
    }
}

pub fn transport_error(provider: ProviderId, err: reqwest::Error) -> SentinelError {
    let err = err.without_url();
    let name = provider.display_name();
    let detail = if err.is_connect() {
        format!("could not connect to {name}: {err}. Check your internet connection or proxy.")
    } else if err.is_timeout() {
        format!("{name} did not respond in time. Try again, or pick a smaller model.")
    } else {
        format!("request to {name} failed: {err}")
    };
    SentinelError::Network { detail }
}

/// Extracts the provider's own message from the common error envelopes:
/// `{"error":{"message":..}}` (Anthropic, OpenAI, Gemini) and `{"error":".."}` (Ollama).
pub fn error_message(body: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let message = parsed.as_ref().and_then(|v| match &v["error"] {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("message").and_then(Value::as_str).map(str::to_owned),
        _ => v["message"].as_str().map(str::to_owned),
    });
    truncate(message.as_deref().unwrap_or(body).trim())
}

pub fn status_error(provider: ProviderId, status: StatusCode, body: &str) -> SentinelError {
    let name = provider.display_name();
    let message = error_message(body);
    let code = status.as_u16();
    let detail = if is_auth_status(code, body) {
        format!("{name} rejected the API key ({message}). Update it in Settings › AI agent.")
    } else if code == 429 {
        format!("{name} rate limit reached ({message}). Wait a moment and try again.")
    } else if code == 404 {
        format!("{message}. Check the model name and that your account can use it.")
    } else if code >= 500 {
        format!("{name} is having trouble (HTTP {code}): {message}. Try again shortly.")
    } else if message.is_empty() {
        format!("HTTP {code}")
    } else {
        message
    };
    SentinelError::Provider {
        provider: name.to_owned(),
        status: Some(code),
        detail,
    }
}

fn is_auth_status(code: u16, body: &str) -> bool {
    code == 401 || code == 403 || (code == 400 && body.contains("API_KEY_INVALID"))
}

/// True when a provider definitively rejected the credentials (as opposed to being unreachable).
pub fn is_auth_rejection(err: &SentinelError) -> bool {
    matches!(err, SentinelError::Provider { status: Some(code), detail, .. }
        if *code == 401 || *code == 403 || detail.contains("rejected the API key"))
}

pub fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_ERROR_DETAIL {
        return text.to_owned();
    }
    let cut: String = text.chars().take(MAX_ERROR_DETAIL).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_messages_from_each_envelope() {
        let anthropic = include_str!("../../tests/fixtures/anthropic/error_auth.json");
        assert_eq!(error_message(anthropic), "invalid x-api-key");
        let openai = include_str!("../../tests/fixtures/openai/error_auth.json");
        assert!(error_message(openai).starts_with("Incorrect API key provided"));
        let gemini = include_str!("../../tests/fixtures/gemini/error_invalid_key.json");
        assert_eq!(
            error_message(gemini),
            "API key not valid. Please pass a valid API key."
        );
        assert_eq!(
            error_message(r#"{"error":"model 'nope:1b' not found"}"#),
            "model 'nope:1b' not found"
        );
        assert_eq!(error_message("plain failure"), "plain failure");
    }

    #[test]
    fn maps_statuses_to_actionable_errors() {
        let gemini = include_str!("../../tests/fixtures/gemini/error_invalid_key.json");
        let err = status_error(ProviderId::Gemini, StatusCode::BAD_REQUEST, gemini);
        assert!(is_auth_rejection(&err), "{err:?}");

        let err = status_error(
            ProviderId::Anthropic,
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#,
        );
        assert!(!is_auth_rejection(&err));
        assert!(err.to_string().contains("rate limit"), "{err}");

        let err = status_error(ProviderId::Openai, StatusCode::BAD_GATEWAY, "<html>");
        assert!(err.to_string().contains("HTTP 502"), "{err}");
    }

    #[test]
    fn truncates_long_bodies() {
        let long = "x".repeat(2000);
        assert!(truncate(&long).chars().count() <= MAX_ERROR_DETAIL + 1);
    }
}
