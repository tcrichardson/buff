# LLM Chat Panel (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a toggleable middle "Chat" panel that streams replies from a local LM Studio server, driven by `/ask` and `/clear` commands from the existing capture box, with per-day conversation persistence.

**Architecture:** Synchronous Ratatui TUI with no async runtime. Each `/ask` spawns a short-lived worker thread that does a blocking `ureq` streaming HTTP call and sends `LlmEvent`s over an `mpsc` channel. The event loop polls the channel each iteration (it already wakes ~10×/sec) and appends tokens to chat state. Request identity via a process-global atomic id provides cancellation (stale events dropped). The pure core (model/storage/config/command/actions) stays terminal-free and unit-tested; the TUI layer renders state.

**Tech Stack:** Rust (edition 2024), ratatui 0.30, crossterm 0.29, chrono, serde, toml, `ureq` 2.x (new), `serde_json` (new).

**Reference spec:** `docs/superpowers/specs/2026-06-07-llm-chat-panel-phase1-design.md`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `Cargo.toml` | Add `ureq`, `serde_json` |
| Modify | `src/config.rs` | LLM + chat config fields and defaults |
| Modify | `src/storage.rs` | `chat_path_for`, `load_chat`, `save_chat` |
| Create | `src/app/llm.rs` | `LlmEvent`, `ChatRequest`, `parse_sse_line`, `next_request_id`, `spawn` (HTTP/SSE worker) |
| Modify | `src/app/mod.rs` | Register `llm` module |
| Modify | `src/app/state.rs` | `ChatRole`, `ChatMessage`, `ChatState`, `Focus::Chat`, `chat` field, `handle_llm_event`, `save_chat` |
| Modify | `src/app/command.rs` | `Ask`/`Clear` variants + parse |
| Modify | `src/app/actions.rs` | Dispatch `Ask`/`Clear`; preserve sender + load sidecar on day switch |
| Modify | `src/app/input.rs` | `Ctrl-L` toggle, `Focus::Chat` Tab cycle + scroll actions |
| Create | `src/ui/chat_panel.rs` | Render conversation + thinking/status |
| Modify | `src/ui/mod.rs` | Register `chat_panel` module |
| Modify | `src/ui/layout.rs` | Conditional three-column split |
| Modify | `src/main.rs` | Create channel, store sender, poll events |
| Modify | `src/ui/help.rs`, `README.md` | Document new commands/keys/config |

**Task order rationale:** small shared types and pure modules first (config, storage, llm, state), then commands/actions that use them, then input/UI, then wiring, then docs. Enum-variant additions (`Focus::Chat`, `Command::Ask/Clear`) are introduced in the same task that adds their match arms so the crate always compiles at a task boundary.

---

## Task 1: Add dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `ureq` and `serde_json` to `[dependencies]`**

In `Cargo.toml`, under `[dependencies]`, add these two lines (keep the existing entries):

```toml
ureq = "2"
serde_json = "1"
```

- [ ] **Step 2: Verify it builds and the lockfile updates**

Run: `cargo build`
Expected: compiles successfully; `Cargo.lock` now lists `ureq` and `serde_json`.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add ureq and serde_json dependencies"
```

---

## Task 2: Config fields for LLM + chat panel

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests for the new defaults and parsing**

Add to the `tests` module in `src/config.rs`:

```rust
#[test]
fn llm_and_chat_defaults() {
    let config = Config::default();
    assert_eq!(config.llm_base_url, "http://localhost:1234/v1");
    assert_eq!(config.llm_model, "google/gemma-4-12b-qat");
    assert_eq!(config.llm_system_prompt, "");
    assert_eq!(config.chat_width, 40);
    assert!(config.chat_visible);
}

#[test]
fn parse_llm_and_chat_fields_from_toml() {
    let toml = r#"
        llm_base_url = "http://127.0.0.1:9999/v1"
        llm_model = "my-model"
        llm_system_prompt = "be terse"
        chat_width = 50
        chat_visible = false
    "#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.llm_base_url, "http://127.0.0.1:9999/v1");
    assert_eq!(config.llm_model, "my-model");
    assert_eq!(config.llm_system_prompt, "be terse");
    assert_eq!(config.chat_width, 50);
    assert!(!config.chat_visible);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib config::tests::llm_and_chat_defaults`
Expected: FAIL — compile error, fields don't exist.

- [ ] **Step 3: Add the fields and defaults**

In `src/config.rs`, add these fields to the `Config` struct (after `capture_height`):

```rust
    pub llm_base_url: String,
    pub llm_model: String,
    pub llm_system_prompt: String,
    pub chat_width: u16,
    pub chat_visible: bool,
```

In `impl Default for Config`, add to the returned `Self { ... }` (after `capture_height: 5,`):

```rust
            llm_base_url: "http://localhost:1234/v1".to_string(),
            llm_model: "google/gemma-4-12b-qat".to_string(),
            llm_system_prompt: String::new(),
            chat_width: 40,
            chat_visible: true,
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib config::tests`
Expected: PASS (all config tests).

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: add LLM and chat-panel config fields"
```

---

## Task 3: ChatRole / ChatMessage types

**Files:**
- Modify: `src/app/state.rs`

- [ ] **Step 1: Write a failing serde round-trip test**

Add a `tests` module at the bottom of `src/app/state.rs` (the file currently has none):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_json_roundtrip() {
        let msgs = vec![
            ChatMessage { role: ChatRole::User, content: "hi".to_string() },
            ChatMessage { role: ChatRole::Assistant, content: "hello".to_string() },
        ];
        let json = serde_json::to_string(&msgs).unwrap();
        let back: Vec<ChatMessage> = serde_json::from_str(&json).unwrap();
        assert_eq!(msgs, back);
        // role serializes lowercase
        assert!(json.contains("\"role\":\"user\""), "got: {}", json);
        assert!(json.contains("\"role\":\"assistant\""), "got: {}", json);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib app::state::tests::chat_message_json_roundtrip`
Expected: FAIL — compile error, types don't exist.

- [ ] **Step 3: Add the types**

At the top of `src/app/state.rs` (after the existing `use` lines), add:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib app::state::tests::chat_message_json_roundtrip`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app/state.rs
git commit -m "feat: add ChatRole and ChatMessage types"
```

---

## Task 4: Chat sidecar persistence in storage

**Files:**
- Modify: `src/storage.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/storage.rs`:

```rust
#[test]
fn chat_path_for_uses_chat_json_suffix() {
    let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
    let dir = std::path::PathBuf::from("/tmp/notes");
    assert_eq!(
        chat_path_for(&dir, date, "%Y-%m-%d-%a"),
        std::path::PathBuf::from("/tmp/notes/2026-06-04-Thu.chat.json")
    );
}

#[test]
fn save_then_load_chat_roundtrip() {
    use crate::app::state::{ChatMessage, ChatRole};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("c.chat.json");
    let msgs = vec![
        ChatMessage { role: ChatRole::User, content: "q".to_string() },
        ChatMessage { role: ChatRole::Assistant, content: "a".to_string() },
    ];
    save_chat(&path, &msgs).unwrap();
    assert_eq!(load_chat(&path), msgs);
}

#[test]
fn save_empty_removes_file() {
    use crate::app::state::ChatMessage;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("c.chat.json");
    std::fs::write(&path, "[]").unwrap();
    let empty: Vec<ChatMessage> = vec![];
    save_chat(&path, &empty).unwrap();
    assert!(!path.exists(), "empty conversation should remove the sidecar");
}

#[test]
fn load_missing_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("nope.chat.json");
    assert!(load_chat(&path).is_empty());
}

#[test]
fn load_malformed_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("bad.chat.json");
    std::fs::write(&path, "{ not json").unwrap();
    assert!(load_chat(&path).is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib storage::tests::save_then_load_chat_roundtrip`
Expected: FAIL — compile error, functions don't exist.

- [ ] **Step 3: Implement the functions**

Add to `src/storage.rs` (after `path_for`):

```rust
pub fn chat_path_for(notes_dir: &Path, date: NaiveDate, date_format: &str) -> PathBuf {
    notes_dir.join(format!("{}.chat.json", date.format(date_format)))
}

pub fn load_chat(path: &Path) -> Vec<crate::app::state::ChatMessage> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_chat(
    path: &Path,
    messages: &[crate::app::state::ChatMessage],
) -> anyhow::Result<()> {
    if messages.is_empty() {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(messages)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
```

Add `use anyhow;` is unnecessary (anyhow used via full path). Ensure the file already imports `std::path::{Path, PathBuf}` (it does).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib storage::tests`
Expected: PASS (all storage tests).

- [ ] **Step 5: Commit**

```bash
git add src/storage.rs
git commit -m "feat: add per-day chat sidecar load/save"
```

---

## Task 5: LLM module — events, request, SSE parser, worker

**Files:**
- Create: `src/app/llm.rs`
- Modify: `src/app/mod.rs`

- [ ] **Step 1: Register the module**

In `src/app/mod.rs`, add `pub mod llm;` (keep alphabetical order):

```rust
pub mod actions;
pub mod command;
pub mod input;
pub mod llm;
pub mod state;
```

- [ ] **Step 2: Write the module with a failing SSE-parser test**

Create `src/app/llm.rs`:

```rust
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
}
```

- [ ] **Step 3: Run to verify the parser tests fail then pass**

Run: `cargo test --lib app::llm::tests`
Expected: PASS (the module compiles and all parser tests pass). If you wrote the test first and the impl second within this file, the intermediate failure is the compile error.

- [ ] **Step 4: Add the streaming worker `spawn`**

Append to `src/app/llm.rs` (before the `#[cfg(test)]` module):

```rust
/// Build the OpenAI-compatible request body for a chat completion.
fn build_body(req: &ChatRequest) -> serde_json::Value {
    let mut messages: Vec<serde_json::Value> = Vec::new();
    if let Some(system) = &req.system {
        if !system.is_empty() {
            messages.push(serde_json::json!({"role": "system", "content": system}));
        }
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
```

- [ ] **Step 5: Add a test for `build_body` (pure, no network)**

Add to the `tests` module in `src/app/llm.rs`:

```rust
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
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --lib app::llm::tests`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/app/mod.rs src/app/llm.rs
git commit -m "feat: add LLM event types, SSE parser, and streaming worker"
```

---

## Task 6: ChatState, Focus integration, handle_llm_event

**Files:**
- Modify: `src/app/state.rs`
- Modify: `src/ui/layout.rs` (test helper), `src/ui/right_panel.rs` (test literals)

- [ ] **Step 1: Write failing tests for `handle_llm_event`**

Add to the `tests` module in `src/app/state.rs`:

```rust
use crate::app::llm::LlmEvent;
use crate::config::Config;
use chrono::NaiveDate;

fn chat_state_with(messages: Vec<ChatMessage>, active: u64, pending: bool) -> AppState {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = AppState::open_day(
        tmp.path().to_path_buf(),
        Config::default(),
        NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
    )
    .unwrap();
    s.chat.messages = messages;
    s.chat.active_request = active;
    s.chat.pending = pending;
    s
}

#[test]
fn token_appends_to_last_assistant() {
    let mut s = chat_state_with(
        vec![
            ChatMessage { role: ChatRole::User, content: "q".into() },
            ChatMessage { role: ChatRole::Assistant, content: String::new() },
        ],
        5,
        true,
    );
    s.handle_llm_event(LlmEvent::Token { id: 5, text: "Hel".into() });
    s.handle_llm_event(LlmEvent::Token { id: 5, text: "lo".into() });
    assert_eq!(s.chat.messages.last().unwrap().content, "Hello");
}

#[test]
fn stale_token_is_ignored() {
    let mut s = chat_state_with(
        vec![ChatMessage { role: ChatRole::Assistant, content: String::new() }],
        5,
        true,
    );
    s.handle_llm_event(LlmEvent::Token { id: 4, text: "nope".into() });
    assert_eq!(s.chat.messages.last().unwrap().content, "");
}

#[test]
fn done_clears_pending() {
    let mut s = chat_state_with(
        vec![ChatMessage { role: ChatRole::Assistant, content: "hi".into() }],
        5,
        true,
    );
    s.handle_llm_event(LlmEvent::Done { id: 5 });
    assert!(!s.chat.pending);
}

#[test]
fn error_before_tokens_removes_empty_placeholder_and_sets_status() {
    let mut s = chat_state_with(
        vec![
            ChatMessage { role: ChatRole::User, content: "q".into() },
            ChatMessage { role: ChatRole::Assistant, content: String::new() },
        ],
        5,
        true,
    );
    s.handle_llm_event(LlmEvent::Error { id: 5, message: "boom".into() });
    assert!(!s.chat.pending);
    assert_eq!(s.chat.messages.len(), 1); // empty assistant removed
    assert_eq!(s.chat.status.as_deref(), Some("boom"));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib app::state::tests::token_appends_to_last_assistant`
Expected: FAIL — compile error: `chat` field, `ChatState`, `handle_llm_event` don't exist.

- [ ] **Step 3: Add `ChatState`, the `chat` field, and methods**

> **Ordering note:** Do NOT add `Focus::Chat` here. It is added in Task 9 together with the `input.rs` match arms — adding it now would make `input.rs`'s exhaustive `match state.focus` blocks fail to compile.

Add the `ChatState` struct to `src/app/state.rs` (after the `Overlay` enum):

```rust
#[derive(Default)]
pub struct ChatState {
    pub visible: bool,
    pub messages: Vec<ChatMessage>,
    pub pending: bool,
    pub active_request: u64,
    pub scroll: usize,
    pub status: Option<String>,
    pub event_tx: Option<std::sync::mpsc::Sender<crate::app::llm::LlmEvent>>,
}
```

Add the field to `AppState` (after `panel_todos`):

```rust
    pub chat: ChatState,
```

In `open_day`, before the `Ok(Self { ... })`, load the sidecar:

```rust
        let chat_path = storage::chat_path_for(&notes_dir, date, &config.date_format);
        let chat_messages = storage::load_chat(&chat_path);
```

and add to the constructed `Self { ... }` (after `panel_todos,`):

```rust
            chat: ChatState {
                visible: config.chat_visible,
                messages: chat_messages,
                ..Default::default()
            },
```

Add these methods to `impl AppState` (after `update_context_display`):

```rust
    pub fn save_chat(&self) -> anyhow::Result<()> {
        let path = storage::chat_path_for(&self.notes_dir, self.date, &self.config.date_format);
        storage::save_chat(&path, &self.chat.messages)
    }

    pub fn handle_llm_event(&mut self, event: crate::app::llm::LlmEvent) {
        use crate::app::llm::LlmEvent;
        if event.id() != self.chat.active_request {
            return; // stale: superseded, cleared, or day switched
        }
        match event {
            LlmEvent::Started { .. } => {}
            LlmEvent::Token { text, .. } => {
                if let Some(last) = self.chat.messages.last_mut() {
                    if last.role == ChatRole::Assistant {
                        last.content.push_str(&text);
                    }
                }
            }
            LlmEvent::Done { .. } => {
                self.chat.pending = false;
                let _ = self.save_chat();
            }
            LlmEvent::Error { message, .. } => {
                self.chat.pending = false;
                if matches!(
                    self.chat.messages.last(),
                    Some(m) if m.role == ChatRole::Assistant && m.content.is_empty()
                ) {
                    self.chat.messages.pop();
                }
                self.chat.status = Some(message);
                let _ = self.save_chat();
            }
        }
    }
```

- [ ] **Step 4: Fix the test-only AppState literals that now miss the `chat` field**

In `src/ui/layout.rs`, the `test_app` helper builds `AppState { ... }` literally. Add this field after `panel_todos: Vec::new(),`:

```rust
            chat: crate::app::state::ChatState::default(),
```

In `src/ui/right_panel.rs`, three tests build `AppState { ... }` literally (`render_shows_current_month_header`, `render_shows_todo_text`, `render_selected_item_has_reversed_modifier`). In each, add after `panel_todos,` (or `panel_todos: Vec::new(),`):

```rust
            chat: crate::app::state::ChatState::default(),
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib`
Expected: PASS (state tests pass; layout and right_panel tests still compile and pass).

- [ ] **Step 6: Commit**

```bash
git add src/app/state.rs src/ui/layout.rs src/ui/right_panel.rs
git commit -m "feat: add ChatState, Focus::Chat, and handle_llm_event"
```

---

## Task 7: `/ask` and `/clear` commands (parse + dispatch)

**Files:**
- Modify: `src/app/command.rs`
- Modify: `src/app/actions.rs`

This task adds the `Command::Ask`/`Command::Clear` variants AND their `dispatch` arms together, so the crate compiles at the task boundary.

- [ ] **Step 1: Write failing parse tests**

Add to the `tests` module in `src/app/command.rs`:

```rust
#[test]
fn parse_ask_with_text() {
    assert_eq!(parse("/ask how are you"), Command::Ask("how are you".to_string()));
}

#[test]
fn parse_ask_empty_is_invalid() {
    assert_eq!(parse("/ask"), Command::InvalidArgs("/ask needs a message".to_string()));
}

#[test]
fn parse_clear() {
    assert_eq!(parse("/clear"), Command::Clear);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib app::command::tests::parse_clear`
Expected: FAIL — compile error: variants don't exist.

- [ ] **Step 3: Add the variants and parse rules**

In `src/app/command.rs`, add to the `Command` enum (after `Summarize,`):

```rust
    Ask(String),
    Clear,
```

In `parse`, add match arms (after the `"/summarize" => Command::Summarize,` arm):

```rust
        "/clear" => Command::Clear,
        "/ask" => {
            if rest.is_empty() {
                Command::InvalidArgs("/ask needs a message".to_string())
            } else {
                Command::Ask(rest.to_string())
            }
        }
```

- [ ] **Step 4: Write failing dispatch tests**

Add to the `tests` module in `src/app/actions.rs`:

```rust
#[test]
fn ask_pushes_user_and_placeholder_and_sets_pending() {
    use crate::app::llm::LlmEvent;
    use crate::app::state::ChatRole;
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    // Keep the receiver alive for the duration of the test so the spawned
    // worker's send (if any) is harmless. We assert only on state, not on the
    // channel, to avoid depending on network timing.
    let (tx, _rx) = std::sync::mpsc::channel::<LlmEvent>();
    state.chat.event_tx = Some(tx);

    dispatch(&mut state, Command::Ask("hello".to_string())).unwrap();

    assert_eq!(state.chat.messages.len(), 2);
    assert_eq!(state.chat.messages[0].role, ChatRole::User);
    assert_eq!(state.chat.messages[0].content, "hello");
    assert_eq!(state.chat.messages[1].role, ChatRole::Assistant);
    assert_eq!(state.chat.messages[1].content, "");
    assert!(state.chat.pending);
    assert!(state.chat.visible, "/ask should reveal the panel");
    assert!(state.chat.active_request > 0);
}

#[test]
fn ask_persists_user_message_to_sidecar() {
    use crate::app::llm::LlmEvent;
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    let (tx, _rx) = std::sync::mpsc::channel::<LlmEvent>();
    state.chat.event_tx = Some(tx);

    dispatch(&mut state, Command::Ask("persist me".to_string())).unwrap();

    let path = crate::storage::chat_path_for(&state.notes_dir, state.date, &state.config.date_format);
    let loaded = crate::storage::load_chat(&path);
    assert_eq!(loaded.first().map(|m| m.content.as_str()), Some("persist me"));
}

#[test]
fn clear_empties_messages_and_bumps_request() {
    use crate::app::state::{ChatMessage, ChatRole};
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.chat.messages = vec![ChatMessage { role: ChatRole::User, content: "x".into() }];
    state.chat.active_request = 1;
    state.chat.pending = true;

    dispatch(&mut state, Command::Clear).unwrap();

    assert!(state.chat.messages.is_empty());
    assert!(!state.chat.pending);
    assert!(state.chat.active_request > 1);
}
```

- [ ] **Step 5: Run to verify failure**

Run: `cargo test --lib app::actions::tests::clear_empties_messages_and_bumps_request`
Expected: FAIL — compile error: `dispatch` has no arm for `Ask`/`Clear` (non-exhaustive match).

- [ ] **Step 6: Add the dispatch arms**

In `src/app/actions.rs`, add to the `match cmd` in `dispatch` (after the `Command::Summarize => { ... }` arm):

```rust
        Command::Ask(text) => {
            let Some(tx) = state.chat.event_tx.clone() else {
                state.chat.status = Some("LLM channel unavailable".to_string());
                return Ok(());
            };
            state.chat.visible = true;
            state.chat.status = None;
            state.chat.scroll = 0;
            state.chat.messages.push(crate::app::state::ChatMessage {
                role: crate::app::state::ChatRole::User,
                content: text.clone(),
            });
            // Persist the user message before the reply streams in.
            let _ = state.save_chat();

            let request_messages = state.chat.messages.clone();
            let id = crate::app::llm::next_request_id();
            state.chat.active_request = id;
            state.chat.pending = true;
            // Empty assistant placeholder that tokens append into.
            state.chat.messages.push(crate::app::state::ChatMessage {
                role: crate::app::state::ChatRole::Assistant,
                content: String::new(),
            });

            let system = if state.config.llm_system_prompt.is_empty() {
                None
            } else {
                Some(state.config.llm_system_prompt.clone())
            };
            let req = crate::app::llm::ChatRequest {
                id,
                base_url: state.config.llm_base_url.clone(),
                model: state.config.llm_model.clone(),
                system,
                messages: request_messages,
            };
            crate::app::llm::spawn(req, tx);
        }
        Command::Clear => {
            state.chat.messages.clear();
            state.chat.active_request = crate::app::llm::next_request_id();
            state.chat.pending = false;
            state.chat.status = None;
            state.chat.scroll = 0;
            let _ = state.save_chat();
        }
```

- [ ] **Step 7: Run to verify pass**

Run: `cargo test --lib app::command::tests app::actions::tests`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/app/command.rs src/app/actions.rs
git commit -m "feat: add /ask and /clear commands"
```

---

## Task 8: Preserve chat sender + load sidecar across day switches

**Files:**
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Write a failing test**

Add to the `tests` module in `src/app/actions.rs`:

```rust
#[test]
fn go_to_date_preserves_sender_and_loads_target_chat() {
    use crate::app::llm::LlmEvent;
    use crate::app::state::{ChatMessage, ChatRole};
    let tmp = tempfile::tempdir().unwrap();
    let config = Config::default();
    let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let other = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();

    // Pre-write a chat sidecar for the *other* day.
    let other_chat = crate::storage::chat_path_for(tmp.path(), other, &config.date_format);
    crate::storage::save_chat(
        &other_chat,
        &[ChatMessage { role: ChatRole::Assistant, content: "from other day".into() }],
    )
    .unwrap();

    let mut state = AppState::open_day(tmp.path().to_path_buf(), config, today).unwrap();
    let (tx, _rx) = std::sync::mpsc::channel::<LlmEvent>();
    state.chat.event_tx = Some(tx);

    go_to_date(&mut state, other).unwrap();

    assert!(state.chat.event_tx.is_some(), "sender must survive day switch");
    assert_eq!(state.chat.messages.len(), 1);
    assert_eq!(state.chat.messages[0].content, "from other day");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib app::actions::tests::go_to_date_preserves_sender_and_loads_target_chat`
Expected: FAIL — `event_tx` is `None` after the switch (open_day resets it).

- [ ] **Step 3: Update `go_to_date`**

In `src/app/actions.rs`, replace the body of `go_to_date` with:

```rust
pub fn go_to_date(state: &mut AppState, date: chrono::NaiveDate) -> anyhow::Result<()> {
    state.save()?;
    state.save_chat()?;
    let tx = state.chat.event_tx.take();
    let notes_dir = state.notes_dir.clone();
    let config = state.config.clone();
    *state = AppState::open_day(notes_dir, config, date)?;
    state.chat.event_tx = tx;
    state.save()?;
    state.dates_with_notes =
        crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
    Ok(())
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib app::actions::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat: preserve chat sender and load sidecar on day switch"
```

---

## Task 9: Input — Ctrl-L toggle, Focus::Chat cycle and scrolling

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/app/input.rs`:

```rust
#[test]
fn ctrl_l_toggles_chat() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(&tmp);
    assert_eq!(
        key_to_action(&state, ctrl(KeyCode::Char('l'))),
        Some(UiAction::ToggleChat)
    );
}

#[test]
fn toggle_chat_flips_visibility() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    let before = state.chat.visible;
    execute_action(&mut state, UiAction::ToggleChat).unwrap();
    assert_eq!(state.chat.visible, !before);
}

#[test]
fn toggle_chat_off_while_focused_returns_to_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.chat.visible = true;
    state.focus = Focus::Chat;
    execute_action(&mut state, UiAction::ToggleChat).unwrap(); // turns off
    assert!(!state.chat.visible);
    assert_eq!(state.focus, Focus::Capture);
}

#[test]
fn tab_from_navigate_focuses_chat_when_visible() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Navigate;
    state.chat.visible = true;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::FocusChat)
    );
}

#[test]
fn tab_from_navigate_skips_chat_when_hidden() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Navigate;
    state.chat.visible = false;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::FocusRightPanel)
    );
}

#[test]
fn tab_from_chat_goes_to_right_panel() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Chat;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::FocusRightPanel)
    );
}

#[test]
fn esc_in_chat_blurs_to_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Chat;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Esc)),
        Some(UiAction::ChatBlur)
    );
}

#[test]
fn chat_scroll_keys_map() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Chat;
    assert_eq!(key_to_action(&state, make_key(KeyCode::Char('k'))), Some(UiAction::ChatScrollUp));
    assert_eq!(key_to_action(&state, make_key(KeyCode::Char('j'))), Some(UiAction::ChatScrollDown));
}

#[test]
fn chat_scroll_down_saturates_at_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.chat.scroll = 0;
    execute_action(&mut state, UiAction::ChatScrollDown).unwrap();
    assert_eq!(state.chat.scroll, 0);
}

#[test]
fn chat_scroll_up_increments() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.chat.scroll = 0;
    execute_action(&mut state, UiAction::ChatScrollUp).unwrap();
    assert_eq!(state.chat.scroll, 1);
}

#[test]
fn focus_chat_sets_focus() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    execute_action(&mut state, UiAction::FocusChat).unwrap();
    assert_eq!(state.focus, Focus::Chat);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib app::input::tests::ctrl_l_toggles_chat`
Expected: FAIL — compile error: actions/variants don't exist.

- [ ] **Step 3: Add the `Focus::Chat` enum variant**

In `src/app/state.rs`, add `Chat` to the `Focus` enum (this is the deferred addition from Task 6; the match arms below make it compile):

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Capture,
    Navigate,
    RightPanel,
    Chat,
}
```

- [ ] **Step 3b: Add the new `UiAction` variants**

In `src/app/input.rs`, add to the `UiAction` enum (after the `// Right panel` group):

```rust
    // Chat panel
    ToggleChat,
    FocusChat,
    ChatBlur,
    ChatScrollUp,
    ChatScrollDown,
    ChatPageUp,
    ChatPageDown,
```

- [ ] **Step 4: Add the `Ctrl-L` hotkey**

In `key_to_action`, in the global Ctrl block (step "3."), add the `'l'` arm:

```rust
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('t') => return Some(UiAction::GoToday),
            KeyCode::Char('l') => return Some(UiAction::ToggleChat),
            _ => {}
        }
    }
```

- [ ] **Step 5: Update the Tab cycle and Esc handling for `Focus::Chat`**

Replace the Tab block (step "4.") with:

```rust
    if key.code == KeyCode::Tab {
        return match state.focus {
            Focus::Capture => Some(UiAction::TypeIndent),
            Focus::Navigate => {
                if state.chat.visible {
                    Some(UiAction::FocusChat)
                } else {
                    Some(UiAction::FocusRightPanel)
                }
            }
            Focus::Chat => Some(UiAction::FocusRightPanel),
            Focus::RightPanel => Some(UiAction::RightPanelBlur),
        };
    }
```

Add a `Focus::Chat` arm to the Esc block (step "5."):

```rust
            Focus::RightPanel => Some(UiAction::RightPanelBlur),
            Focus::Chat => Some(UiAction::ChatBlur),
```

- [ ] **Step 6: Add the mode-specific `Focus::Chat` arm**

In the final `match state.focus { ... }` (step "7."), add a `Focus::Chat` arm (alongside `Focus::RightPanel`):

```rust
        Focus::Chat => match key.code {
            KeyCode::Down | KeyCode::Char('j') => Some(UiAction::ChatScrollDown),
            KeyCode::Up | KeyCode::Char('k') => Some(UiAction::ChatScrollUp),
            KeyCode::PageDown => Some(UiAction::ChatPageDown),
            KeyCode::PageUp => Some(UiAction::ChatPageUp),
            _ => None,
        },
```

- [ ] **Step 7: Add `execute_action` handlers**

In `execute_action`, add arms (after the `// Right panel` group, before the closing of the match):

```rust
        // Chat panel
        UiAction::ToggleChat => {
            state.chat.visible = !state.chat.visible;
            if !state.chat.visible && state.focus == Focus::Chat {
                state.focus = Focus::Capture;
            }
        }
        UiAction::FocusChat => {
            state.focus = Focus::Chat;
        }
        UiAction::ChatBlur => {
            state.focus = Focus::Capture;
        }
        UiAction::ChatScrollUp => {
            state.chat.scroll += 1;
        }
        UiAction::ChatScrollDown => {
            state.chat.scroll = state.chat.scroll.saturating_sub(1);
        }
        UiAction::ChatPageUp => {
            state.chat.scroll += 10;
        }
        UiAction::ChatPageDown => {
            state.chat.scroll = state.chat.scroll.saturating_sub(10);
        }
```

- [ ] **Step 8: Run to verify pass**

Run: `cargo test --lib app::input::tests`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: chat panel toggle, focus cycle, and scrolling"
```

---

## Task 10: Chat panel rendering

**Files:**
- Create: `src/ui/chat_panel.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Register the module**

In `src/ui/mod.rs`, add `mod chat_panel;` (alongside the other `mod` lines):

```rust
mod calendar;
mod capture;
mod chat_panel;
mod document;
pub mod help;
pub mod layout;
pub mod right_panel;
```

- [ ] **Step 2: Create the module with `wrap_line` + a failing wrap test**

Create `src/ui/chat_panel.rs` (no top-level imports yet — they're added with `render` in Step 4 to avoid unused-import warnings):

```rust
/// Wrap `text` to `width` columns, splitting on spaces and honoring existing newlines.
fn wrap_line(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let mut current = String::new();
        for word in raw.split(' ') {
            if current.is_empty() {
                current = word.to_string();
            } else if current.chars().count() + 1 + word.chars().count() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                out.push(std::mem::take(&mut current));
                current = word.to_string();
            }
        }
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_breaks_on_width() {
        let lines = wrap_line("one two three", 7);
        assert_eq!(lines, vec!["one two".to_string(), "three".to_string()]);
    }

    #[test]
    fn wrap_preserves_newlines() {
        let lines = wrap_line("a\nb", 10);
        assert_eq!(lines, vec!["a".to_string(), "b".to_string()]);
    }
}
```

- [ ] **Step 3: Run to verify the wrap test passes**

Run: `cargo test --lib ui::chat_panel::tests::wrap_breaks_on_width`
Expected: PASS.

- [ ] **Step 4: Add imports, the `PANEL_BG` const, and the `render` function**

At the **top** of `src/ui/chat_panel.rs`, add:

```rust
use crate::app::state::{AppState, ChatRole, Focus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Padding, Paragraph};

const PANEL_BG: Color = Color::Rgb(230, 230, 240);
```

Then append, before the `#[cfg(test)]` module:

```rust
pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState) {
    let bg = Block::default()
        .style(Style::default().bg(PANEL_BG))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = bg.inner(area);
    frame.render_widget(bg, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(0),    // body
            Constraint::Length(1), // status
        ])
        .split(inner);

    let header = Paragraph::new("Chat").style(
        Style::default()
            .bg(PANEL_BG)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(header, chunks[0]);

    let body_area = chunks[1];
    let width = body_area.width as usize;

    let mut lines: Vec<Line> = Vec::new();
    for msg in &app.chat.messages {
        let (prefix, style) = match msg.role {
            ChatRole::User => ("You: ", Style::default().add_modifier(Modifier::DIM)),
            ChatRole::Assistant => ("AI:  ", Style::default()),
        };
        let full = format!("{}{}", prefix, msg.content);
        for wl in wrap_line(&full, width) {
            lines.push(Line::styled(wl, style.bg(PANEL_BG)));
        }
    }
    // Thinking indicator while waiting for the first token.
    if app.chat.pending
        && matches!(
            app.chat.messages.last(),
            Some(m) if m.role == ChatRole::Assistant && m.content.is_empty()
        )
    {
        lines.push(Line::styled("…", Style::default().bg(PANEL_BG)));
    }

    let height = body_area.height as usize;
    let total = lines.len();
    let max_top = total.saturating_sub(height);
    let scroll = if app.focus == Focus::Chat { app.chat.scroll } else { 0 };
    let top = max_top.saturating_sub(scroll);
    let end = (top + height).min(total);
    let visible: Vec<Line> = if top < end { lines[top..end].to_vec() } else { Vec::new() };
    frame.render_widget(
        Paragraph::new(visible).style(Style::default().bg(PANEL_BG)),
        body_area,
    );

    if let Some(status) = &app.chat.status {
        let status_widget = Paragraph::new(status.clone())
            .style(Style::default().bg(PANEL_BG).fg(Color::Red));
        frame.render_widget(status_widget, chunks[2]);
    }
}
```

- [ ] **Step 5: Add render tests**

Add to the `tests` module in `src/ui/chat_panel.rs`:

```rust
use crate::app::state::{AppState, ChatMessage, ChatRole};
use crate::config::Config;
use chrono::NaiveDate;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn app_with_messages(messages: Vec<ChatMessage>) -> AppState {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = AppState::open_day(
        tmp.path().to_path_buf(),
        Config::default(),
        NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
    )
    .unwrap();
    s.chat.visible = true;
    s.chat.messages = messages;
    s
}

#[test]
fn render_shows_message_text() {
    let app = app_with_messages(vec![
        ChatMessage { role: ChatRole::User, content: "ping".into() },
        ChatMessage { role: ChatRole::Assistant, content: "pong".into() },
    ]);
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, f.area(), &app)).unwrap();
    let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("ping"), "got: {}", content);
    assert!(content.contains("pong"), "got: {}", content);
    assert!(content.contains("Chat"), "header missing: {}", content);
}

#[test]
fn render_shows_thinking_indicator() {
    let mut app = app_with_messages(vec![
        ChatMessage { role: ChatRole::User, content: "q".into() },
        ChatMessage { role: ChatRole::Assistant, content: String::new() },
    ]);
    app.chat.pending = true;
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, f.area(), &app)).unwrap();
    let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains('…'), "thinking indicator missing: {}", content);
}
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --lib ui::chat_panel::tests`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/ui/mod.rs src/ui/chat_panel.rs
git commit -m "feat: render chat panel with wrapping, thinking indicator, status"
```

---

## Task 11: Layout — conditional three-column split

**Files:**
- Modify: `src/ui/layout.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/ui/layout.rs`:

```rust
#[test]
fn chat_panel_renders_when_visible() {
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::Capture, 0);
    app.chat.visible = true;
    app.chat.messages = vec![crate::app::state::ChatMessage {
        role: crate::app::state::ChatRole::Assistant,
        content: "paneltext".to_string(),
    }];

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("paneltext"), "chat text missing: {}", content);
}

#[test]
fn chat_panel_hidden_does_not_render_chat() {
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::Capture, 0);
    app.chat.visible = false;
    app.chat.messages = vec![crate::app::state::ChatMessage {
        role: crate::app::state::ChatRole::Assistant,
        content: "paneltext".to_string(),
    }];

    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
    assert!(!content.contains("paneltext"), "chat should be hidden: {}", content);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib ui::layout::tests::chat_panel_renders_when_visible`
Expected: FAIL — chat text not rendered (no chat column yet).

- [ ] **Step 3: Make the outer split conditional and render the chat panel**

In `src/ui/layout.rs`, replace the outer-split block:

```rust
    // Outer horizontal split: left = doc+chrome, right = panel
    let panel_width = app.config.panel_width;
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(panel_width)])
        .split(frame.area());

    let left_area = outer[0];
    let panel_area = outer[1];
```

with:

```rust
    // Outer horizontal split: left = doc+chrome, [chat], right = panel
    let panel_width = app.config.panel_width;
    let (left_area, chat_area, panel_area) = if app.chat.visible {
        let outer = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(app.config.chat_width),
                Constraint::Length(panel_width),
            ])
            .split(frame.area());
        (outer[0], Some(outer[1]), outer[2])
    } else {
        let outer = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(panel_width)])
            .split(frame.area());
        (outer[0], None, outer[1])
    };
```

Then, just before the `// Right panel` render call (`super::right_panel::render(frame, panel_area, app);`), add:

```rust
    // Chat panel (middle column), when visible
    if let Some(chat_area) = chat_area {
        super::chat_panel::render(frame, chat_area, app);
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib ui::layout::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ui/layout.rs
git commit -m "feat: insert toggleable chat column into layout"
```

---

## Task 12: Wire the LLM channel into the event loop

**Files:**
- Modify: `src/main.rs`

This is the runtime glue. It has no unit test (it owns terminal + threads); verify by build + manual run.

- [ ] **Step 1: Create the channel, store the sender, and poll events**

In `src/main.rs`, inside `run()`, after `let mut app = ... .context("Failed to open day")?;` and before `let mut terminal = ratatui::init();`, add:

```rust
    // LLM event channel: worker threads send LlmEvents; the loop polls them.
    let (llm_tx, llm_rx) = std::sync::mpsc::channel::<buff::app::llm::LlmEvent>();
    app.chat.event_tx = Some(llm_tx);
```

Then, inside the `loop { ... }`, after the `terminal.draw(...)?;` call and before `if let Some(key) = read_key()? {`, add:

```rust
        // Drain any LLM events that arrived since the last iteration.
        while let Ok(event) = llm_rx.try_recv() {
            app.handle_llm_event(event);
        }
```

- [ ] **Step 2: Verify it builds**

Run: `cargo build`
Expected: compiles successfully.

- [ ] **Step 3: Manual smoke test (requires LM Studio on port 1234)**

Start LM Studio serving `google/gemma-4-12b-qat` on port 1234, then run:

```bash
cargo run -- --notes-dir /tmp/buff-chat-test
```

Verify, in order:
1. The chat panel is visible by default between the document and the right panel.
2. Typing `/ask hello` in the capture box shows `You: hello` and a `…` indicator, then streams an `AI:` reply token-by-token.
3. `Ctrl-L` hides/shows the chat panel.
4. `Esc` then `Tab` reaches the chat panel; `j`/`k` scroll the history; `Esc` returns to the capture box.
5. `/clear` empties the conversation.
6. A `2026-…-….chat.json` sidecar exists in `/tmp/buff-chat-test`; quitting and reopening the same day restores the conversation.
7. Stop LM Studio, run `/ask hi` — a red error line appears in the panel and the app stays responsive (no freeze; `Ctrl-C` still quits).

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire LLM event channel into the event loop"
```

---

## Task 13: Documentation — help overlay and README

**Files:**
- Modify: `src/ui/help.rs`
- Modify: `README.md`

- [ ] **Step 1: Add chat entries to the help overlay**

In `src/ui/help.rs`, bump the popup height so the added lines fit — change `let popup_height = 20;` to:

```rust
    let popup_height = 24;
```

Then add two command lines to the `Commands:` block of the `help_text` literal. Change:

```rust
  /goto YYYY-MM-DD  jump to date
  /today, Ctrl-T   jump to today
```

to:

```rust
  /goto YYYY-MM-DD  jump to date
  /today, Ctrl-T   jump to today
  /ask message     ask the local LLM (streams to chat)
  /clear           clear the chat conversation

Chat panel:
  Ctrl-L           show/hide chat panel
  Tab              focus chat (then j/k to scroll)
```

- [ ] **Step 2: Lock the new help content with a test**

The help-overlay render test lives in `src/ui/layout.rs` (`render_help_overlay`, which asserts `/meeting` is present). Add an assertion to it that the buffer also contains `/ask`:

```rust
        assert!(
            content.contains("/ask"),
            "Expected '/ask' in help buffer, got: {}",
            content
        );
```

Run: `cargo test --lib ui::layout::tests::render_help_overlay`
Expected: PASS.

- [ ] **Step 3: Update the README**

In `README.md`:
- Add `/ask "..."`/`/ask <message>` and `/clear` rows to the slash-command table.
- Add a "### Chat panel" section describing the middle panel, the `Ctrl-L` toggle, focus/scroll keys, and that it talks to a local LM Studio server.
- Add the new config keys to the `config.toml` example block:

```toml
llm_base_url = "http://localhost:1234/v1"   # OpenAI-compatible local server
llm_model = "google/gemma-4-12b-qat"        # model id served by LM Studio
llm_system_prompt = ""                       # optional system prompt
chat_width = 40                              # chat panel width in columns
chat_visible = true                          # show the chat panel on startup
```

- Move "Local LLM integration" out of "Future features" (Phase 1 chat is now implemented; note Phase 2 assistant prompts remain pending).

- [ ] **Step 4: Run the test suite and build**

Run: `cargo test && cargo build`
Expected: PASS / compiles.

- [ ] **Step 5: Commit**

```bash
git add src/ui/help.rs README.md
git commit -m "docs: document /ask, /clear, Ctrl-L, and chat config"
```

---

## Final verification

- [ ] **Run the full test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Run clippy and formatting (if used by the project)**

Run: `cargo clippy --all-targets && cargo fmt --check`
Expected: no errors (fix warnings introduced by this work).

- [ ] **Manual end-to-end** per Task 12 Step 3 against a live LM Studio.

---

## Notes & known Phase-1 simplifications

- **Concurrent `/ask`:** sending a new `/ask` while a reply streams orphans the in-flight reply (its tokens are dropped by id mismatch); the prior partial assistant message stays in history. Accepted for Phase 1.
- **Scroll model:** `chat.scroll` is "lines up from the bottom"; render clamps it to the available range. Scrolling up past the top is a no-op visually but the counter can grow; it resets to 0 on each `/ask` and `/clear`. Good enough for Phase 1.
- **`ureq` pinned at 2.x** for its blocking `into_reader()` streaming API; do not bump to 3.x in this work without revisiting `spawn`.
- **No network in unit tests:** only `parse_sse_line`/`build_body` are tested for the worker; the live HTTP path is covered by the manual smoke test.
