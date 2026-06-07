use crate::app::state::{ChatMessage, ChatRole};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A globally-unique, monotonically increasing request id.
/// Global (not stored in state) because AppState is replaced on day switch.
pub fn next_request_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Events emitted by a worker thread back to the UI event loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmEvent {
    Started { id: u64 },
    Token { id: u64, text: String },
    Done { id: u64 },
    Error { id: u64, message: String },
}

impl LlmEvent {
    pub fn id(&self) -> u64 {
        match self {
            LlmEvent::Started { id }
            | LlmEvent::Token { id, .. }
            | LlmEvent::Done { id }
            | LlmEvent::Error { id, .. } => *id,
        }
    }
}

/// Everything a worker needs to perform one streaming chat completion.
pub struct ChatRequest {
    pub id: u64,
    pub base_url: String,
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
}

/// Result of parsing one line of an SSE stream.
#[derive(Debug, PartialEq, Eq)]
pub enum SseLine {
    Delta(String),
    Done,
    Ignore,
}

/// Pure parser for a single SSE line from an OpenAI-compatible stream.
pub fn parse_sse_line(line: &str) -> SseLine {
    let line = line.trim();
    if line.is_empty() {
        return SseLine::Ignore;
    }
    let Some(data) = line.strip_prefix("data:") else {
        return SseLine::Ignore;
    };
    let data = data.trim();
    if data == "[DONE]" {
        return SseLine::Done;
    }
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(v) => match v["choices"][0]["delta"]["content"].as_str() {
            Some(s) if !s.is_empty() => SseLine::Delta(s.to_string()),
            _ => SseLine::Ignore,
        },
        Err(_) => SseLine::Ignore,
    }
}

/// Build the OpenAI-compatible request body for a chat completion.
fn build_body(req: &ChatRequest) -> serde_json::Value {
    let mut messages: Vec<serde_json::Value> = Vec::new();
    if let Some(system) = &req.system && !system.is_empty() {
        messages.push(serde_json::json!({"role": "system", "content": system}));
    }
    for m in &req.messages {
        let role = match m.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        };
        messages.push(serde_json::json!({"role": role, "content": m.content}));
    }
    serde_json::json!({
        "model": req.model,
        "messages": messages,
        "stream": true,
    })
}

/// Spawn a worker thread that performs one streaming chat completion and emits
/// LlmEvents over `tx`. Send errors (UI gone) are ignored.
pub fn spawn(req: ChatRequest, tx: Sender<LlmEvent>) {
    std::thread::spawn(move || {
        let id = req.id;
        let _ = tx.send(LlmEvent::Started { id });

        let url = format!("{}/chat/completions", req.base_url.trim_end_matches('/'));
        let body = build_body(&req);

        let resp = match ureq::post(&url).send_json(body) {
            Ok(resp) => resp,
            Err(ureq::Error::Status(code, _)) => {
                let _ = tx.send(LlmEvent::Error {
                    id,
                    message: format!("LLM returned HTTP {code}"),
                });
                return;
            }
            Err(ureq::Error::Transport(t)) => {
                let _ = tx.send(LlmEvent::Error {
                    id,
                    message: format!("can't reach LLM at {}: {}", req.base_url, t),
                });
                return;
            }
        };

        let reader = BufReader::new(resp.into_reader());
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    let _ = tx.send(LlmEvent::Error {
                        id,
                        message: format!("stream read error: {e}"),
                    });
                    return;
                }
            };
            match parse_sse_line(&line) {
                SseLine::Delta(text) => {
                    let _ = tx.send(LlmEvent::Token { id, text });
                }
                SseLine::Done => {
                    let _ = tx.send(LlmEvent::Done { id });
                    return;
                }
                SseLine::Ignore => {}
            }
        }
        // Stream ended without an explicit [DONE].
        let _ = tx.send(LlmEvent::Done { id });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_delta_line() {
        let line = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        assert_eq!(parse_sse_line(line), SseLine::Delta("Hello".to_string()));
    }

    #[test]
    fn parse_done_line() {
        assert_eq!(parse_sse_line("data: [DONE]"), SseLine::Done);
    }

    #[test]
    fn parse_role_only_chunk_is_ignored() {
        // First chunk often has a role but no content.
        let line = r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#;
        assert_eq!(parse_sse_line(line), SseLine::Ignore);
    }

    #[test]
    fn parse_blank_and_garbage_ignored() {
        assert_eq!(parse_sse_line(""), SseLine::Ignore);
        assert_eq!(parse_sse_line(": keep-alive"), SseLine::Ignore);
        assert_eq!(parse_sse_line("data: {not json"), SseLine::Ignore);
    }

    #[test]
    fn event_id_accessor() {
        assert_eq!(LlmEvent::Token { id: 7, text: "x".into() }.id(), 7);
        assert_eq!(LlmEvent::Done { id: 9 }.id(), 9);
    }

    #[test]
    fn request_ids_are_monotonic() {
        let a = next_request_id();
        let b = next_request_id();
        assert!(b > a);
    }

    #[test]
    fn build_body_includes_system_and_messages_and_stream() {
        let req = ChatRequest {
            id: 1,
            base_url: "http://x/v1".to_string(),
            model: "m".to_string(),
            system: Some("sys".to_string()),
            messages: vec![ChatMessage { role: ChatRole::User, content: "hi".to_string() }],
        };
        let body = super::build_body(&req);
        assert_eq!(body["model"], "m");
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "sys");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "hi");
    }

    #[test]
    fn build_body_omits_empty_system() {
        let req = ChatRequest {
            id: 1,
            base_url: "http://x/v1".to_string(),
            model: "m".to_string(),
            system: Some(String::new()),
            messages: vec![ChatMessage { role: ChatRole::User, content: "hi".to_string() }],
        };
        let body = super::build_body(&req);
        assert_eq!(body["messages"][0]["role"], "user");
    }
}
