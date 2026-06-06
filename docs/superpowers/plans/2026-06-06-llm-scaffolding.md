# LLM Scaffolding Pre-Work Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish non-blocking LLM infrastructure and typed AppState sub-structs so the LLM feature can land cleanly without freezing the event loop or retrofitting a flat 26-field AppState.

**Architecture:** Extract `right_panel_selected/scroll/panel_todos` into a `RightPanelState` sub-struct; add a `ChatState` stub with an `Option<Sender<LlmRequest>>`; create placeholder `LlmRequest`/`LlmEvent` types in `src/app/llm.rs`; wire an `mpsc` channel into `run()` with a non-blocking `try_recv()` poll each iteration. No LLM functionality is implemented — this is pure scaffolding.

**Tech Stack:** Rust, `std::sync::mpsc`, existing `anyhow`, ratatui

**Spec:** `docs/superpowers/specs/2026-06-06-llm-scaffolding-design.md`

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src/app/state.rs` | Add `RightPanelState`, `ChatState`; update `AppState` and `open_day` |
| Create | `src/app/llm.rs` | `LlmRequest`, `LlmEvent` placeholder enums + channel smoke test |
| Modify | `src/app/mod.rs` | Add `pub mod llm;` |
| Modify | `src/main.rs` | `use mpsc`, channel creation, placeholder thread, `try_recv()` poll |
| Modify | `src/app/actions.rs` | Rename `state.panel_todos` → `state.right_panel.todos`, `state.right_panel_selected` → `state.right_panel.selected` |
| Modify | `src/app/input.rs` | Same renames |
| Modify | `src/ui/right_panel.rs` | Same renames (production code in render functions) |
| Modify | `src/ui/layout.rs` | Update `AppState` construction in test helper |

---

## Task 1: Create `src/app/llm.rs` with placeholder types and smoke test

**Files:**
- Create: `src/app/llm.rs`
- Modify: `src/app/mod.rs`

This task has no dependencies. It creates the types that all subsequent tasks will reference.

- [ ] **Step 1: Verify the current test suite is green**

```bash
cargo test 2>&1 | grep "^test result"
```

Expected: all tests pass.

- [ ] **Step 2: Create `src/app/llm.rs`**

```rust
/// A request sent from the UI thread to the LLM worker thread.
/// Placeholder — LLM feature work populates the variants.
pub enum LlmRequest {
    // future: Prompt { text: String, context: Vec<String> }, Cancel, etc.
}

/// An event sent from the LLM worker thread back to the UI event loop.
/// Placeholder — LLM feature work populates the variants.
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

        // Dropping event_tx causes try_recv to return Err(Disconnected).
        drop(event_tx);
        assert!(event_rx.try_recv().is_err());

        // Request receiver returns Err(Empty) — sender still alive, no messages sent.
        assert!(rx.try_recv().is_err());
    }
}
```

- [ ] **Step 3: Add `pub mod llm;` to `src/app/mod.rs`**

The file currently reads:
```rust
pub mod actions;
pub mod command;
pub mod input;
pub mod state;
```

Add `pub mod llm;` in alphabetical order:
```rust
pub mod actions;
pub mod command;
pub mod input;
pub mod llm;
pub mod state;
```

- [ ] **Step 4: Run the new test and full suite**

```bash
cargo test llm_channel_smoke_test
cargo test 2>&1 | grep "^test result"
```

Expected: `llm_channel_smoke_test` passes; all other tests still pass.

- [ ] **Step 5: Commit**

```bash
git add src/app/llm.rs src/app/mod.rs
git commit -m "feat: add placeholder LlmRequest/LlmEvent types and channel smoke test"
```

---

## Task 2: Extract `RightPanelState` and add `ChatState` to `AppState`

**Files:**
- Modify: `src/app/state.rs`

This is a pure structural change to `AppState`. All callers will break until Task 3 renames the callsites, so Tasks 2 and 3 must be committed together or the build will fail between them. We do Task 2 here and verify with `cargo check` (not `cargo test`) before proceeding to Task 3.

- [ ] **Step 1: Replace the three flat fields and add sub-structs in `src/app/state.rs`**

The full new content of `src/app/state.rs`:

```rust
use crate::app::llm::LlmEvent;
use crate::config::Config;
use crate::model::day::{Document, Selectable};
use crate::storage;
use crate::ui::right_panel::{self, PanelTodo};
use chrono::NaiveDate;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Capture,
    Navigate,
    RightPanel,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Overlay {
    None,
    Help,
}

pub struct RightPanelState {
    pub selected: usize,
    pub scroll: usize,  // scroll offset for todo list — scroll-follow not yet implemented
    pub todos: Vec<PanelTodo>,
}

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

pub struct AppState {
    pub doc: Document,
    pub date: NaiveDate,
    pub notes_dir: PathBuf,
    pub config: Config,
    pub context: Context,
    pub focus: Focus,
    pub selected: usize,
    pub status: String,
    pub input: String,
    pub cursor_pos: usize,  // byte offset into `input`; always <= input.len(), always on a char boundary
    pub overlay: Overlay,
    pub editing: Option<usize>,
    pub should_quit: bool,
    pub selectables: Vec<Selectable>,
    pub context_display: String,
    pub pending_delete: bool,
    pub dates_with_notes: BTreeSet<NaiveDate>,
    pub right_panel: RightPanelState,
    pub chat: ChatState,
}

impl AppState {
    pub fn open_day(notes_dir: PathBuf, config: Config, date: NaiveDate) -> anyhow::Result<Self> {
        let path = storage::path_for(&notes_dir, date, &config.date_format);
        let doc = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            Document::from_text(&text)
        } else {
            Document::new_for_date(date)
        };
        let selectables = doc.selectables();
        let context_display = "context: Notes".to_string();
        let dates_with_notes = storage::dates_with_notes(&notes_dir, &config.date_format);
        Ok(Self {
            doc,
            date,
            notes_dir,
            config,
            context: Context::Notes,
            focus: Focus::Capture,
            selected: 0,
            status: String::new(),
            input: String::new(),
            cursor_pos: 0,
            overlay: Overlay::None,
            editing: None,
            should_quit: false,
            selectables,
            context_display,
            pending_delete: false,
            dates_with_notes,
            right_panel: RightPanelState {
                selected: 0,
                scroll: 0,
                todos: right_panel::collect_panel_todos(&notes_dir, date, &config),
            },
            chat: ChatState::default(),
        })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.notes_dir)?;
        let path = storage::path_for(&self.notes_dir, self.date, &self.config.date_format);
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.doc.to_text())?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn current_time_hhmm(&self) -> String {
        chrono::Local::now().format("%H:%M").to_string()
    }

    pub fn update_context_display(&mut self) {
        self.context_display = match self.context {
            Context::Notes => "context: Notes".to_string(),
            Context::Meeting(ord) => {
                let meetings = self.doc.meetings();
                match meetings.get(ord) {
                    Some(m) => format!("context: {}", m.name),
                    None => "context: Notes".to_string(),
                }
            }
            Context::NoteBlock(ord) => {
                let notes = self.doc.note_headings();
                match notes.get(ord) {
                    Some(n) => format!("context: {}", n.name),
                    None => "context: Notes".to_string(),
                }
            }
        };
    }

    /// Called by the event loop when the LLM worker thread sends an event.
    /// Placeholder — handled by LLM feature implementation.
    pub fn handle_llm_event(&mut self, _event: LlmEvent) {}
}
```

- [ ] **Step 2: Verify it compiles (ignoring callsite errors)**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

Expected: errors only about `right_panel_selected`, `right_panel_scroll`, `panel_todos` — no errors in `state.rs` itself. Do NOT commit yet.

---

## Task 3: Update all callsites for `RightPanelState` fields

**Files:**
- Modify: `src/app/actions.rs`
- Modify: `src/app/input.rs`
- Modify: `src/ui/right_panel.rs`
- Modify: `src/ui/layout.rs`

These are all mechanical field renames. Do all four files, then compile and commit.

- [ ] **Step 1: Update `src/app/actions.rs`**

Apply these renames throughout the file (use your editor's find-and-replace):

| Find | Replace |
|------|---------|
| `state.panel_todos` | `state.right_panel.todos` |
| `state.right_panel_selected` | `state.right_panel.selected` |
| `state.right_panel_scroll` | `state.right_panel.scroll` |

Affected lines in `actions.rs`: 33, 34, 159, 160, 218, 227, 228, 251, 252, 259, 261, 263, 863, 865, 876, 877, 892, 893, 896, 897, 907, 915, 936, 964, 969, 990, 991, 998, 1002, 1003.

Note: lines referencing `collect_panel_todos(...)` as a function call do NOT need renaming — only the `state.panel_todos` field access does.

- [ ] **Step 2: Update `src/app/input.rs`**

Apply the same renames:

| Find | Replace |
|------|---------|
| `state.panel_todos` | `state.right_panel.todos` |
| `state.right_panel_selected` | `state.right_panel.selected` |
| `state.right_panel_scroll` | `state.right_panel.scroll` |

Affected lines in `input.rs`: 363, 370, 371, 375, 376, 377, 897, 900, 917, 918, 931, 939, 944, 946, 954, 955, 968, 976, 978.

- [ ] **Step 3: Update `src/ui/right_panel.rs`**

Apply the same renames to `app.panel_todos`, `app.right_panel_selected`, `app.right_panel_scroll` (these are accessed via `app: &AppState` parameter, not `state`):

| Find | Replace |
|------|---------|
| `app.panel_todos` | `app.right_panel.todos` |
| `app.right_panel_selected` | `app.right_panel.selected` |
| `app.right_panel_scroll` | `app.right_panel.scroll` |

Also update the three test helper `AppState` constructions in the test module (lines 225–227, 279–281, 332–334) which use struct literal syntax. Change:
```rust
right_panel_selected: 0,
right_panel_scroll: 0,
panel_todos: Vec::new(),
```
To:
```rust
right_panel: crate::app::state::RightPanelState {
    selected: 0,
    scroll: 0,
    todos: Vec::new(),
},
chat: crate::app::state::ChatState::default(),
```

- [ ] **Step 4: Update `src/ui/layout.rs`**

The test helper `test_app()` constructs an `AppState` via struct literal. Find the three lines:
```rust
right_panel_selected: 0,
right_panel_scroll: 0,
panel_todos: Vec::new(),
```
And replace with:
```rust
right_panel: buff::app::state::RightPanelState {
    selected: 0,
    scroll: 0,
    todos: Vec::new(),
},
chat: buff::app::state::ChatState::default(),
```

- [ ] **Step 5: Verify the full build compiles cleanly**

```bash
cargo build 2>&1 | grep -E "^error|warning\[unused"
```

Expected: zero errors, zero unused-import warnings.

- [ ] **Step 6: Run the full test suite**

```bash
cargo test 2>&1 | grep "^test result"
```

Expected: all tests pass.

- [ ] **Step 7: Commit Tasks 2 and 3 together**

```bash
git add src/app/state.rs src/app/actions.rs src/app/input.rs src/ui/right_panel.rs src/ui/layout.rs
git commit -m "refactor: extract RightPanelState and add ChatState sub-structs to AppState"
```

---

## Task 4: Wire the mpsc channel into `run()`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace `src/main.rs` with the channel-wired version**

The full new content of `src/main.rs`:

```rust
use anyhow::{Context, Result};
use ratatui::crossterm::event::{Event, KeyEventKind};
use std::sync::mpsc;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn read_key() -> Result<Option<ratatui::crossterm::event::KeyEvent>> {
    if !ratatui::crossterm::event::poll(std::time::Duration::from_millis(100))? {
        return Ok(None);
    }
    match ratatui::crossterm::event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => Ok(Some(key)),
        _ => Ok(None),
    }
}

struct CliArgs {
    notes_dir: Option<String>,
}

fn parse_cli_args() -> Result<Option<CliArgs>> {
    let mut args = std::env::args().skip(1);
    let mut notes_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--notes-dir" => match args.next() {
                Some(v) => notes_dir = Some(v),
                None => {
                    return Err(anyhow::anyhow!("--notes-dir requires a value"));
                }
            },
            "--help" => {
                println!("Usage: buff [--notes-dir <path>]");
                return Ok(None);
            }
            "--version" => {
                println!("buff {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown flag: {}", arg));
            }
        }
    }
    Ok(Some(CliArgs { notes_dir }))
}

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

- [ ] **Step 2: Run the full test suite**

```bash
cargo test 2>&1 | grep "^test result"
```

Expected: all tests pass.

- [ ] **Step 3: Build cleanly**

```bash
cargo build 2>&1 | grep -E "^error|warning\[unused"
```

Expected: zero errors. There may be one warning about `_llm_rx_worker` being unused — that is expected and acceptable (the underscore prefix suppresses it).

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire mpsc channel into run() for non-blocking LLM event polling"
```

---

## Final Verification

- [ ] **Run the complete test suite one last time**

```bash
cargo test 2>&1 | grep "^test result"
```

Expected: all tests pass including `llm::tests::llm_channel_smoke_test`.

- [ ] **Confirm new struct and channel are visible to future LLM work**

```bash
cargo doc --no-deps 2>&1 | grep "^error"
```

Expected: no errors. The `LlmRequest`, `LlmEvent`, `ChatState`, and `RightPanelState` types are all public and documented.
