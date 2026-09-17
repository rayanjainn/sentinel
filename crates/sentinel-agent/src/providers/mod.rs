//! `AgentBackend` adapters, one per provider wire format, plus the helpers they share.

pub mod anthropic;
pub mod gemini;
pub mod http;
pub mod ollama;
pub mod openai;
pub mod stream;

use reqwest::Response;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use sentinel_core::CoreResult;

use crate::backend::{ChatMessage, ContentBlock, StreamDelta};
use crate::settings::ProviderId;
use stream::{NdjsonParser, SseEvent, SseParser};

/// Tool-call id for providers that do not issue one (Ollama, often Gemini). Shape is accepted by
/// every provider's id validation, so history stays valid after a provider switch.
pub fn new_call_id() -> String {
    format!("call_{}", uuid::Uuid::new_v4().simple())
}

/// Name of the tool whose `ToolUse` block has `tool_use_id`; Gemini and Ollama key results by name.
pub fn tool_name_for<'a>(messages: &'a [ChatMessage], tool_use_id: &str) -> Option<&'a str> {
    messages
        .iter()
        .flat_map(|m| m.content.iter())
        .find_map(|block| match block {
            ContentBlock::ToolUse { id, name, .. } if id == tool_use_id => Some(name.as_str()),
            _ => None,
        })
}

/// Parses streamed tool arguments. Unparseable arguments become a JSON string so the engine can
/// return a precise tool error to the model instead of failing the whole turn.
pub fn parse_arguments(raw: &str) -> Value {
    if raw.trim().is_empty() {
        return Value::Object(Default::default());
    }
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
}

pub fn send_text(deltas: &UnboundedSender<StreamDelta>, text: &str) {
    if !text.is_empty() {
        let _ = deltas.send(StreamDelta::Text(text.to_owned()));
    }
}

/// Appends streamed text, merging with a trailing text block.
pub fn push_text(content: &mut Vec<ContentBlock>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(ContentBlock::Text { text: last }) = content.last_mut() {
        last.push_str(text);
    } else {
        content.push(ContentBlock::Text {
            text: text.to_owned(),
        });
    }
}

/// Feeds an SSE body to `on_event` until it returns `true` or the body ends.
pub async fn drive_sse<F>(
    provider: ProviderId,
    mut response: Response,
    mut on_event: F,
) -> CoreResult<()>
where
    F: FnMut(SseEvent) -> CoreResult<bool>,
{
    let mut parser = SseParser::new();
    while let Some(chunk) = http::next_chunk(provider, &mut response).await? {
        for event in parser.push(&chunk) {
            if on_event(event)? {
                return Ok(());
            }
        }
    }
    for event in parser.finish() {
        if on_event(event)? {
            return Ok(());
        }
    }
    Ok(())
}

/// Feeds an NDJSON body to `on_value` until it returns `true` or the body ends.
pub async fn drive_ndjson<F>(
    provider: ProviderId,
    mut response: Response,
    mut on_value: F,
) -> CoreResult<()>
where
    F: FnMut(Value) -> CoreResult<bool>,
{
    let mut parser = NdjsonParser::new();
    while let Some(chunk) = http::next_chunk(provider, &mut response).await? {
        for line in parser.push(&chunk) {
            if on_value(line.map_err(|e| http::malformed(provider, e))?)? {
                return Ok(());
            }
        }
    }
    for line in parser.finish() {
        if on_value(line.map_err(|e| http::malformed(provider, e))?)? {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::stream::SseParser;
    use super::*;

    /// Splits a fixture into `split`-byte chunks, as a network would.
    pub fn sse_events(fixture: &str, split: usize) -> Vec<SseEvent> {
        let mut parser = SseParser::new();
        let mut out = Vec::new();
        for chunk in fixture.as_bytes().chunks(split.max(1)) {
            out.extend(parser.push(chunk));
        }
        out.extend(parser.finish());
        out
    }

    pub fn ndjson_values(fixture: &str, split: usize) -> Vec<Value> {
        let mut parser = NdjsonParser::new();
        let mut out = Vec::new();
        for chunk in fixture.as_bytes().chunks(split.max(1)) {
            out.extend(parser.push(chunk));
        }
        out.extend(parser.finish());
        out.into_iter().map(|v| v.expect("fixture line")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Role;

    #[test]
    fn call_ids_are_portable() {
        let id = new_call_id();
        assert!(id.len() <= 40);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn finds_tool_name_and_parses_arguments() {
        let messages = vec![ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "a".into(),
                name: "list_volumes".into(),
                input: Value::Null,
            }],
        }];
        assert_eq!(tool_name_for(&messages, "a"), Some("list_volumes"));
        assert_eq!(tool_name_for(&messages, "b"), None);
        assert_eq!(parse_arguments(""), serde_json::json!({}));
        assert_eq!(parse_arguments("{\"x\":1}"), serde_json::json!({"x": 1}));
        assert_eq!(parse_arguments("{broken"), Value::String("{broken".into()));
    }
}
