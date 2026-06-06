# Design: LLM Pre-Work Scaffolding — Sub-Structs + Channel Wiring

**Date:** 2026-06-06  
**Status:** Approved  
**Context:** Pre-work before implementing local LLM support. Establishes non-blocking async infrastructure and typed state groupings so the LLM feature can land cleanly without retrofitting around a frozen event loop or a flat 26-field AppState.

---

## Goals

1. Extract `RightPanelState` sub-struct from `AppState` — establishes the pattern and makes room for `ChatState`
2. Add `ChatState` sub-struct to `AppState` — the future home of all LLM conversation state
3. Add placeholder `LlmRequest` / `LlmEvent` message types in a new `src/app/llm.rs` module
4. Wire an `mpsc` channel into `run()` with a non-blocking poll in the event loop — prevents LLM calls from freezing the UI
5. Add a stub `handle_llm_event` on `AppState` — the integration point for LLM feature work

All behavior is preserved. This is scaffolding only; no LLM functionality is implemented.

---

## Approach

**Sender in `ChatState`, receiver in `run()`.**

- `Sender<LlmRequest>` lives in `ChatState` — accessible from anywhere that needs to fire a request (e.g., `execute_action`, future `/ask` command handler)
- `Receiver<LlmEvent>` lives as a local in `run()` — only the event loop polls it; keeping it there avoids awkward borrow splitting when rendering reads `AppState`

---

## File Map

| Action | File | What changes |
|--------|------|-------------|
| Modify | `src/app/state.rs` | Add `RightPanelState`, `ChatState`; update `AppState`, `open_day` |
| Create | `src/app/llm.rs` | `LlmRequest`, `LlmEvent` enums, channel smoke test |
| Modify | `src/app/mod.rs` | Add `pub mod llm;` |
| Modify | `src/main.rs` | Channel creation, placeholder thread, `try_recv()` poll in loop |
| Modify | `src/app/input.rs` | ~15 callsite renames for `right_panel.*` |
| Modify | `src/app/actions.rs` | Callsite renames for `right_panel.*` |
| Modify | `src/ui/right_panel.rs` | Callsite renames for `right_panel.*` |

---

## Section 1: `RightPanelState`

### New struct in `src/app/state.rs`

```rust
pub struct RightPanelState {
    pub selected: usize,
    pub scroll: usize,
    pub todos: Vec<crate::ui::right_panel::PanelTodo>,
}
```

### Changes to `AppState`

Remove:
```rust
pub right_panel_selected: usize,
pub right_panel_scroll: usize,
pub panel_todos: Vec<PanelTodo>,
```

Add:
```rust
pub right_panel: RightPanelState,
```

### Changes to `open_day`

Replace:
```rust
right_panel_selected: 0,
right_panel_scroll: 0,
panel_todos,
```

With:
```rust
right_panel: RightPanelState {
    selected: 0,
    scroll: 0,
    todos: right_panel::collect_panel_todos(&notes_dir, date, &config),
},
```

### Callsite renames (mechanical)

| Old | New |
|-----|-----|
| `state.right_panel_selected` | `state.right_panel.selected` |
| `state.right_panel_scroll` | `state.right_panel.scroll` |
| `state.panel_todos` | `state.right_panel.todos` |

Affected files: `src/app/input.rs` (~10 occurrences), `src/app/actions.rs` (~5 occurrences), `src/ui/right_panel.rs` (~8 occurrences).

---

## Section 2: `ChatState` and placeholder message types

### New struct in `src/app/state.rs`

```rust
pub struct ChatState {
    /// Sender for dispatching requests to the LLM background thread.
    /// None until the LLM feature is configured and its worker thread is running.
    pub llm_tx: Option<std::sync::mpsc::Sender<crate::app::llm::LlmRequest>>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self { llm_tx: None }
    }
}
```

### Changes to `AppState`

Add field:
```rust
pub chat: ChatState,
```

### Changes to `open_day`

Add to constructor:
```rust
chat: ChatState::default(),
```

### New file: `src/app/llm.rs`

```rust
/// A request sent from the UI thread to the LLM worker thread.
/// Variants are placeholder — LLM feature work populates this.
pub enum LlmRequest {
    // future: Prompt { text: String, context: Vec<String> }, Cancel, etc.
}

/// An event sent from the LLM worker thread back to the UI event loop.
/// Variants are placeholder — LLM feature work populates this.
pub enum LlmEvent {
    // future: Token(String), Done, Error(String), etc.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn llm_channel_smoke_test() {
        // Confirms channel types compile and messages flow correctly.
        let (_tx, rx) = mpsc::channel::<LlmRequest>();
        let (event_tx, event_rx) = mpsc::channel::<LlmEvent>();

        // Receiver returns Disconnected when sender is dropped — correct stub behaviour.
        drop(event_tx);
        assert!(event_rx.try_recv().is_err());

        // Request receiver returns Empty (not Disconnected) — sender is still alive.
        // Using is_err() covers both; the important thing is no message is present.
        assert!(rx.try_recv().is_err());
    }
}
```

### Changes to `src/app/mod.rs`

Add:
```rust
pub mod llm;
```

---

## Section 3: Channel wiring in `run()`

### Changes to `src/main.rs`

Add import:
```rust
use std::sync::mpsc;
```

Replace the current `run()` body with:

```rust
fn run() -> Result<()> {
    let Some(cli) = parse_cli_args()? else {
        return Ok(());
    };

    let (config, notes_dir) = buff::config::load(cli.notes_dir).context("Config error")?;
    let mut app =
        buff::app::state::AppState::open_day(notes_dir, config, chrono::Local::now().date_naive())
            .context("Failed to open day")?;

    // LLM channel: UI thread sends LlmRequests, receives LlmEvents.
    // The sender lives in app.chat; the receiver stays here in the event loop.
    let (llm_tx, _llm_rx_worker) = mpsc::channel::<buff::app::llm::LlmRequest>();
    let (llm_event_tx, llm_event_rx) = mpsc::channel::<buff::app::llm::LlmEvent>();
    app.chat.llm_tx = Some(llm_tx);

    // Placeholder LLM worker thread — does nothing until LLM feature is implemented.
    // Dropping llm_event_tx causes llm_event_rx.try_recv() to return Err(Disconnected),
    // so the poll arm below never fires in this stub state.
    std::thread::spawn(move || {
        drop(llm_event_tx);
    });

    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;

    loop {
        terminal.draw(|frame| {
            buff::ui::render(frame, &app);
        })?;

        // Poll for LLM events (non-blocking — never fires in stub state)
        if let Ok(event) = llm_event_rx.try_recv() {
            app.handle_llm_event(event);
        }

        if let Some(key) = read_key()? {
            if let Some(action) = buff::app::input::key_to_action(&app, key) {
                if buff::app::input::execute_action(&mut app, action)?
                    == buff::app::input::EventOutcome::Quit
                {
                    break;
                }
            }
        }
    }

    Ok(())
}
```

---

## Section 4: `handle_llm_event` stub

Add to `impl AppState` in `src/app/state.rs`:

```rust
/// Called by the event loop when the LLM worker thread sends an event.
/// Placeholder — handled by LLM feature implementation.
pub fn handle_llm_event(&mut self, _event: crate::app::llm::LlmEvent) {}
```

---

## Section 5: Testing

**No new behavioral tests** are needed beyond `llm_channel_smoke_test` in `src/app/llm.rs`. The scaffolding has no observable behavior change.

**Regression check:** All existing tests pass unchanged. The `RightPanelState` extraction is mechanical field renaming — all tests that previously set `state.right_panel_selected = 0` etc. are updated to `state.right_panel.selected = 0`. The `test_state` helper in `src/app/actions.rs` is updated to construct `right_panel: RightPanelState { selected: 0, scroll: 0, todos: vec![] }`.

---

## What This Is Not

- No LLM API calls
- No chat UI panel
- No new `UiAction` variants
- No changes to `key_to_action` or `execute_action` logic
- No changes to `dispatch` or any command handling

The LLM feature work begins after this scaffolding is merged.

---

## How LLM Feature Work Uses This

When the LLM feature lands:

1. Populate `LlmRequest` and `LlmEvent` variants
2. Replace the placeholder thread body with real LLM worker logic (HTTP/subprocess)
3. `execute_action` sends requests via `state.chat.llm_tx.as_ref().unwrap().send(...)`
4. `handle_llm_event` handles response chunks, updating `state.chat` fields as they're added
5. `state.chat` gains `messages`, `pending`, `scroll` etc. as needed — all in one place
