//! Incremental parsers for the two streaming framings providers use: Server-Sent Events
//! (Anthropic, OpenAI, Gemini) and newline-delimited JSON (Ollama). Both accept arbitrary byte
//! chunk boundaries, including splits inside a multi-byte UTF-8 sequence.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    event: Option<String>,
    data: Vec<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.line(&String::from_utf8_lossy(&line), &mut out);
        }
        out
    }

    /// Flushes an event not followed by a blank line when the body ends.
    pub fn finish(&mut self) -> Vec<SseEvent> {
        let mut out = Vec::new();
        if !self.buf.is_empty() {
            let rest = std::mem::take(&mut self.buf);
            let text = String::from_utf8_lossy(&rest).into_owned();
            self.line(text.trim_end_matches('\r'), &mut out);
        }
        self.dispatch(&mut out);
        out
    }

    fn line(&mut self, line: &str, out: &mut Vec<SseEvent>) {
        if line.is_empty() {
            self.dispatch(out);
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => self.data.push(value.to_owned()),
            _ => {}
        }
    }

    fn dispatch(&mut self, out: &mut Vec<SseEvent>) {
        if self.data.is_empty() {
            self.event = None;
            return;
        }
        out.push(SseEvent {
            event: self.event.take(),
            data: std::mem::take(&mut self.data).join("\n"),
        });
    }
}

#[derive(Debug, Default)]
pub struct NdjsonParser {
    buf: Vec<u8>,
}

impl NdjsonParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<Result<Value, String>> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            Self::parse(&line, &mut out);
        }
        out
    }

    pub fn finish(&mut self) -> Vec<Result<Value, String>> {
        let mut out = Vec::new();
        let rest = std::mem::take(&mut self.buf);
        Self::parse(&rest, &mut out);
        out
    }

    fn parse(line: &[u8], out: &mut Vec<Result<Value, String>>) {
        let text = String::from_utf8_lossy(line);
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        out.push(serde_json::from_str(text).map_err(|e| format!("{e}: {text}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sse_all(input: &[u8], split: usize) -> Vec<SseEvent> {
        let mut p = SseParser::new();
        let mut out = Vec::new();
        for chunk in input.chunks(split.max(1)) {
            out.extend(p.push(chunk));
        }
        out.extend(p.finish());
        out
    }

    #[test]
    fn sse_parses_named_events_at_every_split() {
        let input = "event: message_start\ndata: {\"a\":1}\n\n: keepalive\n\nevent: ping\r\ndata: {}\r\n\r\ndata: héllo\ndata: wörld\n\n".as_bytes();
        let expected = vec![
            SseEvent {
                event: Some("message_start".into()),
                data: "{\"a\":1}".into(),
            },
            SseEvent {
                event: Some("ping".into()),
                data: "{}".into(),
            },
            SseEvent {
                event: None,
                data: "héllo\nwörld".into(),
            },
        ];
        for split in 1..input.len() {
            assert_eq!(sse_all(input, split), expected, "split {split}");
        }
    }

    #[test]
    fn sse_flushes_trailing_event_without_blank_line() {
        assert_eq!(
            sse_all(b"data: [DONE]", 4),
            vec![SseEvent {
                event: None,
                data: "[DONE]".into()
            }]
        );
    }

    #[test]
    fn ndjson_handles_splits_and_bad_lines() {
        let input = "{\"x\":\"ü\"}\n\n{\"y\":2}\nnot json\n{\"z\":3}".as_bytes();
        for split in 1..input.len() {
            let mut p = NdjsonParser::new();
            let mut out = Vec::new();
            for chunk in input.chunks(split) {
                out.extend(p.push(chunk));
            }
            out.extend(p.finish());
            assert_eq!(out.len(), 4, "split {split}");
            assert_eq!(out[0].as_ref().unwrap()["x"], "ü");
            assert!(out[2].is_err());
            assert_eq!(out[3].as_ref().unwrap()["z"], 3);
        }
    }
}
