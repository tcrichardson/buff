# Vim Notes Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Navigate mode in the notes panel with a vim-style modal editor providing hjkl/arrow navigation, insert mode text editing, dd/yy/p, undo, and automatic context derivation from cursor position.

**Architecture:** A `VimState` struct is added to `AppState` tracking cursor position, pending multi-key op, yank buffer, and undo stack. `Focus::Navigate` is renamed to `Focus::VimNormal` and `Focus::VimInsert` is added. A `context_at_line()` pure function derives context from cursor position and is called on every cursor move. The notes panel renders the cursor line as raw markdown (for column-exact cursor positioning) while rendering all other lines formatted, plus a one-line mode indicator at the bottom.

**Tech Stack:** Rust, ratatui, crossterm. No new dependencies required.

**Spec:** `docs/superpowers/specs/2026-06-08-vim-notes-editor-design.md`

---

### Task 1: Add VimState, UndoEntry, and Context::Todos

**Files:**
- Modify: `src/app/state.rs`
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Write failing test for Context::Todos in update_context_display**

Add to the `#[cfg(test)]` block in `src/app/state.rs`:

```rust
#[test]
fn context_todos_display() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = AppState::open_day(
        tmp.path().to_path_buf(),
        Config::default(),
        NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
    )
    .unwrap();
    s.context = Context::Todos;
    s.update_context_display();
    assert!(
        s.context_display.contains("To-do"),
        "expected To-do in display, got: {}",
        s.context_display
    );
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test context_todos_display
```
Expected: compile error — `Context::Todos` does not exist.

- [ ] **Step 3: Add Context::Todos variant**

In `src/app/state.rs`, change the `Context` enum to:

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
    Section { heading_line: usize, level: u8 },
    Todos,
}
```

- [ ] **Step 4: Handle Context::Todos in update_context_display**

In `src/app/state.rs`, add a new arm to `update_context_display`:

```rust
Context::Todos => "context: To-dos (use /todo to add)".to_string(),
```

The full match now has 5 arms: `Notes`, `Meeting`, `NoteBlock`, `Section`, `Todos`.

- [ ] **Step 5: Add VimState and UndoEntry structs**

In `src/app/state.rs`, add before the `AppState` struct definition:

```rust
#[derive(Clone, Debug)]
pub struct UndoEntry {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

#[derive(Clone, Debug, Default)]
pub struct VimState {
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub pending_op: Option<char>,
    pub yank_buffer: Vec<String>,
    pub undo_stack: Vec<UndoEntry>,
}
```

- [ ] **Step 6: Add vim field to AppState**

In `src/app/state.rs`, add `pub vim: VimState,` to the `AppState` struct after `pub chat: ChatState,`.

- [ ] **Step 7: Initialize vim in open_day**

In `AppState::open_day`, add `vim: VimState::default(),` to the `Ok(Self { ... })` block (alongside the other fields).

- [ ] **Step 7b: Fix test helpers that construct AppState directly**

`src/ui/layout.rs` and `src/ui/document.rs` both have a `test_app()` helper that constructs `AppState` with a struct literal. After adding the `vim` field, these fail to compile. Add `vim: crate::app::state::VimState::default(),` to each struct literal in both helpers:

In `src/ui/layout.rs` `test_app`:
```rust
AppState {
    // ... all existing fields unchanged ...
    vim: crate::app::state::VimState::default(),
    chat: crate::app::state::ChatState::default(),
}
```

In `src/ui/document.rs` `test_app` (same change).

Run `cargo build` after this step to confirm compilation succeeds before the tests.

- [ ] **Step 8: Handle Context::Todos in actions.rs dispatch**

In `src/app/actions.rs`, the `Command::Entry` arm has a `match &state.context` that does not yet handle `Todos`. Change the match to:

```rust
let target = match &state.context {
    Context::Notes | Context::Todos => EntryTarget::Notes,
    Context::Meeting(ord) => EntryTarget::Meeting(*ord),
    Context::NoteBlock(ord) => EntryTarget::NoteBlock(*ord),
    Context::Section { heading_line, level } => {
        EntryTarget::Section { heading_line: *heading_line, level: *level }
    }
};
```

Also handle `Context::Todos` in the `Command::Todo` arm's meeting-name lookup (the existing `_ => None` arm already covers it, so no change needed there).

- [ ] **Step 9: Run tests**

```bash
cargo test
```
Expected: all existing tests pass, `context_todos_display` passes.

- [ ] **Step 10: Commit**

```bash
git add src/app/state.rs src/app/actions.rs
git commit -m "feat: add VimState, UndoEntry, and Context::Todos"
```

---

### Task 2: Add context_at_line() — pure context derivation

**Files:**
- Modify: `src/app/state.rs`

- [ ] **Step 1: Write failing tests**

Add to `#[cfg(test)]` in `src/app/state.rs`:

```rust
#[cfg(test)]
mod context_tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(|l| l.to_string()).collect()
    }

    #[test]
    fn cursor_above_all_sections_is_notes() {
        let ls = lines("# 2026-06-08 (Mon)\n");
        assert_eq!(context_at_line(&ls, 0), Context::Notes);
    }

    #[test]
    fn cursor_in_meetings_no_heading_is_notes() {
        let ls = lines("# Day\n\n## Meetings\n\n## Notes\n");
        // line 3 is blank inside ## Meetings
        assert_eq!(context_at_line(&ls, 3), Context::Notes);
    }

    #[test]
    fn cursor_on_meeting_heading_is_meeting_0() {
        let ls = lines("# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n");
        // line 4 is "### Standup"
        assert_eq!(context_at_line(&ls, 4), Context::Meeting(0));
    }

    #[test]
    fn cursor_in_second_meeting_is_meeting_1() {
        let ls = lines(
            "# Day\n\n## Meetings\n\n### Standup\n- note\n\n### Design Review\n\n## Notes\n",
        );
        // line 7 is "### Design Review"
        assert_eq!(context_at_line(&ls, 7), Context::Meeting(1));
    }

    #[test]
    fn cursor_in_section_under_meeting() {
        let ls = lines("# Day\n\n## Meetings\n\n### Standup\n\n#### Action Items\n\n## Notes\n");
        // line 6 is "#### Action Items"
        assert_eq!(
            context_at_line(&ls, 6),
            Context::Section { heading_line: 6, level: 4 }
        );
    }

    #[test]
    fn cursor_in_todos_section() {
        let ls = lines("# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n\n- [ ] task\n");
        // line 8 is "- [ ] task"
        assert_eq!(context_at_line(&ls, 8), Context::Todos);
    }

    #[test]
    fn cursor_in_note_block() {
        let ls = lines("# Day\n\n## Meetings\n\n## Notes\n\n### My Note\n\n## To-dos\n");
        // line 6 is "### My Note"
        assert_eq!(context_at_line(&ls, 6), Context::NoteBlock(0));
    }

    #[test]
    fn cursor_on_empty_lines_vec() {
        assert_eq!(context_at_line(&[], 0), Context::Notes);
    }
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test context_tests
```
Expected: compile error — `context_at_line` does not exist.

- [ ] **Step 3: Implement context_at_line**

Add to `src/app/state.rs` (outside the `impl AppState` block, as a standalone function):

```rust
/// Derive the editing context from a cursor position in the document.
/// Used to update `state.context` automatically as the vim cursor moves.
pub fn context_at_line(lines: &[String], cursor_line: usize) -> Context {
    if lines.is_empty() || cursor_line >= lines.len() {
        return Context::Notes;
    }

    // Step 1: walk backward to find the enclosing ## section boundary
    let boundary = match (0..=cursor_line).rev().find(|&i| lines[i].starts_with("## ")) {
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

    // Step 2: walk forward from boundary to cursor_line tracking headings
    let mut l3_line: Option<usize> = None; // nearest ### heading
    let mut l4_line: Option<usize> = None; // nearest #### or deeper
    let mut l4_level: u8 = 0;

    for i in (boundary + 1)..=cursor_line {
        let line = &lines[i];
        if line.starts_with("## ") {
            break; // hit another top-level section — stop
        } else if line.starts_with("### ") {
            l3_line = Some(i);
            l4_line = None; // reset sub-section on new L3 heading
            l4_level = 0;
        } else if line.starts_with("#### ")
            || line.starts_with("##### ")
            || line.starts_with("###### ")
        {
            if l3_line.is_some() {
                l4_level = line.chars().take_while(|&c| c == '#').count() as u8;
                l4_line = Some(i);
            }
        }
    }

    // Step 3: return most specific context found
    if let Some(l4) = l4_line {
        return Context::Section { heading_line: l4, level: l4_level };
    }

    if let Some(l3) = l3_line {
        // Ordinal = number of ### headings from section start to l3, 0-indexed
        let ordinal = lines[(boundary + 1)..=l3]
            .iter()
            .filter(|l| l.starts_with("### "))
            .count()
            .saturating_sub(1);
        return if in_meetings {
            Context::Meeting(ordinal)
        } else {
            Context::NoteBlock(ordinal)
        };
    }

    Context::Notes
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test context_tests
```
Expected: all 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/app/state.rs
git commit -m "feat: add context_at_line() for cursor-derived context"
```

---

### Task 3: Add toggle_todo_at_line() to Document

**Files:**
- Modify: `src/model/writer.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` block in `src/model/writer.rs` (or in the test module at the bottom — follow the existing pattern in the file):

```rust
#[test]
fn toggle_todo_at_line_unchecked_to_checked() {
    let mut doc = Document::from_text(
        "# Day\n\n## To-dos\n\n- [ ] write tests\n",
    );
    // line 4 is "- [ ] write tests"
    doc.toggle_todo_at_line(4).unwrap();
    assert_eq!(doc.lines[4], "- [x] write tests");
}

#[test]
fn toggle_todo_at_line_checked_to_unchecked() {
    let mut doc = Document::from_text(
        "# Day\n\n## To-dos\n\n- [x] done task\n",
    );
    doc.toggle_todo_at_line(4).unwrap();
    assert_eq!(doc.lines[4], "- [ ] done task");
}

#[test]
fn toggle_todo_at_line_uppercase_x_to_unchecked() {
    let mut doc = Document::from_text(
        "# Day\n\n## To-dos\n\n- [X] done task\n",
    );
    doc.toggle_todo_at_line(4).unwrap();
    assert_eq!(doc.lines[4], "- [ ] done task");
}

#[test]
fn toggle_todo_at_line_non_todo_returns_err() {
    let mut doc = Document::from_text("# Day\n\n## Notes\n\n- just a bullet\n");
    let result = doc.toggle_todo_at_line(4);
    assert!(result.is_err(), "expected error for non-todo line");
}

#[test]
fn toggle_todo_at_line_out_of_bounds_returns_err() {
    let mut doc = Document::from_text("# Day\n");
    let result = doc.toggle_todo_at_line(99);
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test toggle_todo_at_line
```
Expected: compile error — `toggle_todo_at_line` does not exist.

- [ ] **Step 3: Implement toggle_todo_at_line**

In `src/model/writer.rs`, add this method to the `impl Document` block, after the existing `toggle_todo` method:

```rust
/// Toggle the todo on a specific line by raw line index.
/// Returns Ok(()) if the line was a todo, Err if it was not or index is out of bounds.
pub fn toggle_todo_at_line(&mut self, line_idx: usize) -> anyhow::Result<()> {
    let line = self
        .lines
        .get(line_idx)
        .ok_or_else(|| anyhow::anyhow!("line index {} out of bounds", line_idx))?;
    if line.starts_with("- [ ] ") {
        let rest = line[6..].to_string();
        self.lines[line_idx] = format!("- [x] {}", rest);
        Ok(())
    } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
        let rest = line[6..].to_string();
        self.lines[line_idx] = format!("- [ ] {}", rest);
        Ok(())
    } else {
        Err(anyhow::anyhow!("line {} is not a todo", line_idx))
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test toggle_todo_at_line
```
Expected: all 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/model/writer.rs
git commit -m "feat: add toggle_todo_at_line() to Document"
```

---

### Task 4: Rename Focus::Navigate → VimNormal, add VimInsert, remove pending_delete

This is a mechanical refactor — rename to keep the app compiling before replacing behaviour in Task 5.

**Files:**
- Modify: `src/app/state.rs`
- Modify: `src/app/input.rs`
- Modify: `src/ui/layout.rs`

- [ ] **Step 1: Update Focus enum in state.rs**

Replace the `Focus` enum in `src/app/state.rs`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Capture,
    VimNormal,   // was Navigate
    VimInsert,   // new
    RightPanel,
    Chat,
}
```

Remove `pub pending_delete: bool,` from the `AppState` struct.

Remove `pending_delete: false,` from `AppState::open_day`.

- [ ] **Step 2: Update input.rs — rename Navigate references**

In `src/app/input.rs`, do all of the following:

a) Rename `ExitNavigateMode` to `ExitVimNormal` in the `UiAction` enum.

b) Rename `FocusNavigate` to `FocusVimNormal` in the `UiAction` enum.

c) Remove `pending_delete`-related `UiAction` variants: `InitiateDelete`, `ConfirmDelete`, `CancelDelete`. (They will be replaced by vim ops in Task 5.)

d) In `key_to_action`, Tab handling — change `Focus::Navigate` arms to `Focus::VimNormal`:

```rust
if key.code == KeyCode::Tab {
    return match state.focus {
        Focus::Capture => Some(UiAction::TypeIndent),
        Focus::VimNormal | Focus::VimInsert => Some(UiAction::SwitchToCapture),
        Focus::Chat => Some(UiAction::FocusRightPanel),
        Focus::RightPanel => Some(UiAction::FocusVimNormal),
    };
}

if key.code == KeyCode::BackTab {
    return match state.focus {
        Focus::Capture => Some(UiAction::RemoveIndent),
        Focus::VimNormal | Focus::VimInsert => Some(UiAction::FocusRightPanel),
        Focus::Chat => Some(UiAction::FocusVimNormal),
        Focus::RightPanel => {
            if state.chat.visible {
                Some(UiAction::FocusChat)
            } else {
                Some(UiAction::FocusVimNormal)
            }
        }
    };
}
```

e) In Esc handling, rename the Navigate arm:

```rust
Focus::VimNormal | Focus::VimInsert => Some(UiAction::ExitVimNormal),
```
(This is a temporary placeholder — Task 5 will split VimNormal and VimInsert Esc handling properly.)

f) In `can_navigate` check:
```rust
let can_navigate = matches!(state.focus, Focus::VimNormal)
    || (matches!(state.focus, Focus::Capture) && state.input.is_empty());
```

g) In the mode-specific match at the bottom, rename `Focus::Navigate` to `Focus::VimNormal`. Remove the `pending_delete` guard block inside it. Keep all the key mappings intact for now (they use the old Navigate actions as placeholders; Task 5 will replace them).

h) In `execute_action`, rename:
- `UiAction::ExitNavigateMode` (now `ExitVimNormal`) → `state.focus = Focus::Capture; state.pending_delete = false;` becomes `state.focus = Focus::Capture;`
- `UiAction::FocusNavigate` (now `FocusVimNormal`) → `state.focus = Focus::VimNormal;`
- `UiAction::ExitCaptureMode` → `state.focus = Focus::VimNormal;` (was Navigate)
- `UiAction::SwitchToCapture` → `state.focus = Focus::Capture;` (remove `state.pending_delete = false;`)
- Remove the `InitiateDelete`, `ConfirmDelete`, `CancelDelete` execute arms.

- [ ] **Step 3: Update layout.rs**

In `src/ui/layout.rs`, change the `notes_focused` line:

```rust
let notes_focused = matches!(app.focus, Focus::Capture | Focus::VimNormal | Focus::VimInsert);
```

- [ ] **Step 4: Fix all remaining compile errors**

Run:
```bash
cargo build 2>&1 | head -60
```
Fix any remaining references to `Focus::Navigate`, `pending_delete`, `InitiateDelete`, `ConfirmDelete`, `CancelDelete`, `ExitNavigateMode`, `FocusNavigate` in the codebase. Common locations: `layout.rs` test helpers, `input.rs` tests.

- [ ] **Step 5: Update tests in input.rs**

Tests that reference `Focus::Navigate` must be updated to `Focus::VimNormal`. Tests that reference `pending_delete` must be updated or removed (the `InitiateDelete`/`ConfirmDelete` tests become obsolete — remove them). Tests that check `ExitCaptureMode` switches to `Focus::Navigate` must now assert `Focus::VimNormal`.

Key tests to update:
- `exit_capture_mode_switches_focus_to_navigate` → assert `Focus::VimNormal`
- `exit_navigate_mode_switches_focus_to_capture_and_clears_pending` → rename, remove pending_delete assertion
- `navigate_j_selects_next` → update `state.focus = Focus::VimNormal`
- etc.

- [ ] **Step 6: Run all tests**

```bash
cargo test
```
Expected: all tests pass. (Navigate-mode behaviour is still intact under `VimNormal` name.)

- [ ] **Step 7: Commit**

```bash
git add src/app/state.rs src/app/input.rs src/ui/layout.rs
git commit -m "refactor: rename Focus::Navigate to VimNormal, add VimInsert, remove pending_delete"
```

---

### Task 5: VimNormal key bindings

Replace the Navigate key map with the full vim normal mode key map.

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Add vim UiAction variants**

In `src/app/input.rs`, add to the `UiAction` enum — remove all the old Navigate-mode variants (`SelectNext`, `SelectPrev`, `SelectFirst`, `SelectLast`, `ToggleSelected`, `BeginEdit`, `ResumeHeading`) and replace with:

```rust
// VimNormal actions
VimMoveLeft,
VimMoveRight,
VimMoveUp,
VimMoveDown,
VimMoveWordForward,
VimMoveWordBackward,
VimMoveWordEnd,
VimMoveLineStart,
VimMoveLineEnd,
VimMoveFileStart,
VimMoveFileEnd,
VimSetPendingOp(char),
VimClearPendingOp,
VimEnterInsert,
VimEnterInsertAfter,
VimEnterInsertEOL,
VimInsertLineBelow,
VimInsertLineAbove,
VimDeleteChar,
VimDeleteLine,
VimYankLine,
VimPasteBelow,
VimPasteAbove,
VimUndo,
VimToggleTodo,
// VimInsert actions
VimInsertChar(char),
VimInsertNewline,
VimInsertBackspace,
VimInsertDeleteWordBefore,
VimExitInsert,
```

- [ ] **Step 2: Replace VimNormal key_to_action arm**

Replace the `Focus::VimNormal` match arm in `key_to_action`:

```rust
Focus::VimNormal => {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    // Multi-key pending op
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
```

- [ ] **Step 3: Add VimInsert key_to_action arm**

Add after the VimNormal arm:

```rust
Focus::VimInsert => {
    match key.code {
        KeyCode::Esc     => Some(UiAction::VimExitInsert),
        KeyCode::Enter   => Some(UiAction::VimInsertNewline),
        KeyCode::Backspace => Some(UiAction::VimInsertBackspace),
        KeyCode::Left    => Some(UiAction::VimMoveLeft),
        KeyCode::Right   => Some(UiAction::VimMoveRight),
        KeyCode::Up      => Some(UiAction::VimMoveUp),
        KeyCode::Down    => Some(UiAction::VimMoveDown),
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
```

- [ ] **Step 4: Update Esc handling for separate modes**

In the Esc section of `key_to_action`, replace the combined arm with separate arms:

```rust
if key.code == KeyCode::Esc {
    return match state.focus {
        Focus::Capture => {
            if state.editing.is_some() {
                Some(UiAction::CancelEdit)
            } else {
                Some(UiAction::ExitCaptureMode)
            }
        }
        Focus::VimNormal => None, // Esc is no-op in normal mode
        Focus::VimInsert => Some(UiAction::VimExitInsert),
        Focus::RightPanel => Some(UiAction::RightPanelBlur),
        Focus::Chat => Some(UiAction::ChatBlur),
    };
}
```

- [ ] **Step 5: Add execute_action stubs for vim actions**

Add stub arms in `execute_action` so the code compiles (full logic comes in Task 6/7):

```rust
UiAction::VimMoveLeft
| UiAction::VimMoveRight
| UiAction::VimMoveUp
| UiAction::VimMoveDown
| UiAction::VimMoveWordForward
| UiAction::VimMoveWordBackward
| UiAction::VimMoveWordEnd
| UiAction::VimMoveLineStart
| UiAction::VimMoveLineEnd
| UiAction::VimMoveFileStart
| UiAction::VimMoveFileEnd
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
| UiAction::VimToggleTodo
| UiAction::VimInsertChar(_)
| UiAction::VimInsertNewline
| UiAction::VimInsertBackspace
| UiAction::VimInsertDeleteWordBefore
| UiAction::VimExitInsert => { /* implemented in Task 6 & 7 */ }
```

- [ ] **Step 6: Write key mapping tests**

Add to `#[cfg(test)]` in `input.rs`:

```rust
#[test]
fn vimnormal_h_moves_left() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char('h'))),
        Some(UiAction::VimMoveLeft)
    );
}

#[test]
fn vimnormal_arrow_left_moves_left() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Left)),
        Some(UiAction::VimMoveLeft)
    );
}

#[test]
fn vimnormal_dd_with_pending_deletes_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.vim.pending_op = Some('d');
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char('d'))),
        Some(UiAction::VimDeleteLine)
    );
}

#[test]
fn vimnormal_gg_with_pending_moves_file_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.vim.pending_op = Some('g');
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char('g'))),
        Some(UiAction::VimMoveFileStart)
    );
}

#[test]
fn vimnormal_pending_op_unknown_second_key_clears() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.vim.pending_op = Some('d');
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char('x'))),
        Some(UiAction::VimClearPendingOp)
    );
}

#[test]
fn vimnormal_esc_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    assert_eq!(key_to_action(&state, make_key(KeyCode::Esc)), None);
}

#[test]
fn viminsert_esc_exits_insert() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimInsert;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Esc)),
        Some(UiAction::VimExitInsert)
    );
}

#[test]
fn viminsert_char_emits_insert_char() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimInsert;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char('a'))),
        Some(UiAction::VimInsertChar('a'))
    );
}

#[test]
fn viminsert_arrow_right_moves_right() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimInsert;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Right)),
        Some(UiAction::VimMoveRight)
    );
}

#[test]
fn vimnormal_tab_switches_to_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::SwitchToCapture)
    );
}
```

- [ ] **Step 7: Run tests**

```bash
cargo test
```
Expected: all new tests pass. App compiles.

- [ ] **Step 8: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: add VimNormal and VimInsert key bindings"
```

---

### Task 6: Implement cursor movement execute_action handlers

**Files:**
- Modify: `src/app/input.rs`
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Add vim_update_context helper to actions.rs**

Add to `src/app/actions.rs`:

```rust
/// Re-derive context from the current vim cursor position and update state.context + display.
pub fn vim_update_context(state: &mut AppState) {
    use crate::app::state::context_at_line;
    state.context = context_at_line(&state.doc.lines, state.vim.cursor_line);
    state.update_context_display();
}
```

- [ ] **Step 2: Add char boundary helpers to input.rs**

Add near the top of `src/app/input.rs` (these complement the existing `prev_char_boundary` / `next_char_boundary` functions that already exist for capture-mode cursor math):

```rust
/// Clamp `col` to a valid char boundary within `line`, respecting vim normal-mode
/// convention that the cursor must land on a character (not past the last one).
fn vim_clamp_col(line: &str, col: usize) -> usize {
    if line.is_empty() {
        return 0;
    }
    let max = prev_char_boundary(line, line.len()); // byte offset of last char start
    let col = col.min(max);
    // walk backward to a valid boundary
    let mut c = col;
    while c > 0 && !line.is_char_boundary(c) {
        c -= 1;
    }
    c
}
```

- [ ] **Step 3: Replace stub execute_action arms with cursor movement**

Replace the large stub arm from Task 5 with separate arms. Start with the movement actions (cursor-only, no insert-mode transitions yet):

```rust
UiAction::VimMoveLeft => {
    let col = state.vim.cursor_col;
    if col > 0 {
        state.vim.cursor_col = prev_char_boundary(
            &state.doc.lines[state.vim.cursor_line],
            col,
        );
    }
}
UiAction::VimMoveRight => {
    let line = &state.doc.lines[state.vim.cursor_line];
    let col = state.vim.cursor_col;
    let next = next_char_boundary(line, col);
    // Normal mode: cannot move past last character
    if next < line.len() {
        state.vim.cursor_col = next;
    }
    crate::app::actions::vim_update_context(state);
}
UiAction::VimMoveDown => {
    let n = state.doc.lines.len();
    if state.vim.cursor_line + 1 < n {
        state.vim.cursor_line += 1;
        state.vim.cursor_col = vim_clamp_col(
            &state.doc.lines[state.vim.cursor_line],
            state.vim.cursor_col,
        );
        crate::app::actions::vim_update_context(state);
    }
}
UiAction::VimMoveUp => {
    if state.vim.cursor_line > 0 {
        state.vim.cursor_line -= 1;
        state.vim.cursor_col = vim_clamp_col(
            &state.doc.lines[state.vim.cursor_line],
            state.vim.cursor_col,
        );
        crate::app::actions::vim_update_context(state);
    }
}
UiAction::VimMoveLineStart => {
    state.vim.cursor_col = 0;
}
UiAction::VimMoveLineEnd => {
    let line = &state.doc.lines[state.vim.cursor_line];
    state.vim.cursor_col = if line.is_empty() {
        0
    } else {
        prev_char_boundary(line, line.len())
    };
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
    state.vim.cursor_col = vim_clamp_col(
        &state.doc.lines[state.vim.cursor_line],
        state.vim.cursor_col,
    );
    crate::app::actions::vim_update_context(state);
}
UiAction::VimSetPendingOp(op) => {
    state.vim.pending_op = Some(op);
}
UiAction::VimClearPendingOp => {
    state.vim.pending_op = None;
}
```

- [ ] **Step 4: Implement word motion helpers and actions**

Add a helper function in `src/app/input.rs`:

```rust
/// Find the byte offset of the start of the next word on `line` from `col`.
fn next_word_start(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line[col..].chars().collect();
    let mut i = 0;
    // skip current word chars (non-whitespace)
    while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
    // skip whitespace
    while i < chars.len() && chars[i].is_whitespace() { i += 1; }
    col + chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>()
}

/// Find the byte offset of the start of the previous word on `line` from `col`.
fn prev_word_start(line: &str, col: usize) -> usize {
    let before: Vec<char> = line[..col].chars().collect();
    let mut i = before.len();
    // skip whitespace going backward
    while i > 0 && before[i - 1].is_whitespace() { i -= 1; }
    // skip word chars going backward
    while i > 0 && !before[i - 1].is_whitespace() { i -= 1; }
    before[..i].iter().map(|c| c.len_utf8()).sum::<usize>()
}

/// Find the byte offset of the end of the current/next word on `line` from `col`.
fn word_end(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line[col..].chars().collect();
    let mut i = 0;
    // skip one char if at a non-whitespace (to find NEXT end)
    if i < chars.len() && !chars[i].is_whitespace() { i += 1; }
    // skip whitespace
    while i < chars.len() && chars[i].is_whitespace() { i += 1; }
    // skip non-whitespace to end of word
    while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
    let end_byte = col + chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>();
    // position on last char of word (one back)
    if end_byte > col {
        prev_char_boundary(line, end_byte)
    } else {
        col
    }
}
```

Add execute arms:

```rust
UiAction::VimMoveWordForward => {
    let line = &state.doc.lines[state.vim.cursor_line];
    let new_col = next_word_start(line, state.vim.cursor_col);
    state.vim.cursor_col = vim_clamp_col(line, new_col);
}
UiAction::VimMoveWordBackward => {
    let line = &state.doc.lines[state.vim.cursor_line];
    state.vim.cursor_col = prev_word_start(line, state.vim.cursor_col);
}
UiAction::VimMoveWordEnd => {
    let line = &state.doc.lines[state.vim.cursor_line];
    state.vim.cursor_col = word_end(line, state.vim.cursor_col);
}
```

- [ ] **Step 5: Implement mode-transition actions (EnterInsert etc.)**

```rust
UiAction::VimEnterInsert => {
    // Save undo snapshot
    state.vim.undo_stack.push(crate::app::state::UndoEntry {
        lines: state.doc.lines.clone(),
        cursor_line: state.vim.cursor_line,
        cursor_col: state.vim.cursor_col,
    });
    state.vim.pending_op = None;
    state.focus = Focus::VimInsert;
}
UiAction::VimEnterInsertAfter => {
    state.vim.undo_stack.push(crate::app::state::UndoEntry {
        lines: state.doc.lines.clone(),
        cursor_line: state.vim.cursor_line,
        cursor_col: state.vim.cursor_col,
    });
    state.vim.pending_op = None;
    // advance cursor one (insert after)
    let line = &state.doc.lines[state.vim.cursor_line];
    let next = next_char_boundary(line, state.vim.cursor_col);
    state.vim.cursor_col = next.min(line.len()); // insert mode can be at end
    state.focus = Focus::VimInsert;
}
UiAction::VimEnterInsertEOL => {
    state.vim.undo_stack.push(crate::app::state::UndoEntry {
        lines: state.doc.lines.clone(),
        cursor_line: state.vim.cursor_line,
        cursor_col: state.vim.cursor_col,
    });
    state.vim.pending_op = None;
    state.vim.cursor_col = state.doc.lines[state.vim.cursor_line].len(); // past last char
    state.focus = Focus::VimInsert;
}
UiAction::VimExitInsert => {
    // Move cursor left one (vim convention: exit insert lands on last typed char)
    let col = state.vim.cursor_col;
    let line = &state.doc.lines[state.vim.cursor_line];
    if col > 0 {
        state.vim.cursor_col = prev_char_boundary(line, col);
    }
    state.vim.cursor_col = vim_clamp_col(
        &state.doc.lines[state.vim.cursor_line],
        state.vim.cursor_col,
    );
    state.vim.pending_op = None;
    state.focus = Focus::VimNormal;
    let _ = crate::app::actions::after_vim_edit(state);
}
```

Add `after_vim_edit` to `src/app/actions.rs` (called whenever insert mode changes the doc):

```rust
pub fn after_vim_edit(state: &mut AppState) -> anyhow::Result<()> {
    state.selectables = state.doc.selectables();
    state.save()?;
    state.dates_with_notes =
        crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
    state.panel_todos =
        crate::ui::right_panel::collect_panel_todos(&state.notes_dir, state.date, &state.config);
    vim_update_context(state);
    Ok(())
}
```

- [ ] **Step 6: Write tests for cursor movement**

Add to `#[cfg(test)]` in `input.rs`:

```rust
#[test]
fn vim_move_right_advances_cursor() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["hello".to_string()];
    state.vim.cursor_line = 0;
    state.vim.cursor_col = 0;
    execute_action(&mut state, UiAction::VimMoveRight).unwrap();
    assert_eq!(state.vim.cursor_col, 1);
}

#[test]
fn vim_move_right_does_not_go_past_last_char_in_normal_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["hi".to_string()];
    state.vim.cursor_line = 0;
    state.vim.cursor_col = 1; // on 'i', last char
    execute_action(&mut state, UiAction::VimMoveRight).unwrap();
    assert_eq!(state.vim.cursor_col, 1, "cursor should not move past last char");
}

#[test]
fn vim_move_down_advances_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["line 0".to_string(), "line 1".to_string()];
    state.vim.cursor_line = 0;
    execute_action(&mut state, UiAction::VimMoveDown).unwrap();
    assert_eq!(state.vim.cursor_line, 1);
}

#[test]
fn vim_move_up_stays_at_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["line 0".to_string()];
    state.vim.cursor_line = 0;
    execute_action(&mut state, UiAction::VimMoveUp).unwrap();
    assert_eq!(state.vim.cursor_line, 0);
}

#[test]
fn vim_move_file_end_goes_to_last_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    execute_action(&mut state, UiAction::VimMoveFileEnd).unwrap();
    assert_eq!(state.vim.cursor_line, 2);
}

#[test]
fn vim_enter_insert_sets_vim_insert_focus() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["hello".to_string()];
    execute_action(&mut state, UiAction::VimEnterInsert).unwrap();
    assert_eq!(state.focus, Focus::VimInsert);
}

#[test]
fn vim_enter_insert_pushes_undo_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["hello".to_string()];
    assert!(state.vim.undo_stack.is_empty());
    execute_action(&mut state, UiAction::VimEnterInsert).unwrap();
    assert_eq!(state.vim.undo_stack.len(), 1);
}

#[test]
fn vim_pending_op_is_set_then_cleared() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    execute_action(&mut state, UiAction::VimSetPendingOp('d')).unwrap();
    assert_eq!(state.vim.pending_op, Some('d'));
    execute_action(&mut state, UiAction::VimClearPendingOp).unwrap();
    assert!(state.vim.pending_op.is_none());
}
```

- [ ] **Step 7: Run tests**

```bash
cargo test
```
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/app/input.rs src/app/actions.rs
git commit -m "feat: implement VimNormal cursor movement and mode transition actions"
```

---

### Task 7: Vim document-editing operations (insert, delete, yank, paste, undo)

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Implement VimInsert text editing actions**

Add these execute arms:

```rust
UiAction::VimInsertChar(c) => {
    let line = &mut state.doc.lines[state.vim.cursor_line];
    line.insert(state.vim.cursor_col, c);
    state.vim.cursor_col += c.len_utf8();
}
UiAction::VimInsertNewline => {
    let tail = state.doc.lines[state.vim.cursor_line][state.vim.cursor_col..].to_string();
    state.doc.lines[state.vim.cursor_line].truncate(state.vim.cursor_col);
    state.vim.cursor_line += 1;
    state.doc.lines.insert(state.vim.cursor_line, tail);
    state.vim.cursor_col = 0;
}
UiAction::VimInsertBackspace => {
    let col = state.vim.cursor_col;
    if col > 0 {
        // Delete char before cursor on current line
        let prev = prev_char_boundary(&state.doc.lines[state.vim.cursor_line], col);
        state.doc.lines[state.vim.cursor_line].remove(prev);
        state.vim.cursor_col = prev;
    } else if state.vim.cursor_line > 0 {
        // Merge with previous line
        let current = state.doc.lines.remove(state.vim.cursor_line);
        state.vim.cursor_line -= 1;
        let prev_len = state.doc.lines[state.vim.cursor_line].len();
        state.doc.lines[state.vim.cursor_line].push_str(&current);
        state.vim.cursor_col = prev_len;
    }
}
UiAction::VimInsertDeleteWordBefore => {
    let col = state.vim.cursor_col;
    let new_col = prev_word_start(&state.doc.lines[state.vim.cursor_line], col);
    let line = &mut state.doc.lines[state.vim.cursor_line];
    line.drain(new_col..col);
    state.vim.cursor_col = new_col;
}
```

- [ ] **Step 2: Implement Normal-mode document operations**

```rust
UiAction::VimDeleteChar => {
    let line = &state.doc.lines[state.vim.cursor_line];
    if line.is_empty() {
        // nothing to delete
    } else {
        let col = state.vim.cursor_col;
        let end = next_char_boundary(line, col);
        let line = &mut state.doc.lines[state.vim.cursor_line];
        line.drain(col..end);
        // Clamp cursor after deletion
        let line_len = state.doc.lines[state.vim.cursor_line].len();
        if col > 0 && col >= line_len {
            state.vim.cursor_col = prev_char_boundary(
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
    if n == 0 { /* nothing */ } else {
        let removed = state.doc.lines.remove(state.vim.cursor_line);
        state.vim.yank_buffer = vec![removed];
        // Clamp cursor_line
        let new_n = state.doc.lines.len();
        if state.vim.cursor_line >= new_n && new_n > 0 {
            state.vim.cursor_line = new_n - 1;
        }
        if !state.doc.lines.is_empty() {
            state.vim.cursor_col = vim_clamp_col(
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
```

- [ ] **Step 3: Implement VimInsertLineBelow, VimInsertLineAbove**

```rust
UiAction::VimInsertLineBelow => {
    // Save undo snapshot
    state.vim.undo_stack.push(crate::app::state::UndoEntry {
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
    state.vim.undo_stack.push(crate::app::state::UndoEntry {
        lines: state.doc.lines.clone(),
        cursor_line: state.vim.cursor_line,
        cursor_col: state.vim.cursor_col,
    });
    state.vim.pending_op = None;
    state.doc.lines.insert(state.vim.cursor_line, String::new());
    state.vim.cursor_col = 0;
    state.focus = Focus::VimInsert;
}
```

- [ ] **Step 4: Implement VimUndo and VimToggleTodo**

```rust
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
```

- [ ] **Step 5: Write tests for document operations**

```rust
#[test]
fn vim_insert_char_adds_to_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimInsert;
    state.doc.lines = vec!["hello".to_string()];
    state.vim.cursor_line = 0;
    state.vim.cursor_col = 5; // end of "hello"
    execute_action(&mut state, UiAction::VimInsertChar('!')).unwrap();
    assert_eq!(state.doc.lines[0], "hello!");
    assert_eq!(state.vim.cursor_col, 6);
}

#[test]
fn vim_insert_newline_splits_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimInsert;
    state.doc.lines = vec!["hello world".to_string()];
    state.vim.cursor_line = 0;
    state.vim.cursor_col = 5; // between "hello" and " world"
    execute_action(&mut state, UiAction::VimInsertNewline).unwrap();
    assert_eq!(state.doc.lines[0], "hello");
    assert_eq!(state.doc.lines[1], " world");
    assert_eq!(state.vim.cursor_line, 1);
    assert_eq!(state.vim.cursor_col, 0);
}

#[test]
fn vim_insert_backspace_removes_char() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimInsert;
    state.doc.lines = vec!["hello".to_string()];
    state.vim.cursor_line = 0;
    state.vim.cursor_col = 3; // after "hel"
    execute_action(&mut state, UiAction::VimInsertBackspace).unwrap();
    assert_eq!(state.doc.lines[0], "helo");
    assert_eq!(state.vim.cursor_col, 2);
}

#[test]
fn vim_insert_backspace_at_line_start_merges_with_prev() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimInsert;
    state.doc.lines = vec!["first".to_string(), "second".to_string()];
    state.vim.cursor_line = 1;
    state.vim.cursor_col = 0;
    execute_action(&mut state, UiAction::VimInsertBackspace).unwrap();
    assert_eq!(state.doc.lines.len(), 1);
    assert_eq!(state.doc.lines[0], "firstsecond");
    assert_eq!(state.vim.cursor_line, 0);
    assert_eq!(state.vim.cursor_col, 5);
}

#[test]
fn vim_delete_line_removes_and_yanks() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["keep".to_string(), "delete me".to_string(), "keep2".to_string()];
    state.vim.cursor_line = 1;
    execute_action(&mut state, UiAction::VimDeleteLine).unwrap();
    assert_eq!(state.doc.lines.len(), 2);
    assert_eq!(state.doc.lines[0], "keep");
    assert_eq!(state.vim.yank_buffer, vec!["delete me".to_string()]);
}

#[test]
fn vim_yank_line_does_not_delete() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["yanked".to_string()];
    state.vim.cursor_line = 0;
    execute_action(&mut state, UiAction::VimYankLine).unwrap();
    assert_eq!(state.doc.lines.len(), 1, "line should still be there");
    assert_eq!(state.vim.yank_buffer, vec!["yanked".to_string()]);
}

#[test]
fn vim_paste_below_inserts_after_cursor() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["line 0".to_string(), "line 2".to_string()];
    state.vim.yank_buffer = vec!["line 1".to_string()];
    state.vim.cursor_line = 0;
    execute_action(&mut state, UiAction::VimPasteBelow).unwrap();
    assert_eq!(state.doc.lines[1], "line 1");
    assert_eq!(state.vim.cursor_line, 1);
}

#[test]
fn vim_undo_restores_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec!["original".to_string()];
    // Simulate entering insert and making a change
    execute_action(&mut state, UiAction::VimEnterInsert).unwrap(); // pushes snapshot
    state.doc.lines[0] = "modified".to_string();
    execute_action(&mut state, UiAction::VimExitInsert).unwrap();
    // Now undo
    execute_action(&mut state, UiAction::VimUndo).unwrap();
    assert_eq!(state.doc.lines[0], "original");
}

#[test]
fn vim_toggle_todo_checks_unchecked() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.doc.lines = vec![
        "# Day".to_string(),
        String::new(),
        "## To-dos".to_string(),
        String::new(),
        "- [ ] a task".to_string(),
    ];
    state.vim.cursor_line = 4;
    execute_action(&mut state, UiAction::VimToggleTodo).unwrap();
    assert_eq!(state.doc.lines[4], "- [x] a task");
}
```

- [ ] **Step 6: Run tests**

```bash
cargo test
```
Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs src/app/actions.rs
git commit -m "feat: implement vim document editing operations (insert, dd, yy, p, undo)"
```

---

### Task 8: Layout split and rendering (mode line + cursor)

**Files:**
- Modify: `src/ui/layout.rs`
- Modify: `src/ui/document.rs`

- [ ] **Step 1: Split notes inner area for mode line in layout.rs**

In `src/ui/layout.rs`, replace the notes rendering block:

```rust
// Before:
let notes_inner = notes_block.inner(notes_area);
frame.render_widget(notes_block, notes_area);
super::document::render(frame, app, notes_inner, theme);

// After:
let notes_inner = notes_block.inner(notes_area);
frame.render_widget(notes_block, notes_area);

let notes_layout = ratatui::layout::Layout::default()
    .direction(ratatui::layout::Direction::Vertical)
    .constraints([
        ratatui::layout::Constraint::Min(0),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(notes_inner);
let notes_content_area = notes_layout[0];
let notes_mode_area = notes_layout[1];

super::document::render(frame, app, notes_content_area, theme);
super::document::render_mode_line(frame, app, notes_mode_area, theme);
```

- [ ] **Step 2: Update document.rs render() for vim cursor**

Replace `src/ui/document.rs` `render()` function entirely:

```rust
pub fn render(frame: &mut ratatui::Frame, app: &AppState, area: Rect, theme: &Theme) {
    use crate::app::state::Focus;

    let vim_active = matches!(app.focus, Focus::VimNormal | Focus::VimInsert);
    let cursor_line = app.vim.cursor_line;

    // For Navigate mode (legacy), keep old selected-range highlight
    // (Navigate is gone, but keep the None path for Capture/Chat focus)
    let mut in_code = false;
    let text_lines: Vec<Line> = app
        .doc
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            // Cursor line: render raw in vim modes
            if vim_active && i == cursor_line {
                in_code = false; // reset; raw line shown anyway
                let bg_style = Style::default()
                    .bg(theme.notes_panel_bg); // subtle tint via bg; cursor cell highlighted separately
                return Line::from(Span::styled(line.as_str(), bg_style));
            }

            let fence = line.trim_start().starts_with("```");
            if in_code || fence {
                if fence {
                    in_code = !in_code;
                }
                return Line::from(Span::styled(line.as_str(), Style::default().fg(theme.code)));
            }

            if let Some(rest) = line.strip_prefix("###### ") {
                Line::from(Span::styled(
                    format!("###### {}", rest),
                    Style::default().fg(theme.heading6).add_modifier(Modifier::BOLD),
                ))
            } else if let Some(rest) = line.strip_prefix("##### ") {
                Line::from(Span::styled(
                    format!("##### {}", rest),
                    Style::default().fg(theme.heading5).add_modifier(Modifier::BOLD),
                ))
            } else if let Some(rest) = line.strip_prefix("#### ") {
                Line::from(Span::styled(
                    format!("#### {}", rest),
                    Style::default().fg(theme.heading4).add_modifier(Modifier::BOLD),
                ))
            } else if let Some(rest) = line.strip_prefix("### ") {
                Line::from(Span::styled(
                    format!("### {}", rest),
                    Style::default().fg(theme.heading3).add_modifier(Modifier::BOLD),
                ))
            } else if let Some(rest) = line.strip_prefix("## ") {
                Line::from(Span::styled(
                    format!("## {}", rest),
                    Style::default().fg(theme.heading2).add_modifier(Modifier::BOLD),
                ))
            } else if let Some(rest) = line.strip_prefix("# ") {
                Line::from(Span::styled(
                    format!("# {}", rest),
                    Style::default().fg(theme.heading1).add_modifier(Modifier::BOLD),
                ))
            } else if let Some(rest) = line.strip_prefix("- [ ] ") {
                Line::from(vec![Span::raw("☐ "), Span::raw(rest)])
            } else if let Some(rest) = line
                .strip_prefix("- [x] ")
                .or_else(|| line.strip_prefix("- [X] "))
            {
                Line::from(vec![
                    Span::styled("☑ ", Style::default().fg(theme.todo_done)),
                    Span::styled(
                        rest,
                        Style::default()
                            .fg(theme.todo_done)
                            .add_modifier(Modifier::CROSSED_OUT),
                    ),
                ])
            } else if let Some(rest) = line
                .strip_prefix("> ")
                .or_else(|| if line == ">" { Some("") } else { None })
            {
                Line::from(vec![
                    Span::styled(
                        "│ ",
                        Style::default()
                            .fg(theme.quote_marker)
                            .add_modifier(Modifier::ITALIC),
                    ),
                    Span::styled(rest, Style::default().add_modifier(Modifier::ITALIC)),
                ])
            } else if let Some(rest) = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .or_else(|| line.strip_prefix("+ "))
            {
                Line::from(vec![Span::raw("• "), Span::raw(rest)])
            } else if crate::model::parser::is_ordered(line) {
                Line::from(Span::raw(line.as_str()))
            } else {
                Line::from(line.as_str())
            }
        })
        .collect();

    // Scroll: follow cursor in vim mode, else 0
    let scroll_offset: usize = if vim_active {
        let visible_height = area.height as usize;
        cursor_line.saturating_sub(visible_height.saturating_sub(1))
    } else {
        0
    };

    let paragraph = Paragraph::new(Text::from(text_lines)).scroll((scroll_offset as u16, 0));
    frame.render_widget(paragraph, area);

    // Place terminal cursor for vim modes
    if vim_active {
        let line_text = app.doc.lines.get(cursor_line).map(|l| l.as_str()).unwrap_or("");
        // cursor_col is a byte offset; we need the display column (char count)
        let display_col = line_text[..app.vim.cursor_col.min(line_text.len())]
            .chars()
            .count() as u16;
        let display_row = (cursor_line.saturating_sub(scroll_offset)) as u16;
        if display_row < area.height {
            frame.set_cursor_position((
                area.x + display_col,
                area.y + display_row,
            ));
        }
    }
}

pub fn render_mode_line(
    frame: &mut ratatui::Frame,
    app: &AppState,
    area: Rect,
    theme: &Theme,
) {
    use crate::app::state::Focus;
    let total = app.doc.lines.len();
    let current = app.vim.cursor_line + 1;
    let (mode_label, mode_color) = match app.focus {
        Focus::VimNormal => ("-- NORMAL --", theme.heading2),
        Focus::VimInsert => ("-- INSERT --", theme.heading3),
        _ => return, // no mode line when not in vim mode
    };
    let left = Span::styled(mode_label, Style::default().fg(mode_color));
    let right_text = format!("ln {}/{}", current, total);
    let right = Span::styled(right_text, Style::default().fg(theme.heading6));
    // Pad the middle
    let left_len = mode_label.len() as u16;
    let right_len = format!("ln {}/{}", current, total).len() as u16;
    let gap = area.width.saturating_sub(left_len + right_len);
    let line = Line::from(vec![
        left,
        Span::raw(" ".repeat(gap as usize)),
        right,
    ]);
    let widget = ratatui::widgets::Paragraph::new(line);
    frame.render_widget(widget, area);
}
```

- [ ] **Step 3: Add `heading2`, `heading3`, `heading6` re-use note (these already exist in Theme)**

Run:
```bash
cargo build
```
Fix any compile errors (likely missing `use` imports for `Style`, `Span`, `Line`, `Modifier`).

- [ ] **Step 4: Write rendering test**

Add to `#[cfg(test)]` in `src/ui/layout.rs`:

```rust
#[test]
fn render_vim_normal_shows_mode_line() {
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::VimNormal, 0);
    app.vim.cursor_line = 0;

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(
        content.contains("NORMAL"),
        "Expected NORMAL in mode line, got: {}",
        content
    );
}

#[test]
fn render_vim_insert_shows_insert_mode_line() {
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::VimInsert, 0);
    app.vim.cursor_line = 0;

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(
        content.contains("INSERT"),
        "Expected INSERT in mode line, got: {}",
        content
    );
}
```

Note: the existing `test_app` helper in `layout.rs` constructs `AppState` directly. It will need to include `vim: VimState::default()` after Task 1 is done. Add it now:

```rust
fn test_app(doc: Document, focus: Focus, selected: usize) -> AppState {
    let selectables = doc.selectables();
    AppState {
        // ... existing fields ...
        vim: crate::app::state::VimState::default(),
        // ...
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test
```
Expected: all tests pass including the two new rendering tests.

- [ ] **Step 6: Commit**

```bash
git add src/ui/layout.rs src/ui/document.rs
git commit -m "feat: add mode line and vim cursor rendering to notes panel"
```

---

### Task 9: Wire context updates, post-submit cursor jump, focus cycle, and help text

**Files:**
- Modify: `src/app/actions.rs`
- Modify: `src/ui/help.rs`

- [ ] **Step 1: Update go_to_date to initialise vim cursor**

In `src/app/actions.rs`, after `*state = AppState::open_day(notes_dir, config, date)?;`, verify that `AppState::open_day` initializes `vim: VimState::default()` (done in Task 1). No additional change needed — the cursor resets to (0, 0) on day navigation automatically.

- [ ] **Step 2: Jump cursor to new content after capture submit**

In `src/app/input.rs`, in the `UiAction::SubmitInput` arm, after `crate::app::actions::dispatch(state, cmd)?;`, call:

```rust
crate::app::actions::vim_jump_to_new_content(state);
```

Add `vim_jump_to_new_content` to `src/app/actions.rs`:

```rust
/// After a capture-bar entry is submitted, move the vim cursor to the last
/// line that was inserted. We find this by diffing line count: the new lines
/// were inserted at the bottom of the appropriate section, so we scan backward
/// from the end of the document for the first non-blank, non-section-header
/// content line that could be the new entry.
/// 
/// Simple heuristic: cursor goes to the last non-empty line in the document.
/// This is correct for all entry types (bullet, todo, meeting heading, etc.)
/// because they are always appended to the end of their section block.
pub fn vim_jump_to_new_content(state: &mut AppState) {
    if !matches!(state.focus, crate::app::state::Focus::VimNormal | crate::app::state::Focus::VimInsert) {
        return;
    }
    // Find last non-empty line
    if let Some(idx) = state.doc.lines.iter().rposition(|l| !l.trim().is_empty()) {
        state.vim.cursor_line = idx;
        state.vim.cursor_col = 0;
        vim_update_context(state);
    }
}
```

- [ ] **Step 3: Ensure after_doc_mutation updates vim context**

In `src/app/actions.rs`, update `after_doc_mutation` to also refresh vim context:

```rust
fn after_doc_mutation(state: &mut AppState) -> anyhow::Result<()> {
    state.selectables = state.doc.selectables();
    state.save()?;
    state.dates_with_notes =
        crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
    state.panel_todos =
        crate::ui::right_panel::collect_panel_todos(&state.notes_dir, state.date, &state.config);
    state.status.clear();
    vim_update_context(state);
    Ok(())
}
```

- [ ] **Step 4: Update help text**

In `src/ui/help.rs`, replace the `help_text` string literal. The current `Navigation:` section describes the old Navigate mode. Replace the entire `help_text` with:

```rust
let help_text = r#"Capture mode:
  type to enter notes, Enter to submit, Esc to navigate
  Tab        insert indent (->)
  Ctrl+.     prepend indent at line start

Commands:
  /meeting "Name"  start meeting context
  /note "Name"     start note context
  /note            switch to Notes context
  /section "Name"  add sub-section (one heading deeper, max ######)
  /todo text       add todo
  /start           record meeting start (current time)
  /end             record meeting end (current time)
  /scheduled HH:MM  record scheduled start time
  /leave           exit meeting context
  /goto YYYY-MM-DD  jump to date
  /today, Ctrl-T   jump to today
  /ask message     ask the local LLM (streams to chat)
  /clear           clear the chat conversation

Chat panel:
  Ctrl-L           show/hide chat panel
  Tab              focus chat (then j/k to scroll)

Notes panel -- Normal mode:
  hjkl / arrows  move cursor
  w / b / e      word forward / backward / end
  0 / $          line start / end
  gg / G         file start / end
  i / a / A      insert before / after / end of line
  o / O          new line below / above, insert mode
  x              delete char at cursor
  dd             delete line (saved to yank buffer)
  yy / p / P     yank line / paste below / paste above
  u              undo last insert session
  t              toggle todo at cursor line
  Tab            focus capture bar
  ?              help

Notes panel -- Insert mode:
  type           edit raw markdown
  Enter          new line
  Backspace      delete char (or merge with prev line)
  Ctrl-W         delete word before cursor
  arrows         move cursor
  Esc            return to normal mode

Right panel:
  Tab        focus right panel
  j/k or ↑/↓  navigate panel todos
  Space/x    toggle selected todo
  Esc        return to document

Navigation:
  [ ]        prev/next day
  Ctrl-C     quit"#;
```

- [ ] **Step 5: Write integration test for context update on cursor move**

Add to `#[cfg(test)]` in `src/app/state.rs` (or `src/app/actions.rs`):

```rust
#[test]
fn vim_cursor_move_updates_context_to_meeting() {
    use crate::app::actions::vim_update_context;
    let tmp = tempfile::tempdir().unwrap();
    let mut state = AppState::open_day(
        tmp.path().to_path_buf(),
        Config::default(),
        NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
    )
    .unwrap();
    // Lines: # Day, blank, ## Meetings, blank, ### Standup, blank, ## Notes
    state.doc.lines = vec![
        "# Day".to_string(),
        String::new(),
        "## Meetings".to_string(),
        String::new(),
        "### Standup".to_string(),
        "- discussed things".to_string(),
        String::new(),
        "## Notes".to_string(),
    ];
    state.vim.cursor_line = 5; // inside Standup meeting
    vim_update_context(&mut state);
    assert_eq!(state.context, Context::Meeting(0));
    assert!(state.context_display.contains("Standup"), "got: {}", state.context_display);
}
```

- [ ] **Step 6: Write test for post-submit cursor jump**

Add to `#[cfg(test)]` in `src/app/input.rs` (where `test_state` is already accessible):

```rust
#[test]
fn submit_entry_moves_vim_cursor_to_new_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    state.context = crate::app::state::Context::Notes;
    state.input = "a test note entry".to_string();
    execute_action(&mut state, UiAction::SubmitInput).unwrap();
    let cursor_line = state.vim.cursor_line;
    assert!(
        cursor_line > 0,
        "cursor should have moved from 0 after submit, was {}",
        cursor_line
    );
    assert!(
        !state.doc.lines[cursor_line].trim().is_empty(),
        "cursor should land on non-empty line"
    );
}
```

- [ ] **Step 7: Run all tests**

```bash
cargo test
```
Expected: all tests pass.

- [ ] **Step 8: Manual smoke test**

Run the app and verify:
- App opens in Capture mode (unchanged)
- `Esc` from Capture switches to VimNormal — `-- NORMAL --` appears at bottom of notes panel
- `hjkl` and arrow keys move cursor through notes
- Cursor line renders raw; other lines render formatted
- `i` enters insert mode — `-- INSERT --` appears
- Typing characters edits the note
- `Esc` returns to normal mode
- `dd` deletes a line, `p` pastes it back
- `u` undoes last insert session
- `t` toggles a todo
- `Tab` from VimNormal focuses capture bar with correct context
- `Enter` in capture bar submits an entry and cursor jumps to it
- `?` in VimNormal shows help overlay with updated vim key reference

```bash
cargo run
```

- [ ] **Step 9: Commit**

```bash
git add src/app/actions.rs src/ui/help.rs src/app/input.rs
git commit -m "feat: wire context-on-cursor-move, post-submit cursor jump, and update help text"
```
