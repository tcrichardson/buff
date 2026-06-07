# Design: Local LLM Chat Panel — Phase 1 (Basic Chat)

**Date:** 2026-06-07
**Status:** Approved
**Context:** First of two phases adding local-LLM support to buff. Phase 1 delivers a working chat panel that talks to LM Studio. Phase 2 (separate spec) turns it into a note-taking assistant with custom Meeting/Note/To-Do prompts. This design supersedes the reverted 2026-06-06 "LLM scaffolding" placeholder commit — it adopts that spec's non-blocking architecture but builds real, behavior-bearing code instead of empty placeholder enums.

---

## Goals

A toggleable middle "Chat" panel sits between the document and the right panel. The user converses with a local LM Studio server by typing `/ask <message>` in the existing bottom capture box; replies stream in token-by-token. Conversations are multi-turn and persist per-day as a sidecar JSON file. `/clear` resets the current day's conversation.

### In scope
- Three-column layout with a toggleable chat panel (`Ctrl-L`), visible by default.
- `/ask <message>` and `/clear` commands, dispatched from the existing capture box.
- Streaming (SSE) worker thread talking to LM Studio's OpenAI-compatible API.
- Per-day conversation persistence as a sidecar JSON file.
- Config for base URL, model, system prompt, panel width, default visibility.
- Non-fatal error handling surfaced in the panel.

### Out of scope (Phase 2)
- Custom Meeting/Note/To-Do prompt modes.
- Injecting AI output into the note document.
- Cross-day or global conversation history.

---

## Decisions (from brainstorming)

| Decision | Choice |
|----------|--------|
| LLM server | LM Studio, OpenAI-compatible `POST {base}/v1/chat/completions` |
| Input mechanism | Reuse the bottom capture box via a `/ask` slash command (no dedicated chat input) |
| Response mode | Streaming, token-by-token (SSE) |
| Panel visibility | Toggleable (`Ctrl-L`), visible by default; width configurable |
| Persistence | Per-day sidecar JSON file alongside the note |
| HTTP client | `ureq` (blocking, lean) + `serde_json` |
| Concurrency | Thread-per-request + a single `LlmEvent` channel polled by the event loop; cancellation via monotonic `request_id` |

---

## Architecture

The app is a synchronous Ratatui TUI: `run()` in `src/main.rs` polls keys every 100ms, draws, and routes actions into a pure core. There is no async runtime, and we are not adding one.

**Non-blocking streaming:** each `/ask` spawns a short-lived worker thread that performs the blocking `ureq` call and reads the SSE stream, sending `LlmEvent`s back over an `mpsc` channel. The event loop polls that channel via `try_recv()` once per iteration; arriving tokens update state and the next draw shows them. Because the loop already wakes ~10×/second, streaming text appears smoothly without any new timer.

**Request identity / cancellation:** every request gets a unique id from a process-global `AtomicU64` in `llm.rs`. `ChatState.active_request` records the id whose events we currently accept. Superseding a request, `/clear`, or switching days bumps/replaces `active_request`, so a still-running worker's later events are silently dropped (`event.id != active_request`). A global atomic is used because `AppState` is fully replaced on day switch (see below), so a counter stored in state could not stay monotonic.

---

## File map

| Action | File | What changes |
|--------|------|-------------|
| Modify | `Cargo.toml` | Add `ureq`, `serde_json` deps |
| Create | `src/app/llm.rs` | `LlmEvent`, `ChatRequest`, `spawn`, pure SSE-line parser, request-id atomic |
| Modify | `src/app/mod.rs` | `pub mod llm;` |
| Modify | `src/app/state.rs` | `ChatRole`, `ChatMessage`, `ChatState`; add `Focus::Chat`; `chat` field on `AppState`; `handle_llm_event`; load chat in `open_day` |
| Modify | `src/app/command.rs` | `Command::Ask(String)`, `Command::Clear`; parse rules |
| Modify | `src/app/actions.rs` | Dispatch `Ask`/`Clear`; preserve `event_tx` + load sidecar in `go_to_date` |
| Modify | `src/app/input.rs` | `Ctrl-L` → `ToggleChat`; `Focus::Chat` in Tab cycle + scroll keys |
| Modify | `src/storage.rs` | `chat_path_for`, `load_chat`, `save_chat` |
| Modify | `src/config.rs` | `llm_base_url`, `llm_model`, `llm_system_prompt`, `chat_width`, `chat_visible` |
| Create | `src/ui/chat_panel.rs` | Render conversation + status/thinking indicator |
| Modify | `src/ui/mod.rs` | `mod chat_panel;` |
| Modify | `src/ui/layout.rs` | Conditional three-column split |
| Modify | `src/ui/help.rs`, `README.md` | Document `/ask`, `/clear`, `Ctrl-L`, config keys |
| Modify | `src/main.rs` | Create `LlmEvent` channel, store `Sender` in `app.chat.event_tx`, poll `try_recv()` |

---

## Section 1: Layout & rendering

### `src/ui/layout.rs`

The outer horizontal split becomes conditional on `app.chat.visible`:

- **Visible:** `[Constraint::Min(0) /*doc*/, Constraint::Length(chat_width) /*chat*/, Constraint::Length(panel_width) /*right*/]`
- **Hidden:** `[Constraint::Min(0), Constraint::Length(panel_width)]` — byte-for-byte today's behavior.

```
Visible:                                Hidden (unchanged):
+----------------+----------+--------+   +-------------------------+--------+
| # date / Notes |  Chat    |June2026|   | # date / Notes          |June2026|
|  notes...      |  You: hi |calendar|   |  notes...               |calendar|
|  status        |  AI: ... |To-dos  |   |  status                 |To-dos  |
| > /ask how...  |          |        |   | > capture box           |        |
+----------------+----------+--------+   +-------------------------+--------+
```

The chat and right panels are both **full-height outer columns** (parallel to today's right panel). The capture box is part of the left column's internal vertical stack (title / document / status / input) and therefore sits only under the document column — it does **not** span beneath the chat or right panels. This makes the layout change minimally invasive: we only insert a third constraint into the existing outer horizontal split and call `chat_panel::render` for it.

### `src/ui/chat_panel.rs` (new, mirrors `right_panel.rs`)

- Renders `app.chat.messages` as wrapped lines with `You:` / `AI:` prefixes, styled distinctly (e.g. user dim, assistant normal). Word-wrap to the panel's inner width.
- Auto-scroll: when not focused, pin to the bottom (latest content). When `Focus::Chat`, honor `chat.scroll`.
- While `chat.pending` and the last assistant message is still empty, show a `…` / "thinking" indicator.
- Bottom line shows `chat.status` (error/info) when set.
- A title/header line (`Chat`) consistent with the right panel's styling.

---

## Section 2: State & data model

### `src/app/state.rs`

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole { User, Assistant }

#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

pub struct ChatState {
    pub visible: bool,                 // seeded from config.chat_visible
    pub messages: Vec<ChatMessage>,
    pub pending: bool,                 // a request is in flight
    pub active_request: u64,           // id of the request whose tokens we accept
    pub scroll: usize,                 // scrollback offset for Focus::Chat
    pub status: Option<String>,        // error/info line for the panel
    pub event_tx: Option<std::sync::mpsc::Sender<crate::app::llm::LlmEvent>>,
}
```

`Focus` gains a `Chat` variant. `AppState` gains `pub chat: ChatState`.

`open_day` constructs `ChatState` with `visible: config.chat_visible`, `messages: storage::load_chat(chat_path)`, `pending: false`, `active_request: 0`, `scroll: 0`, `status: None`, `event_tx: None`. The `Sender` is injected later by `run()` (and preserved across day switches by `go_to_date`).

### `handle_llm_event`

```rust
pub fn handle_llm_event(&mut self, event: LlmEvent) {
    let id = event.id();
    if id != self.chat.active_request { return; }  // stale: superseded / cleared / day-switched
    match event {
        LlmEvent::Started { .. } => { /* pending already true; placeholder already pushed */ }
        LlmEvent::Token { text, .. } => {
            if let Some(last) = self.chat.messages.last_mut() {
                if last.role == ChatRole::Assistant { last.content.push_str(&text); }
            }
        }
        LlmEvent::Done { .. } => {
            self.chat.pending = false;
            let _ = self.save_chat();
        }
        LlmEvent::Error { message, .. } => {
            self.chat.pending = false;
            // remove the empty assistant placeholder if no tokens arrived
            if matches!(self.chat.messages.last(), Some(m)
                if m.role == ChatRole::Assistant && m.content.is_empty()) {
                self.chat.messages.pop();
            }
            self.chat.status = Some(message);
            let _ = self.save_chat();
        }
    }
}
```

`save_chat` is a small helper: `storage::save_chat(&chat_path_for(...), &self.chat.messages)`.

---

## Section 3: Concurrency & LLM worker

### `src/app/llm.rs`

```rust
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
pub fn next_request_id() -> u64 { NEXT_ID.fetch_add(1, Ordering::Relaxed) }

pub enum LlmEvent {
    Started { id: u64 },
    Token   { id: u64, text: String },
    Done    { id: u64 },
    Error   { id: u64, message: String },
}
impl LlmEvent { pub fn id(&self) -> u64 { /* match */ } }

pub struct ChatRequest {
    pub id: u64,
    pub base_url: String,            // e.g. "http://localhost:1234/v1"
    pub model: String,
    pub system: Option<String>,      // None or empty → omit system message
    pub messages: Vec<crate::app::state::ChatMessage>,
}

/// Spawn a worker thread that streams a chat completion and emits LlmEvents.
pub fn spawn(req: ChatRequest, tx: std::sync::mpsc::Sender<LlmEvent>) { /* thread::spawn */ }

/// Pure: parse a single SSE line into an optional content delta or a DONE sentinel.
/// Unit-tested without any network.
pub enum SseLine { Delta(String), Done, Ignore }
pub fn parse_sse_line(line: &str) -> SseLine { /* "data: {json}" | "data: [DONE]" | other */ }
```

Worker body:
1. Send `Started { id }`.
2. Build request body: `{ "model": model, "messages": [ (optional system), ...user/assistant turns ], "stream": true }` via `serde_json`.
3. `ureq::post("{base_url}/chat/completions").send_json(body)`. On transport error or non-2xx → `Error { id, message }` and return.
4. Wrap the response reader in `BufReader`; iterate `lines()`. For each, `parse_sse_line`:
   - `Delta(text)` → `Token { id, text }`
   - `Done` → `Done { id }`, return
   - `Ignore` → skip (blank lines, comments, unparseable chunks)
5. If the stream ends without `[DONE]`, emit `Done { id }`.

The `Sender` may be disconnected if the app is quitting; `send` errors are ignored.

### `src/main.rs`

```rust
let (llm_tx, llm_rx) = std::sync::mpsc::channel::<buff::app::llm::LlmEvent>();
app.chat.event_tx = Some(llm_tx);
// ... in the loop, before/after read_key:
while let Ok(event) = llm_rx.try_recv() {
    app.handle_llm_event(event);
}
```

(`while` drains any burst of tokens that accumulated between iterations in one pass.)

---

## Section 4: Commands & input flow

### `src/app/command.rs`

Add variants `Ask(String)` and `Clear`. Parse:
- `/ask <message>` → `Ask(message)`; empty → `InvalidArgs("/ask needs a message")`.
- `/clear` → `Clear`.

### `src/app/actions.rs::dispatch`

- `Command::Ask(text)`:
  - If `state.chat.event_tx.is_none()` → `state.chat.status = Some("LLM channel unavailable")` (defensive; never happens at runtime).
  - Else: if `!state.chat.visible` set it `true` (auto-reveal); push `ChatMessage { User, text }`; `let id = llm::next_request_id(); state.chat.active_request = id; state.chat.pending = true; state.chat.status = None;` push empty `ChatMessage { Assistant, "" }` placeholder; build `ChatRequest` from `config` + `state.chat.messages` (excluding the empty placeholder); `llm::spawn(req, tx.clone())`; `state.save_chat()`.
  - Does **not** modify the note `Document`.
- `Command::Clear`:
  - `state.chat.messages.clear(); state.chat.active_request = llm::next_request_id(); state.chat.pending = false; state.chat.status = None;` save sidecar (empty → file removed).

### `src/app/actions.rs::go_to_date`

`go_to_date` replaces the whole `AppState`. To survive that:
```rust
pub fn go_to_date(state: &mut AppState, date: NaiveDate) -> anyhow::Result<()> {
    state.save()?;
    state.save_chat()?;                       // persist current day's chat
    let tx = state.chat.event_tx.take();      // carry the sender over
    let notes_dir = state.notes_dir.clone();
    let config = state.config.clone();
    *state = AppState::open_day(notes_dir, config, date)?;  // loads new day's sidecar
    state.chat.event_tx = tx;                 // re-inject sender
    state.save()?;
    state.dates_with_notes = ...;
    Ok(())
}
```
Any in-flight worker keeps streaming, but its events are dropped because the new state's `active_request` is `0` (fresh) and won't match the old id.

### `src/app/input.rs`

- **Ctrl-L** added to the global Ctrl hotkey block → `UiAction::ToggleChat`. `execute_action` flips `state.chat.visible`; if turning off while `focus == Chat`, set `focus = Capture`.
- **Tab cycle** updated. When `chat.visible`: `Navigate --Tab--> Chat`, `Chat --Tab--> RightPanel`, `RightPanel --Tab--> Capture`. When hidden: unchanged (`Navigate --Tab--> RightPanel`). Capture's Tab still inserts the `->` indent marker.
- **`Focus::Chat` keys:** `j`/`Down` and `k`/`Up` adjust `chat.scroll`; PgDn/PgUp page; `Esc` → `Capture`. New actions `ChatScrollUp` / `ChatScrollDown` (and page variants) plus `FocusChat` / `ChatBlur`.

### Help & README

Document `/ask`, `/clear`, the `Ctrl-L` toggle, the chat-panel focus/scroll keys, and the new config keys.

---

## Section 5: Persistence (sidecar JSON)

### `src/storage.rs`

- `chat_path_for(notes_dir, date, date_format) -> PathBuf`: the note's path stem + `.chat.json` (e.g. `2026-06-04-Thu.chat.json`).
- `load_chat(path) -> Vec<ChatMessage>`: missing file → `vec![]`; unreadable/malformed JSON → `vec![]` (non-fatal; never blocks opening a day).
- `save_chat(path, &[ChatMessage])`: if empty, remove the file if present; else atomic temp-write + rename (matching the note save pattern). Pretty JSON for hand-inspectability.

`dates_with_notes` and `collect_panel_todos` must ignore `.chat.json` files — verify the existing date-stem filter doesn't treat them as note days (no spurious calendar dots).

---

## Section 6: Config additions

### `src/config.rs`

New fields on `Config` (all optional in `config.toml`, `#[serde(default)]` already in place):

```rust
pub llm_base_url: String,        // default "http://localhost:1234/v1"
pub llm_model: String,           // default "google/gemma-4-12b-qat"
pub llm_system_prompt: String,   // default "" (empty → omit system message)
pub chat_width: u16,             // default 40
pub chat_visible: bool,          // default true
```

`config.toml` example:
```toml
llm_base_url = "http://localhost:1234/v1"
llm_model    = "google/gemma-4-12b-qat"
llm_system_prompt = ""
chat_width   = 40
chat_visible = true
```

---

## Section 7: Error handling

All failures are non-fatal and surface as `chat.status` in the panel:
- Connection refused / server down → `"can't reach LLM at <base_url>"`.
- Non-2xx HTTP → include the status code (and body snippet if small).
- Malformed individual SSE chunk → skipped (`SseLine::Ignore`); only a hard stream/transport error aborts with `Error`.
- An `Error` arriving before any token removes the empty `Assistant` placeholder so no blank `AI:` bubble lingers.
- `Sender` disconnected (app quitting) → worker's `send` errors are ignored; thread exits.

---

## Section 8: Testing strategy

Follows the project's pure-core / thin-TUI split (no network in tests).

**Pure core (unit):**
- `command.rs`: `/ask` with text, `/ask` empty (→ `InvalidArgs`), `/clear`.
- `actions.rs`: `Ask` pushes user + empty assistant placeholder, sets `pending`, bumps `active_request`, auto-reveals panel (inject a dummy `Sender`, assert the request was sent — drain the paired receiver); `Clear` empties messages + bumps id; `go_to_date` preserves `event_tx` and loads the target day's sidecar.
- `state.rs::handle_llm_event`: token append to last assistant message; stale-id event dropped; `Done` clears `pending`; `Error` before tokens removes the empty placeholder and sets `status`.
- `storage.rs`: `save_chat` → `load_chat` round-trip; empty messages removes the file; malformed JSON → empty; `chat_path_for` naming.
- `llm.rs::parse_sse_line`: `data: {json with delta}` → `Delta`; `data: [DONE]` → `Done`; blank/comment/garbage → `Ignore`.

**TUI (TestBackend render):**
- `chat_panel.rs`: messages render with `You:`/`AI:` prefixes; thinking indicator when `pending` + empty placeholder; status line renders.
- `layout.rs`: three-column split present when `chat.visible`; two-column (current) when hidden.
- `input.rs`: `Ctrl-L` → `ToggleChat`; Tab cycle includes `Chat` only when visible; `Focus::Chat` scroll keys map correctly.

**Manual integration:** the live `llm::spawn` HTTP/streaming path is verified by hand against a running LM Studio (steps documented in the implementation plan), since it requires the server.

---

## How Phase 2 builds on this

- `ChatRequest.system` + a new "chat mode" (Meeting / Note / To-Do) selects a custom system prompt and injects note context (current meeting, notes, todos) into the messages.
- A follow-up action can write AI output into the document via the existing `Document` API (`add_block`, `add_todo`, etc.).
- Persistence, layout, streaming, and error handling are unchanged — Phase 2 is additive. It gets its own spec.

---

## What this is not

- No async runtime (no tokio); `ureq` blocking calls run on worker threads.
- No changes to the note `Document` model or its file format.
- No dedicated chat input box — input flows through the existing capture box.
- No cross-day or global conversation; history is strictly per-day.
- No Phase 2 prompt modes or note-writing.
