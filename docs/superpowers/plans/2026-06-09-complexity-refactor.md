# Complexity Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce complexity in the input/dispatch pipeline by splitting `input.rs` into a module, thinning `actions.rs::dispatch`, and extracting `context_at_line` helpers — with zero behavior changes.

**Architecture:** Two-stage input pipeline (`key_to_action` → `execute_action`) is preserved but split by focus mode into `src/app/input/{mod,capture,vim_normal,vim_insert,right_panel,chat}.rs`. Command handling in `actions.rs` becomes a thin coordinator calling named handler functions. `context_at_line` moves to `src/app/context.rs` with named helper functions.

**Tech Stack:** Rust, Cargo. All tests are inline `#[cfg(test)]` modules. Test runner: `cargo test`.

---

> **Commit cadence:** Each task ends with a commit. Tasks 2 and 3 are independent of Tasks 4–9 and can be reviewed as separate PRs if desired.

---

### Task 1: Establish baseline

**Files:**
- Read: `src/app/mod.rs`, `src/app/state.rs`, `src/app/actions.rs`, `src/app/input.rs`

- [ ] **Step 1: Run the full test suite**

```bash
cargo test
```

Expected: all tests pass. Note the total count (e.g. "test result: ok. 187 passed"). This number is your regression baseline — every subsequent task must end with the same count passing.

- [ ] **Step 2: Confirm the three hotspot functions exist at expected locations**

```bash
cargo test 2>&1 | grep "test result"
```

If any tests fail before you start, stop and investigate before proceeding.

---

### Task 2: Extract `context_at_line` to `src/app/context.rs`

**Files:**
- Create: `src/app/context.rs`
- Modify: `src/app/mod.rs` (add `pub mod context;`)
- Modify: `src/app/state.rs` (remove `context_at_line` and its 7 tests)
- Modify: `src/app/actions.rs` (update import from `state::context_at_line` → `context::context_at_line`)

This is a pure move + refactor. The public signature of `context_at_line` is unchanged.

- [ ] **Step 1: Add `pub mod context;` to `src/app/mod.rs`**

Open `src/app/mod.rs`. It currently reads:
```rust
pub mod actions;
pub mod command;
pub mod input;
pub mod llm;
pub mod state;
```

Add `pub mod context;` so it reads:
```rust
pub mod actions;
pub mod command;
pub mod context;
pub mod input;
pub mod llm;
pub mod state;
```

- [ ] **Step 2: Create `src/app/context.rs` with helpers and refactored `context_at_line`**

Create the file with this exact content:

```rust
use crate::app::state::Context;

/// Find the line index of the nearest "## " heading at or before `cursor_line`.
fn enclosing_l2_heading(lines: &[String], cursor_line: usize) -> Option<usize> {
    (0..=cursor_line).rev().find(|&i| lines[i].starts_with("## "))
}

/// Find the index of the last "### " heading in `lines[start..=end]`.
/// Stops scanning if a "## " heading is encountered (crossed into another section).
fn last_l3_heading(lines: &[String], start: usize, end: usize) -> Option<usize> {
    let mut result = None;
    for i in start..=end {
        if lines[i].starts_with("## ") {
            break;
        }
        if lines[i].starts_with("### ") {
            result = Some(i);
        }
    }
    result
}

/// Find the index and level of the last "####"+ heading in `lines[(l3_line+1)..=end]`.
/// Returns `None` if no such heading exists, or if a "### " heading resets the search.
fn last_l4plus_heading(lines: &[String], l3_line: usize, end: usize) -> Option<(usize, u8)> {
    let mut result = None;
    for i in (l3_line + 1)..=end {
        let line = &lines[i];
        if line.starts_with("## ") || line.starts_with("### ") {
            break;
        }
        if line.starts_with("#### ")
            || line.starts_with("##### ")
            || line.starts_with("###### ")
        {
            let level = line.chars().take_while(|&c| c == '#').count() as u8;
            result = Some((i, level));
        }
    }
    result
}

/// Count the number of "### " headings in `lines[start..=end]`.
/// Used to compute the zero-based ordinal for Meeting/NoteBlock context.
fn count_l3_headings(lines: &[String], start: usize, end: usize) -> usize {
    lines[start..=end]
        .iter()
        .filter(|l| l.starts_with("### "))
        .count()
}

/// Derive the editing context from a cursor position in the document.
/// Used to update `state.context` automatically as the vim cursor moves.
pub fn context_at_line(lines: &[String], cursor_line: usize) -> Context {
    if lines.is_empty() || cursor_line >= lines.len() {
        return Context::Notes;
    }

    let boundary = match enclosing_l2_heading(lines, cursor_line) {
        Some(b) => b,
        None => return Context::Notes,
    };

    let section = &lines[boundary];

    if section == "## To-dos" {
        return Context::Todos;
    }

    let in_meetings = section == "## Meetings";
    let in_notes = section == "## Notes";
    if !in_meetings && !in_notes {
        return Context::Notes;
    }

    let l3 = match last_l3_heading(lines, boundary + 1, cursor_line) {
        Some(l) => l,
        None => return Context::Notes,
    };

    if let Some((l4, level)) = last_l4plus_heading(lines, l3, cursor_line) {
        return Context::Section { heading_line: l4, level };
    }

    let ordinal = count_l3_headings(lines, boundary + 1, l3).saturating_sub(1);
    if in_meetings {
        Context::Meeting(ordinal)
    } else {
        Context::NoteBlock(ordinal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(|l| l.to_string()).collect()
    }

    // --- helper unit tests ---

    #[test]
    fn enclosing_l2_finds_nearest_above() {
        let doc = lines("## Meetings\n### Standup\nsome line");
        assert_eq!(enclosing_l2_heading(&doc, 2), Some(0));
    }

    #[test]
    fn enclosing_l2_returns_none_when_absent() {
        let doc = lines("### Standup\nsome line");
        assert_eq!(enclosing_l2_heading(&doc, 1), None);
    }

    #[test]
    fn last_l3_finds_last_in_range() {
        let doc = lines("## Meetings\n### First\n### Second\ncontent");
        // range 1..=3 (after the ## heading)
        assert_eq!(last_l3_heading(&doc, 1, 3), Some(2));
    }

    #[test]
    fn last_l3_stops_at_l2_boundary() {
        let doc = lines("## Meetings\n### First\n## Notes\n### Other");
        assert_eq!(last_l3_heading(&doc, 1, 3), Some(1));
    }

    #[test]
    fn last_l4plus_finds_heading_after_l3() {
        let doc = lines("### Meeting\n#### Phase 1\ncontent");
        assert_eq!(last_l4plus_heading(&doc, 0, 2), Some((1, 4)));
    }

    #[test]
    fn last_l4plus_returns_none_when_new_l3_resets() {
        let doc = lines("### First\n#### Phase\n### Second\ncontent");
        // l3_line=2 (the "### Second" line), cursor at 3
        assert_eq!(last_l4plus_heading(&doc, 2, 3), None);
    }

    #[test]
    fn count_l3_headings_counts_correctly() {
        let doc = lines("## Meetings\n### First\ncontent\n### Second\ncontent");
        assert_eq!(count_l3_headings(&doc, 1, 4), 2);
    }

    // --- context_at_line integration tests (moved from state.rs) ---

    #[test]
    fn cursor_above_all_sections_is_notes() {
        let doc = lines("some preamble\n## Meetings");
        assert_eq!(context_at_line(&doc, 0), Context::Notes);
    }

    #[test]
    fn cursor_in_meetings_no_heading_is_notes() {
        let doc = lines("## Meetings\nno meeting heading yet");
        assert_eq!(context_at_line(&doc, 1), Context::Notes);
    }

    #[test]
    fn cursor_on_meeting_heading_is_meeting_0() {
        let doc = lines("## Meetings\n### Standup");
        assert_eq!(context_at_line(&doc, 1), Context::Meeting(0));
    }

    #[test]
    fn cursor_in_second_meeting_is_meeting_1() {
        let doc = lines("## Meetings\n### First\nstuff\n### Second\ncontent");
        assert_eq!(context_at_line(&doc, 4), Context::Meeting(1));
    }

    #[test]
    fn cursor_in_section_under_meeting() {
        let doc = lines("## Meetings\n### Standup\n#### Phase 1\ncontent");
        assert_eq!(
            context_at_line(&doc, 3),
            Context::Section { heading_line: 2, level: 4 }
        );
    }

    #[test]
    fn cursor_in_todos_section() {
        let doc = lines("## To-dos\n- [ ] something");
        assert_eq!(context_at_line(&doc, 1), Context::Todos);
    }

    #[test]
    fn cursor_in_note_block() {
        let doc = lines("## Notes\n### My Note\ncontent");
        assert_eq!(context_at_line(&doc, 2), Context::NoteBlock(0));
    }

    #[test]
    fn cursor_on_empty_lines_vec() {
        assert_eq!(context_at_line(&[], 0), Context::Notes);
    }
}
```

- [ ] **Step 3: Update `src/app/actions.rs` import**

Find this line in `src/app/actions.rs` (it is inside `vim_update_context`):
```rust
use crate::app::state::context_at_line;
```

Replace it with:
```rust
use crate::app::context::context_at_line;
```

- [ ] **Step 4: Remove `context_at_line` and its tests from `src/app/state.rs`**

In `src/app/state.rs`:

Remove the entire `context_at_line` function (lines 220–289, including the doc comment starting at line 220).

Remove the entire `#[cfg(test)] mod context_tests { ... }` block (from the `#[cfg(test)]` attribute through the closing `}`). This is the block containing `cursor_above_all_sections_is_notes`, `cursor_in_meetings_no_heading_is_notes`, `cursor_on_meeting_heading_is_meeting_0`, `cursor_in_second_meeting_is_meeting_1`, `cursor_in_section_under_meeting`, `cursor_in_todos_section`, `cursor_in_note_block`, and `cursor_on_empty_lines_vec`.

Also remove the now-unused `pub` visibility marker on `context_at_line` (the function is gone, but ensure `context_at_line` is not still referenced in state.rs — grep to verify).

- [ ] **Step 5: Run tests and verify count matches baseline**

```bash
cargo test
```

Expected: same number of tests pass as in Task 1. The 8 tests that moved from `state.rs` to `context.rs` still run — they just appear under a different module path in the output.

- [ ] **Step 6: Commit**

```bash
git add src/app/mod.rs src/app/context.rs src/app/state.rs src/app/actions.rs
git commit -m "refactor: extract context_at_line helpers to src/app/context.rs"
```

---

### Task 3: Thin `dispatch` in `src/app/actions.rs`

**Files:**
- Modify: `src/app/actions.rs` only

Extract each command arm into a private named function. `dispatch` becomes a ~20-line match coordinator. No logic changes.

- [ ] **Step 1: Run tests to confirm baseline**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 2: Replace `dispatch` with a thin coordinator and extracted handlers**

Replace the entire `pub fn dispatch(state: &mut AppState, cmd: Command) -> anyhow::Result<()>` function (lines 82–299 in the original file) and add the private handler functions. The new content is:

```rust
pub fn dispatch(state: &mut AppState, cmd: Command) -> anyhow::Result<()> {
    match cmd {
        Command::Entry(text)       => handle_entry(state, &text)?,
        Command::Meeting(name)     => handle_meeting(state, &name)?,
        Command::Note(name)        => handle_note(state, name)?,
        Command::Todo(text)        => handle_todo(state, &text)?,
        Command::Leave             => handle_leave(state),
        Command::Help              => state.overlay = crate::app::state::Overlay::Help,
        Command::Quit              => state.should_quit = true,
        Command::Summarize         => handle_summarize(state),
        Command::Ask(text)         => handle_ask(state, &text)?,
        Command::Clear             => handle_clear(state)?,
        Command::Start             => handle_start(state)?,
        Command::End               => handle_end(state)?,
        Command::Scheduled(time)   => handle_scheduled(state, &time)?,
        Command::Section(name)     => handle_section(state, &name)?,
        Command::Unknown(word)     => state.status = format!("Unknown command: /{}", word),
        Command::InvalidArgs(msg)  => state.status = msg,
        Command::Today             => { go_today(state)?; state.status.clear(); }
        Command::Goto(Some(date))  => { go_to_date(state, date)?; state.status.clear(); }
        Command::Goto(None)        => state.status = "usage: /goto YYYY-MM-DD".to_string(),
    }
    Ok(())
}

fn handle_entry(state: &mut AppState, text: &str) -> anyhow::Result<()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    let time_str = state.current_time_hhmm();
    let time = if state.config.timestamp_entries {
        Some(time_str.as_str())
    } else {
        None
    };
    let block = crate::model::writer::format_entry(text, time);
    let target = match &state.context {
        Context::Notes | Context::Todos => crate::model::day::EntryTarget::Notes,
        Context::Meeting(ord) => crate::model::day::EntryTarget::Meeting(*ord),
        Context::NoteBlock(ord) => crate::model::day::EntryTarget::NoteBlock(*ord),
        Context::Section { heading_line, level } => {
            crate::model::day::EntryTarget::Section { heading_line: *heading_line, level: *level }
        }
    };
    state.doc.add_block(&target, &block);
    after_doc_mutation(state)
}

fn handle_meeting(state: &mut AppState, name: &str) -> anyhow::Result<()> {
    let ord = state.doc.add_meeting(name);
    state.context = Context::Meeting(ord);
    state.update_context_display();
    after_doc_mutation(state)
}

fn handle_note(state: &mut AppState, name: Option<String>) -> anyhow::Result<()> {
    if let Some(n) = name {
        let ord = state.doc.add_note_heading(&n);
        state.context = Context::NoteBlock(ord);
        state.update_context_display();
        after_doc_mutation(state)
    } else {
        state.context = Context::Notes;
        state.update_context_display();
        state.status.clear();
        Ok(())
    }
}

fn handle_todo(state: &mut AppState, text: &str) -> anyhow::Result<()> {
    let meeting_name = match &state.context {
        Context::Meeting(ord) => state.doc.meetings().get(*ord).map(|m| m.name.clone()),
        Context::NoteBlock(ord) => state.doc.note_headings().get(*ord).map(|n| n.name.clone()),
        _ => None,
    };
    state.doc.add_todo(text, meeting_name.as_deref());
    after_doc_mutation(state)
}

fn handle_leave(state: &mut AppState) {
    state.context = Context::Notes;
    state.update_context_display();
    state.status.clear();
}

fn handle_summarize(state: &mut AppState) {
    state.status = "summarize is not implemented yet".to_string();
}

fn handle_ask(state: &mut AppState, text: &str) -> anyhow::Result<()> {
    let Some(tx) = state.chat.event_tx.clone() else {
        state.chat.status = Some("LLM channel unavailable".to_string());
        return Ok(());
    };
    state.chat.visible = true;
    state.chat.status = None;
    state.chat.scroll = 0;
    state.chat.messages.push(crate::app::state::ChatMessage {
        role: crate::app::state::ChatRole::User,
        content: text.to_string(),
    });
    let _ = state.save_chat();
    let request_messages = state.chat.messages.clone();
    let id = crate::app::llm::next_request_id();
    state.chat.active_request = id;
    state.chat.pending = true;
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
    Ok(())
}

fn handle_clear(state: &mut AppState) -> anyhow::Result<()> {
    state.chat.messages.clear();
    state.chat.active_request = crate::app::llm::next_request_id();
    state.chat.pending = false;
    state.chat.status = None;
    state.chat.scroll = 0;
    let _ = state.save_chat();
    Ok(())
}

fn handle_start(state: &mut AppState) -> anyhow::Result<()> {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => { state.status = "Not in a meeting".to_string(); return Ok(()); }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        let time = state.current_time_hhmm();
        crate::model::writer::set_meeting_time_field(
            &mut state.doc.lines, heading, "Started", &time,
        );
        after_doc_mutation(state)?;
    }
    Ok(())
}

fn handle_end(state: &mut AppState) -> anyhow::Result<()> {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => { state.status = "Not in a meeting".to_string(); return Ok(()); }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        let time = state.current_time_hhmm();
        crate::model::writer::set_meeting_time_field(
            &mut state.doc.lines, heading, "Ended", &time,
        );
        after_doc_mutation(state)?;
    }
    Ok(())
}

fn handle_scheduled(state: &mut AppState, time: &str) -> anyhow::Result<()> {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => { state.status = "Not in a meeting".to_string(); return Ok(()); }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        crate::model::writer::set_meeting_time_field(
            &mut state.doc.lines, heading, "Scheduled", time,
        );
        after_doc_mutation(state)?;
    }
    Ok(())
}

fn handle_section(state: &mut AppState, name: &str) -> anyhow::Result<()> {
    let current_level: u8 = match &state.context {
        Context::Meeting(_) | Context::NoteBlock(_) => 3,
        Context::Section { level, .. } => *level,
        Context::Notes | Context::Todos => {
            state.status = "Not in a meeting or note".to_string();
            return Ok(());
        }
    };
    if current_level >= 6 {
        state.status = "/section: already at maximum depth (######)".to_string();
        return Ok(());
    }
    let target = match &state.context {
        Context::Meeting(ord) => crate::model::day::EntryTarget::Meeting(*ord),
        Context::NoteBlock(ord) => crate::model::day::EntryTarget::NoteBlock(*ord),
        Context::Section { heading_line, level } => {
            crate::model::day::EntryTarget::Section { heading_line: *heading_line, level: *level }
        }
        Context::Notes | Context::Todos => unreachable!(),
    };
    let next_level = current_level + 1;
    let heading_line = state.doc.add_section_heading(&target, next_level, name);
    state.context = Context::Section { heading_line, level: next_level };
    state.update_context_display();
    after_doc_mutation(state)
}
```

- [ ] **Step 3: Verify the import at the top of `actions.rs` is correct**

The file uses `Context` and `EntryTarget`. Confirm these are in scope:
```rust
use crate::app::command::Command;
use crate::app::state::{AppState, Context};
use crate::model::day::{EntryTarget, SelectableKind};
```

If `EntryTarget` is now accessed via `crate::model::day::EntryTarget` inline in the handler functions, the `use` import can be removed from the top-level imports. Keep whichever approach compiles cleanly.

- [ ] **Step 4: Run tests**

```bash
cargo test
```

Expected: all tests pass, same count as baseline.

- [ ] **Step 5: Commit**

```bash
git add src/app/actions.rs
git commit -m "refactor: thin dispatch() into named command handler functions"
```

---

### Task 4: Convert `src/app/input.rs` to `src/app/input/mod.rs`

**Files:**
- Rename: `src/app/input.rs` → `src/app/input/mod.rs`
- Modify: `src/app/input/mod.rs` (add submodule declarations)

This is a mechanical rename. No code changes. The `pub mod input;` in `src/app/mod.rs` is unchanged — Rust finds `input/mod.rs` automatically.

- [ ] **Step 1: Create the `src/app/input/` directory and move the file**

```bash
mkdir -p src/app/input
mv src/app/input.rs src/app/input/mod.rs
```

Run from the project root (`/Users/timothy/code/buff`).

- [ ] **Step 2: Add submodule declarations to `src/app/input/mod.rs`**

Add these six lines at the very top of `src/app/input/mod.rs` (before any `use` statements):

```rust
mod capture;
mod chat;
mod right_panel;
mod vim_insert;
mod vim_normal;
```

- [ ] **Step 3: Make the six cursor helper functions `pub(super)` in `src/app/input/mod.rs`**

Submodules call these helpers via `super::fn_name()`; that requires the functions to be visible to child modules. In `src/app/input/mod.rs`, change the six private functions to `pub(super)`:

Find and replace each `fn` declaration:
```rust
// Before (×6):
fn prev_char_boundary(s: &str, pos: usize) -> usize {
fn next_char_boundary(s: &str, pos: usize) -> usize {
fn vim_clamp_col(line: &str, col: usize) -> usize {
fn next_word_start(line: &str, col: usize) -> usize {
fn prev_word_start(line: &str, col: usize) -> usize {
fn word_end(line: &str, col: usize) -> usize {

// After (×6):
pub(super) fn prev_char_boundary(s: &str, pos: usize) -> usize {
pub(super) fn next_char_boundary(s: &str, pos: usize) -> usize {
pub(super) fn vim_clamp_col(line: &str, col: usize) -> usize {
pub(super) fn next_word_start(line: &str, col: usize) -> usize {
pub(super) fn prev_word_start(line: &str, col: usize) -> usize {
pub(super) fn word_end(line: &str, col: usize) -> usize {
```

- [ ] **Step 4: Create empty stub files for each submodule**

These must exist for the `mod` declarations to compile. Create each file with just a comment:

```bash
echo '// capture mode key handling' > src/app/input/capture.rs
echo '// chat mode key handling' > src/app/input/chat.rs
echo '// right panel key handling' > src/app/input/right_panel.rs
echo '// vim insert mode key handling' > src/app/input/vim_insert.rs
echo '// vim normal mode key handling' > src/app/input/vim_normal.rs
```

- [ ] **Step 5: Note — the `can_navigate` cross-mode block stays in `mod.rs`**

`src/app/input/mod.rs` contains a cross-mode guard (currently around line 254–263) that handles `[` and `]` day navigation when either in VimNormal focus or in Capture with an empty input:

```rust
let can_navigate = matches!(state.focus, Focus::VimNormal)
    || (matches!(state.focus, Focus::Capture) && state.input.is_empty());
if can_navigate {
    match key.code {
        KeyCode::Char('[') => return Some(UiAction::PrevDay),
        KeyCode::Char(']') => return Some(UiAction::NextDay),
        _ => {}
    }
}
```

This block spans two modes and must remain in `mod.rs` between the Esc handler and the mode dispatch. Do not move it to any mode file.

- [ ] **Step 7: Run tests**

```bash
cargo test
```

Expected: all tests pass, same count as baseline. The rename is transparent to callers.

- [ ] **Step 8: Commit**

```bash
git add src/app/input/
git commit -m "refactor: convert input.rs to input/ module directory"
```

Expected: all tests pass, same count as baseline. The rename is transparent to callers.

- [ ] **Step 5: Commit**

```bash
git add src/app/input/ src/app/input/mod.rs
git commit -m "refactor: convert input.rs to input/ module directory"
```

---

### Task 5: Extract capture mode handler

**Files:**
- Modify: `src/app/input/mod.rs` (replace inline capture code with delegation calls)
- Modify: `src/app/input/capture.rs` (add the extracted functions)

Extract the capture-specific key mapping and action execution into `capture.rs`. Each function is `pub(super)` — visible only within the `input` module.

- [ ] **Step 1: Write `src/app/input/capture.rs`**

Replace the stub content with:

```rust
use crate::app::state::{AppState, Focus, Overlay};
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Enter => {
            if state.editing.is_some() {
                Some(UiAction::CommitEdit)
            } else {
                Some(UiAction::SubmitInput)
            }
        }
        KeyCode::Backspace => Some(UiAction::DeleteChar),
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::TypeNewline)
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
        {
            Some(UiAction::TypeChar(c))
        }
        KeyCode::Left => Some(UiAction::MoveCursorLeft),
        KeyCode::Right => Some(UiAction::MoveCursorRight),
        KeyCode::Home => Some(UiAction::MoveCursorLineStart),
        KeyCode::End => Some(UiAction::MoveCursorLineEnd),
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::MoveCursorLineStart)
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::MoveCursorLineEnd)
        }
        KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::PrependIndent)
        }
        KeyCode::Up | KeyCode::Down => None,
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::TypeChar(c) => {
            state.input.insert(state.cursor_pos, c);
            state.cursor_pos += c.len_utf8();
        }
        UiAction::DeleteChar => {
            if state.cursor_pos > 0 {
                let prev = super::prev_char_boundary(&state.input, state.cursor_pos);
                state.input.remove(prev);
                state.cursor_pos = prev;
            }
        }
        UiAction::TypeNewline => {
            state.input.insert(state.cursor_pos, '\n');
            state.cursor_pos += 1;
        }
        UiAction::TypeIndent => {
            state.input.insert_str(state.cursor_pos, "->");
            state.cursor_pos += 2;
        }
        UiAction::PrependIndent => {
            let line_start = match state.input[..state.cursor_pos].rfind('\n') {
                Some(nl) => nl + 1,
                None => 0,
            };
            state.input.insert_str(line_start, "->");
            state.cursor_pos += 2;
        }
        UiAction::RemoveIndent => {
            let line_start = match state.input[..state.cursor_pos].rfind('\n') {
                Some(nl) => nl + 1,
                None => 0,
            };
            if state.input[line_start..].starts_with("->") {
                state.input.drain(line_start..line_start + 2);
                if state.cursor_pos > line_start {
                    state.cursor_pos = state.cursor_pos.saturating_sub(2).max(line_start);
                }
            }
        }
        UiAction::SubmitInput => {
            let cmd = crate::app::command::parse(&state.input);
            crate::app::actions::dispatch(state, cmd)?;
            state.input.clear();
            state.cursor_pos = 0;
            crate::app::actions::vim_jump_to_new_content(state);
        }
        UiAction::CommitEdit => {
            crate::app::actions::commit_edit(state)?;
        }
        UiAction::MoveCursorLeft => {
            state.cursor_pos = super::prev_char_boundary(&state.input, state.cursor_pos);
        }
        UiAction::MoveCursorRight => {
            state.cursor_pos = super::next_char_boundary(&state.input, state.cursor_pos);
        }
        UiAction::MoveCursorLineStart => {
            let before = &state.input[..state.cursor_pos];
            state.cursor_pos = match before.rfind('\n') {
                Some(nl_pos) => nl_pos + 1,
                None => 0,
            };
        }
        UiAction::MoveCursorLineEnd => {
            let after = &state.input[state.cursor_pos..];
            state.cursor_pos = match after.find('\n') {
                Some(nl_offset) => state.cursor_pos + nl_offset,
                None => state.input.len(),
            };
        }
        UiAction::SelectNext => crate::app::actions::select_next(state),
        UiAction::SelectPrev => crate::app::actions::select_prev(state),
        UiAction::SelectFirst => crate::app::actions::select_first(state),
        UiAction::SelectLast => crate::app::actions::select_last(state),
        UiAction::ToggleSelected => crate::app::actions::toggle_selected(state),
        UiAction::BeginEdit => crate::app::actions::begin_edit_selected(state),
        UiAction::ResumeHeading => crate::app::actions::resume_selected_heading(state),
        _ => unreachable!("capture::execute_action called with non-capture action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 2: Update `execute_action` in `src/app/input/mod.rs` to delegate capture actions**

In `mod.rs`, find the `execute_action` function. After the existing universal action arms (Quit through FocusVimNormal), replace all capture-specific arms with a single delegation:

```rust
        // Capture mode actions
        UiAction::TypeChar(_)
        | UiAction::DeleteChar
        | UiAction::TypeNewline
        | UiAction::TypeIndent
        | UiAction::PrependIndent
        | UiAction::RemoveIndent
        | UiAction::SubmitInput
        | UiAction::CommitEdit
        | UiAction::MoveCursorLeft
        | UiAction::MoveCursorRight
        | UiAction::MoveCursorLineStart
        | UiAction::MoveCursorLineEnd
        | UiAction::SelectNext
        | UiAction::SelectPrev
        | UiAction::SelectFirst
        | UiAction::SelectLast
        | UiAction::ToggleSelected
        | UiAction::BeginEdit
        | UiAction::ResumeHeading => return capture::execute_action(state, action),
```

- [ ] **Step 3: Update `key_to_action` in `src/app/input/mod.rs` to delegate capture keys**

In `mod.rs`, find the `Focus::Capture =>` arm of the mode-specific match (around line 267). Replace the inline match block with:

```rust
        Focus::Capture => capture::key_to_action(state, key),
```

- [ ] **Step 4: Run tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/app/input/mod.rs src/app/input/capture.rs
git commit -m "refactor: extract capture mode handler to input/capture.rs"
```

---

### Task 6: Extract vim_normal mode handler

**Files:**
- Modify: `src/app/input/mod.rs`
- Modify: `src/app/input/vim_normal.rs`

- [ ] **Step 1: Write `src/app/input/vim_normal.rs`**

Replace the stub with:

```rust
use crate::app::state::{AppState, Focus, UndoEntry};
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    if let Some(op) = state.vim.pending_op {
        return match (op, &key.code) {
            ('d', KeyCode::Char('d')) => Some(UiAction::VimDeleteLine),
            ('y', KeyCode::Char('y')) => Some(UiAction::VimYankLine),
            ('g', KeyCode::Char('g')) => Some(UiAction::VimMoveFileStart),
            _ => Some(UiAction::VimClearPendingOp),
        };
    }
    match key.code {
        KeyCode::Char('h') | KeyCode::Left  => Some(UiAction::VimMoveLeft),
        KeyCode::Char('l') | KeyCode::Right => Some(UiAction::VimMoveRight),
        KeyCode::Char('j') | KeyCode::Down  => Some(UiAction::VimMoveDown),
        KeyCode::Char('k') | KeyCode::Up    => Some(UiAction::VimMoveUp),
        KeyCode::Char('w') => Some(UiAction::VimMoveWordForward),
        KeyCode::Char('b') => Some(UiAction::VimMoveWordBackward),
        KeyCode::Char('e') => Some(UiAction::VimMoveWordEnd),
        KeyCode::Char('0') => Some(UiAction::VimMoveLineStart),
        KeyCode::Char('$') => Some(UiAction::VimMoveLineEnd),
        KeyCode::Char('G') => Some(UiAction::VimMoveFileEnd),
        KeyCode::Char('i') => Some(UiAction::VimEnterInsert),
        KeyCode::Char('a') => Some(UiAction::VimEnterInsertAfter),
        KeyCode::Char('A') => Some(UiAction::VimEnterInsertEOL),
        KeyCode::Char('o') => Some(UiAction::VimInsertLineBelow),
        KeyCode::Char('O') => Some(UiAction::VimInsertLineAbove),
        KeyCode::Char('x') => Some(UiAction::VimDeleteChar),
        KeyCode::Char('d') => Some(UiAction::VimSetPendingOp('d')),
        KeyCode::Char('y') => Some(UiAction::VimSetPendingOp('y')),
        KeyCode::Char('g') => Some(UiAction::VimSetPendingOp('g')),
        KeyCode::Char('p') => Some(UiAction::VimPasteBelow),
        KeyCode::Char('P') => Some(UiAction::VimPasteAbove),
        KeyCode::Char('u') => Some(UiAction::VimUndo),
        KeyCode::Char('t') => Some(UiAction::VimToggleTodo),
        KeyCode::Char('?') => Some(UiAction::OpenHelp),
        KeyCode::Tab       => Some(UiAction::SwitchToCapture),
        KeyCode::Esc       => None,
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::VimMoveLeft => {
            let col = state.vim.cursor_col;
            if col > 0 {
                state.vim.cursor_col =
                    super::prev_char_boundary(&state.doc.lines[state.vim.cursor_line], col);
            }
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveRight => {
            let line = &state.doc.lines[state.vim.cursor_line];
            let col = state.vim.cursor_col;
            let next = super::next_char_boundary(line, col);
            if next < line.len() {
                state.vim.cursor_col = next;
            }
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveDown => {
            let n = state.doc.lines.len();
            if state.vim.cursor_line + 1 < n {
                state.vim.cursor_line += 1;
                state.vim.cursor_col = super::vim_clamp_col(
                    &state.doc.lines[state.vim.cursor_line],
                    state.vim.cursor_col,
                );
                crate::app::actions::vim_update_context(state);
            }
        }
        UiAction::VimMoveUp => {
            if state.vim.cursor_line > 0 {
                state.vim.cursor_line -= 1;
                state.vim.cursor_col = super::vim_clamp_col(
                    &state.doc.lines[state.vim.cursor_line],
                    state.vim.cursor_col,
                );
                crate::app::actions::vim_update_context(state);
            }
        }
        UiAction::VimMoveLineStart => {
            state.vim.cursor_col = 0;
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveLineEnd => {
            let line = &state.doc.lines[state.vim.cursor_line];
            state.vim.cursor_col = if line.is_empty() {
                0
            } else {
                super::prev_char_boundary(line, line.len())
            };
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveFileStart => {
            state.vim.pending_op = None;
            state.vim.cursor_line = 0;
            state.vim.cursor_col = 0;
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveFileEnd => {
            let n = state.doc.lines.len();
            state.vim.pending_op = None;
            state.vim.cursor_line = n.saturating_sub(1);
            state.vim.cursor_col = super::vim_clamp_col(
                &state.doc.lines[state.vim.cursor_line],
                state.vim.cursor_col,
            );
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveWordForward => {
            let line = &state.doc.lines[state.vim.cursor_line];
            let new_col = super::next_word_start(line, state.vim.cursor_col);
            state.vim.cursor_col = super::vim_clamp_col(line, new_col);
        }
        UiAction::VimMoveWordBackward => {
            let line = &state.doc.lines[state.vim.cursor_line];
            state.vim.cursor_col = super::prev_word_start(line, state.vim.cursor_col);
        }
        UiAction::VimMoveWordEnd => {
            let line = &state.doc.lines[state.vim.cursor_line];
            state.vim.cursor_col = super::word_end(line, state.vim.cursor_col);
        }
        UiAction::VimSetPendingOp(op) => {
            state.vim.pending_op = Some(op);
        }
        UiAction::VimClearPendingOp => {
            state.vim.pending_op = None;
        }
        UiAction::VimEnterInsert => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            state.focus = Focus::VimInsert;
        }
        UiAction::VimEnterInsertAfter => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            let line = &state.doc.lines[state.vim.cursor_line];
            let next = super::next_char_boundary(line, state.vim.cursor_col);
            state.vim.cursor_col = next.min(line.len());
            state.focus = Focus::VimInsert;
        }
        UiAction::VimEnterInsertEOL => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            state.vim.cursor_col = state.doc.lines[state.vim.cursor_line].len();
            state.focus = Focus::VimInsert;
        }
        UiAction::VimInsertLineBelow => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            let insert_at = state.vim.cursor_line + 1;
            state.doc.lines.insert(insert_at, String::new());
            state.vim.cursor_line = insert_at;
            state.vim.cursor_col = 0;
            state.focus = Focus::VimInsert;
        }
        UiAction::VimInsertLineAbove => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            state.doc.lines.insert(state.vim.cursor_line, String::new());
            state.vim.cursor_col = 0;
            state.focus = Focus::VimInsert;
        }
        UiAction::VimDeleteChar => {
            let line = &state.doc.lines[state.vim.cursor_line];
            if !line.is_empty() {
                let col = state.vim.cursor_col;
                let end = super::next_char_boundary(line, col);
                let line = &mut state.doc.lines[state.vim.cursor_line];
                line.drain(col..end);
                let line_len = state.doc.lines[state.vim.cursor_line].len();
                if col > 0 && col >= line_len {
                    state.vim.cursor_col = super::prev_char_boundary(
                        &state.doc.lines[state.vim.cursor_line],
                        line_len,
                    );
                }
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        UiAction::VimDeleteLine => {
            state.vim.pending_op = None;
            let n = state.doc.lines.len();
            if n > 0 {
                let removed = state.doc.lines.remove(state.vim.cursor_line);
                state.vim.yank_buffer = vec![removed];
                let new_n = state.doc.lines.len();
                if state.vim.cursor_line >= new_n && new_n > 0 {
                    state.vim.cursor_line = new_n - 1;
                }
                if !state.doc.lines.is_empty() {
                    state.vim.cursor_col = super::vim_clamp_col(
                        &state.doc.lines[state.vim.cursor_line],
                        state.vim.cursor_col,
                    );
                }
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        UiAction::VimYankLine => {
            state.vim.pending_op = None;
            let line = state.doc.lines[state.vim.cursor_line].clone();
            state.vim.yank_buffer = vec![line];
        }
        UiAction::VimPasteBelow => {
            if !state.vim.yank_buffer.is_empty() {
                let insert_at = state.vim.cursor_line + 1;
                for (i, line) in state.vim.yank_buffer.clone().into_iter().enumerate() {
                    state.doc.lines.insert(insert_at + i, line);
                }
                state.vim.cursor_line = insert_at;
                state.vim.cursor_col = 0;
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        UiAction::VimPasteAbove => {
            if !state.vim.yank_buffer.is_empty() {
                let insert_at = state.vim.cursor_line;
                for (i, line) in state.vim.yank_buffer.clone().into_iter().enumerate() {
                    state.doc.lines.insert(insert_at + i, line);
                }
                state.vim.cursor_col = 0;
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        UiAction::VimUndo => {
            if let Some(entry) = state.vim.undo_stack.pop() {
                state.doc.lines = entry.lines;
                state.vim.cursor_line = entry.cursor_line;
                state.vim.cursor_col = entry.cursor_col;
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        UiAction::VimToggleTodo => {
            let line_idx = state.vim.cursor_line;
            if state.doc.toggle_todo_at_line(line_idx).is_ok() {
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        _ => unreachable!("vim_normal::execute_action called with non-vim-normal action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 2: Update `mod.rs` to delegate vim_normal actions**

In `execute_action` in `mod.rs`, replace the vim_normal inline arms with:

```rust
        UiAction::VimMoveLeft
        | UiAction::VimMoveRight
        | UiAction::VimMoveUp
        | UiAction::VimMoveDown
        | UiAction::VimMoveLineStart
        | UiAction::VimMoveLineEnd
        | UiAction::VimMoveFileStart
        | UiAction::VimMoveFileEnd
        | UiAction::VimMoveWordForward
        | UiAction::VimMoveWordBackward
        | UiAction::VimMoveWordEnd
        | UiAction::VimSetPendingOp(_)
        | UiAction::VimClearPendingOp
        | UiAction::VimEnterInsert
        | UiAction::VimEnterInsertAfter
        | UiAction::VimEnterInsertEOL
        | UiAction::VimInsertLineBelow
        | UiAction::VimInsertLineAbove
        | UiAction::VimDeleteChar
        | UiAction::VimDeleteLine
        | UiAction::VimYankLine
        | UiAction::VimPasteBelow
        | UiAction::VimPasteAbove
        | UiAction::VimUndo
        | UiAction::VimToggleTodo => return vim_normal::execute_action(state, action),
```

In `key_to_action` in `mod.rs`, replace the `Focus::VimNormal => { ... }` arm with:

```rust
        Focus::VimNormal => vim_normal::key_to_action(state, key),
```

- [ ] **Step 3: Run tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/app/input/mod.rs src/app/input/vim_normal.rs
git commit -m "refactor: extract vim normal mode handler to input/vim_normal.rs"
```

---

### Task 7: Extract vim_insert mode handler

**Files:**
- Modify: `src/app/input/mod.rs`
- Modify: `src/app/input/vim_insert.rs`

- [ ] **Step 1: Write `src/app/input/vim_insert.rs`**

```rust
use crate::app::state::{AppState, Focus};
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Esc       => Some(UiAction::VimExitInsert),
        KeyCode::Enter     => Some(UiAction::VimInsertNewline),
        KeyCode::Backspace => Some(UiAction::VimInsertBackspace),
        KeyCode::Left      => Some(UiAction::VimMoveLeft),
        KeyCode::Right     => Some(UiAction::VimMoveRight),
        KeyCode::Up        => Some(UiAction::VimMoveUp),
        KeyCode::Down      => Some(UiAction::VimMoveDown),
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::VimInsertDeleteWordBefore)
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
        {
            Some(UiAction::VimInsertChar(c))
        }
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::VimExitInsert => {
            let col = state.vim.cursor_col;
            let line = &state.doc.lines[state.vim.cursor_line];
            if col > 0 {
                state.vim.cursor_col = super::prev_char_boundary(line, col);
            }
            state.vim.cursor_col = super::vim_clamp_col(
                &state.doc.lines[state.vim.cursor_line],
                state.vim.cursor_col,
            );
            state.vim.pending_op = None;
            state.focus = Focus::VimNormal;
            let _ = crate::app::actions::after_vim_edit(state);
        }
        UiAction::VimInsertChar(c) => {
            let line = &mut state.doc.lines[state.vim.cursor_line];
            line.insert(state.vim.cursor_col, c);
            state.vim.cursor_col += c.len_utf8();
        }
        UiAction::VimInsertNewline => {
            let tail =
                state.doc.lines[state.vim.cursor_line][state.vim.cursor_col..].to_string();
            state.doc.lines[state.vim.cursor_line].truncate(state.vim.cursor_col);
            state.vim.cursor_line += 1;
            state.doc.lines.insert(state.vim.cursor_line, tail);
            state.vim.cursor_col = 0;
        }
        UiAction::VimInsertBackspace => {
            let col = state.vim.cursor_col;
            if col > 0 {
                let prev =
                    super::prev_char_boundary(&state.doc.lines[state.vim.cursor_line], col);
                state.doc.lines[state.vim.cursor_line].remove(prev);
                state.vim.cursor_col = prev;
            } else if state.vim.cursor_line > 0 {
                let current = state.doc.lines.remove(state.vim.cursor_line);
                state.vim.cursor_line -= 1;
                let prev_len = state.doc.lines[state.vim.cursor_line].len();
                state.doc.lines[state.vim.cursor_line].push_str(&current);
                state.vim.cursor_col = prev_len;
            }
        }
        UiAction::VimInsertDeleteWordBefore => {
            let col = state.vim.cursor_col;
            let new_col =
                super::prev_word_start(&state.doc.lines[state.vim.cursor_line], col);
            let line = &mut state.doc.lines[state.vim.cursor_line];
            line.drain(new_col..col);
            state.vim.cursor_col = new_col;
        }
        _ => unreachable!("vim_insert::execute_action called with non-vim-insert action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 2: Update `mod.rs` to delegate vim_insert actions**

In `execute_action`, replace the vim_insert inline arms with:

```rust
        UiAction::VimInsertChar(_)
        | UiAction::VimInsertNewline
        | UiAction::VimInsertBackspace
        | UiAction::VimInsertDeleteWordBefore
        | UiAction::VimExitInsert => return vim_insert::execute_action(state, action),
```

In `key_to_action`, replace `Focus::VimInsert => { ... }` with:

```rust
        Focus::VimInsert => vim_insert::key_to_action(state, key),
```

- [ ] **Step 3: Run tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/app/input/mod.rs src/app/input/vim_insert.rs
git commit -m "refactor: extract vim insert mode handler to input/vim_insert.rs"
```

---

### Task 8: Extract right_panel mode handler

**Files:**
- Modify: `src/app/input/mod.rs`
- Modify: `src/app/input/right_panel.rs`

- [ ] **Step 1: Write `src/app/input/right_panel.rs`**

```rust
use crate::app::state::AppState;
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(UiAction::RightPanelDown),
        KeyCode::Up   | KeyCode::Char('k') => Some(UiAction::RightPanelUp),
        KeyCode::Char(' ') | KeyCode::Char('x') => Some(UiAction::RightPanelToggle),
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::RightPanelUp => {
            if state.right_panel_selected > 0 {
                state.right_panel_selected -= 1;
            }
        }
        UiAction::RightPanelDown => {
            let max = state.panel_todos.len().saturating_sub(1);
            if state.right_panel_selected < max {
                state.right_panel_selected += 1;
            }
        }
        UiAction::RightPanelToggle => {
            crate::app::actions::toggle_panel_todo(state)?;
        }
        _ => unreachable!("right_panel::execute_action called with non-right-panel action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 2: Update `mod.rs` to delegate right_panel actions**

In `execute_action`, replace the `RightPanelUp / RightPanelDown / RightPanelToggle` arms with:

```rust
        UiAction::RightPanelUp
        | UiAction::RightPanelDown
        | UiAction::RightPanelToggle => return right_panel::execute_action(state, action),
```

In `key_to_action`, replace `Focus::RightPanel => match key.code { ... }` with:

```rust
        Focus::RightPanel => right_panel::key_to_action(state, key),
```

- [ ] **Step 3: Run tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/app/input/mod.rs src/app/input/right_panel.rs
git commit -m "refactor: extract right panel handler to input/right_panel.rs"
```

---

### Task 9: Extract chat mode handler

**Files:**
- Modify: `src/app/input/mod.rs`
- Modify: `src/app/input/chat.rs`

- [ ] **Step 1: Write `src/app/input/chat.rs`**

```rust
use crate::app::state::AppState;
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(UiAction::ChatScrollDown),
        KeyCode::Up   | KeyCode::Char('k') => Some(UiAction::ChatScrollUp),
        KeyCode::PageDown => Some(UiAction::ChatPageDown),
        KeyCode::PageUp   => Some(UiAction::ChatPageUp),
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
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
        _ => unreachable!("chat::execute_action called with non-chat action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 2: Update `mod.rs` to delegate chat actions**

In `execute_action`, replace the `ChatScrollUp / ChatScrollDown / ChatPageUp / ChatPageDown` arms with:

```rust
        UiAction::ChatScrollUp
        | UiAction::ChatScrollDown
        | UiAction::ChatPageUp
        | UiAction::ChatPageDown => return chat::execute_action(state, action),
```

In `key_to_action`, replace `Focus::Chat => match key.code { ... }` with:

```rust
        Focus::Chat => chat::key_to_action(state, key),
```

- [ ] **Step 3: Run tests**

```bash
cargo test
```

Expected: all tests pass, same count as Task 1 baseline.

- [ ] **Step 4: Final check — verify `mod.rs` has no remaining large inline match arms**

At this point `execute_action` in `mod.rs` should contain only:
- Universal/focus-transition action arms (Quit, GoToday, PrevDay, NextDay, CloseHelp, OpenHelp, ExitCaptureMode, ExitVimNormal, CancelEdit, SwitchToCapture, FocusVimNormal, FocusRightPanel, RightPanelBlur, ToggleChat, FocusChat, ChatBlur, OpenHelp)
- Five delegation lines (one per mode)

And `key_to_action` should contain only:
- Universal guards (Ctrl-C, Help overlay, global Ctrl hotkeys)
- Cross-mode key handlers (Tab/BackTab, Esc, `[`/`]` navigation)
- Five delegation lines (one per focus)

If any inline match arms remain that should be delegated, move them now.

- [ ] **Step 5: Commit**

```bash
git add src/app/input/mod.rs src/app/input/chat.rs
git commit -m "refactor: extract chat handler to input/chat.rs, complete input module split"
```

---

## Completion Checklist

After all tasks:

- [ ] `cargo test` passes with the same count as the Task 1 baseline
- [ ] `src/app/input/mod.rs` is under 200 lines
- [ ] `src/app/input/capture.rs`, `vim_normal.rs`, `vim_insert.rs`, `right_panel.rs`, `chat.rs` each exist and compile
- [ ] `src/app/context.rs` exists with `context_at_line` and four helper functions
- [ ] `src/app/state.rs` no longer contains `context_at_line`
- [ ] `src/app/actions.rs` `dispatch` function is under 25 lines
- [ ] No behavior changes: zero test modifications except moving the 8 `cursor_*` tests from `state.rs` to `context.rs`
