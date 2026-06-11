# Input Action Namespacing Refactor

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat `UiAction` enum with namespaced sub-enums so that `execute_action` in `mod.rs` becomes a clean 9-arm dispatcher with no inline state mutation and no long delegation chains.

**Architecture:** Each input subsystem gets its own action sub-enum (`ChatAction`, `RightPanelAction`, `VimNormalAction`, `VimInsertAction`, `CaptureAction`, `GlobalAction`, `OverlayAction`, `FocusAction`). The top-level `UiAction` wraps them. Each sub-module's `execute_action` signature changes from `(state, UiAction)` to `(state, XxxAction)`, eliminating the `_ => unreachable!()` guards. A final task extracts the 7 sequential guard steps in `key_to_action` into named pipeline functions. Each task is a single atomic migration of one subsystem — compilable, test-passing, and committable on its own.

**Tech Stack:** Rust, `ratatui`/`crossterm` for key events, `anyhow::Result`.

**Baseline:** `cargo test` → 536 passed, 0 failed.

---

## File Map

| File | Role |
|------|------|
| `src/app/input/mod.rs` | Owns `UiAction` and all sub-enums; owns top-level `key_to_action` and `execute_action`; delegates to sub-modules |
| `src/app/input/capture.rs` | Capture mode key mapping and action execution |
| `src/app/input/chat.rs` | Chat panel key mapping and action execution |
| `src/app/input/right_panel.rs` | Right panel key mapping and action execution |
| `src/app/input/vim_normal.rs` | Vim normal mode key mapping and action execution |
| `src/app/input/vim_insert.rs` | Vim insert mode key mapping and action execution |

No new files are created. All changes are edits to the six files above.

---

## Task 1: Migrate `ChatAction`

**Files:** `src/app/input/mod.rs`, `src/app/input/chat.rs`

This is the smallest subsystem — 4 scroll variants. It is a template for all subsequent tasks.

- [ ] **Step 1: Add `ChatAction` enum to `mod.rs` and update `UiAction`**

In `mod.rs`, add the new sub-enum **after** the `UiAction` enum. Keep `ToggleChat`, `FocusChat`, and `ChatBlur` as flat variants — those are focus transitions handled in a later task.

```rust
// Add this new enum after the UiAction definition:
#[derive(Debug, PartialEq, Eq)]
pub enum ChatAction {
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
}
```

In the `UiAction` enum:
- **Remove** these variants: `ChatScrollUp`, `ChatScrollDown`, `ChatPageUp`, `ChatPageDown`
- **Add** this variant: `Chat(ChatAction)`
- **Keep unchanged**: `ToggleChat`, `FocusChat`, `ChatBlur` (migrated in Task 6)

- [ ] **Step 2: Update `chat.rs`**

Replace the entire file content:

```rust
use crate::app::state::AppState;
use crate::app::input::{ChatAction, EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(UiAction::Chat(ChatAction::ScrollDown)),
        KeyCode::Up   | KeyCode::Char('k') => Some(UiAction::Chat(ChatAction::ScrollUp)),
        KeyCode::PageDown => Some(UiAction::Chat(ChatAction::PageDown)),
        KeyCode::PageUp   => Some(UiAction::Chat(ChatAction::PageUp)),
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: ChatAction) -> Result<EventOutcome> {
    match action {
        ChatAction::ScrollUp   => { state.chat.scroll += 1; }
        ChatAction::ScrollDown => { state.chat.scroll = state.chat.scroll.saturating_sub(1); }
        ChatAction::PageUp     => { state.chat.scroll += 10; }
        ChatAction::PageDown   => { state.chat.scroll = state.chat.scroll.saturating_sub(10); }
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 3: Update delegation in `mod.rs::execute_action`**

Find and replace the chat delegation block:

```rust
// REMOVE this block:
        UiAction::ChatScrollUp
        | UiAction::ChatScrollDown
        | UiAction::ChatPageUp
        | UiAction::ChatPageDown => return chat::execute_action(state, action),

// REPLACE WITH:
        UiAction::Chat(a) => return chat::execute_action(state, a),
```

- [ ] **Step 4: Update tests in `mod.rs`**

Three tests reference the old flat chat scroll variants. Update them:

```rust
// chat_scroll_keys_map — update the two assertions:
assert_eq!(key_to_action(&state, make_key(KeyCode::Char('k'))), Some(UiAction::Chat(ChatAction::ScrollUp)));
assert_eq!(key_to_action(&state, make_key(KeyCode::Char('j'))), Some(UiAction::Chat(ChatAction::ScrollDown)));

// chat_scroll_down_saturates_at_zero — update the execute_action call:
execute_action(&mut state, UiAction::Chat(ChatAction::ScrollDown)).unwrap();

// chat_scroll_up_increments — update the execute_action call:
execute_action(&mut state, UiAction::Chat(ChatAction::ScrollUp)).unwrap();
```

- [ ] **Step 5: Verify**

```
cargo test
```
Expected: 536 passed, 0 failed.

- [ ] **Step 6: Commit**

```bash
git add src/app/input/mod.rs src/app/input/chat.rs
git commit -m "refactor(input): namespace chat scroll actions under ChatAction"
```

---

## Task 2: Migrate `RightPanelAction`

**Files:** `src/app/input/mod.rs`, `src/app/input/right_panel.rs`

3 variants. `FocusRightPanel` and `RightPanelBlur` remain flat — they are focus transitions migrated in Task 6.

- [ ] **Step 1: Add `RightPanelAction` enum to `mod.rs` and update `UiAction`**

```rust
// Add after ChatAction:
#[derive(Debug, PartialEq, Eq)]
pub enum RightPanelAction {
    Up,
    Down,
    Toggle,
}
```

In `UiAction`:
- **Remove**: `RightPanelUp`, `RightPanelDown`, `RightPanelToggle`
- **Add**: `RightPanel(RightPanelAction)`
- **Keep unchanged**: `FocusRightPanel`, `RightPanelBlur`

- [ ] **Step 2: Update `right_panel.rs`**

Replace the entire file content:

```rust
use crate::app::state::AppState;
use crate::app::input::{EventOutcome, RightPanelAction, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(UiAction::RightPanel(RightPanelAction::Down)),
        KeyCode::Up   | KeyCode::Char('k') => Some(UiAction::RightPanel(RightPanelAction::Up)),
        KeyCode::Char(' ') | KeyCode::Char('x') => Some(UiAction::RightPanel(RightPanelAction::Toggle)),
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: RightPanelAction) -> Result<EventOutcome> {
    match action {
        RightPanelAction::Up => {
            if state.right_panel_selected > 0 {
                state.right_panel_selected -= 1;
            }
        }
        RightPanelAction::Down => {
            let max = state.panel_todos.len().saturating_sub(1);
            if state.right_panel_selected < max {
                state.right_panel_selected += 1;
            }
        }
        RightPanelAction::Toggle => {
            crate::app::actions::toggle_panel_todo(state)?;
        }
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 3: Update delegation in `mod.rs::execute_action`**

```rust
// REMOVE:
        UiAction::RightPanelUp
        | UiAction::RightPanelDown
        | UiAction::RightPanelToggle => return right_panel::execute_action(state, action),

// REPLACE WITH:
        UiAction::RightPanel(a) => return right_panel::execute_action(state, a),
```

- [ ] **Step 4: Verify**

```
cargo test
```
Expected: 536 passed, 0 failed. No test changes required — no `mod.rs` tests use `RightPanelUp/Down/Toggle` directly.

- [ ] **Step 5: Commit**

```bash
git add src/app/input/mod.rs src/app/input/right_panel.rs
git commit -m "refactor(input): namespace right panel actions under RightPanelAction"
```

---

## Task 3: Migrate `VimNormalAction`

**Files:** `src/app/input/mod.rs`, `src/app/input/vim_normal.rs`, `src/app/input/vim_insert.rs` (arrow key production only)

This is the largest individual migration: 26 variants. `vim_insert.rs` currently emits `UiAction::VimMoveLeft` etc. for arrow keys, so its `key_to_action` must be updated in this same task.

- [ ] **Step 1: Add `VimNormalAction` enum to `mod.rs` and update `UiAction`**

```rust
// Add after RightPanelAction:
#[derive(Debug, PartialEq, Eq)]
pub enum VimNormalAction {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordForward,
    MoveWordBackward,
    MoveWordEnd,
    MoveLineStart,
    MoveLineEnd,
    MoveFileStart,
    MoveFileEnd,
    SetPendingOp(char),
    ClearPendingOp,
    EnterInsert,
    EnterInsertAfter,
    EnterInsertEOL,
    InsertLineBelow,
    InsertLineAbove,
    DeleteChar,
    DeleteLine,
    YankLine,
    PasteBelow,
    PasteAbove,
    Undo,
    ToggleTodo,
    BeginEditLine,
}
```

In `UiAction`:
- **Remove** all 26 flat `Vim*` variants: `VimMoveLeft`, `VimMoveRight`, `VimMoveUp`, `VimMoveDown`, `VimMoveWordForward`, `VimMoveWordBackward`, `VimMoveWordEnd`, `VimMoveLineStart`, `VimMoveLineEnd`, `VimMoveFileStart`, `VimMoveFileEnd`, `VimSetPendingOp(char)`, `VimClearPendingOp`, `VimEnterInsert`, `VimEnterInsertAfter`, `VimEnterInsertEOL`, `VimInsertLineBelow`, `VimInsertLineAbove`, `VimDeleteChar`, `VimDeleteLine`, `VimYankLine`, `VimPasteBelow`, `VimPasteAbove`, `VimUndo`, `VimToggleTodo`, `VimBeginEditLine`
- **Add**: `VimNormal(VimNormalAction)`
- **Keep unchanged**: all `VimInsert*` flat variants (`VimInsertChar`, `VimInsertNewline`, `VimInsertBackspace`, `VimInsertDeleteWordBefore`, `VimInsertTab`, `VimExitInsert`) — migrated in Task 4

- [ ] **Step 2: Update `vim_normal.rs` — `key_to_action`**

Replace the `key_to_action` function:

```rust
pub(super) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    if let Some(op) = state.vim.pending_op {
        return match (op, &key.code) {
            ('d', KeyCode::Char('d')) => Some(UiAction::VimNormal(VimNormalAction::DeleteLine)),
            ('y', KeyCode::Char('y')) => Some(UiAction::VimNormal(VimNormalAction::YankLine)),
            ('g', KeyCode::Char('g')) => Some(UiAction::VimNormal(VimNormalAction::MoveFileStart)),
            _ => Some(UiAction::VimNormal(VimNormalAction::ClearPendingOp)),
        };
    }
    match key.code {
        KeyCode::Char('h') | KeyCode::Left  => Some(UiAction::VimNormal(VimNormalAction::MoveLeft)),
        KeyCode::Char('l') | KeyCode::Right => Some(UiAction::VimNormal(VimNormalAction::MoveRight)),
        KeyCode::Char('j') | KeyCode::Down  => Some(UiAction::VimNormal(VimNormalAction::MoveDown)),
        KeyCode::Char('k') | KeyCode::Up    => Some(UiAction::VimNormal(VimNormalAction::MoveUp)),
        KeyCode::Char('w') => Some(UiAction::VimNormal(VimNormalAction::MoveWordForward)),
        KeyCode::Char('b') => Some(UiAction::VimNormal(VimNormalAction::MoveWordBackward)),
        KeyCode::Char('e') => Some(UiAction::VimNormal(VimNormalAction::MoveWordEnd)),
        KeyCode::Char('0') => Some(UiAction::VimNormal(VimNormalAction::MoveLineStart)),
        KeyCode::Char('$') => Some(UiAction::VimNormal(VimNormalAction::MoveLineEnd)),
        KeyCode::Char('G') => Some(UiAction::VimNormal(VimNormalAction::MoveFileEnd)),
        KeyCode::Char('i') => Some(UiAction::VimNormal(VimNormalAction::EnterInsert)),
        KeyCode::Char('a') => Some(UiAction::VimNormal(VimNormalAction::EnterInsertAfter)),
        KeyCode::Char('A') => Some(UiAction::VimNormal(VimNormalAction::EnterInsertEOL)),
        KeyCode::Char('o') => Some(UiAction::VimNormal(VimNormalAction::InsertLineBelow)),
        KeyCode::Char('O') => Some(UiAction::VimNormal(VimNormalAction::InsertLineAbove)),
        KeyCode::Char('x') => Some(UiAction::VimNormal(VimNormalAction::DeleteChar)),
        KeyCode::Char('d') => Some(UiAction::VimNormal(VimNormalAction::SetPendingOp('d'))),
        KeyCode::Char('y') => Some(UiAction::VimNormal(VimNormalAction::SetPendingOp('y'))),
        KeyCode::Char('g') => Some(UiAction::VimNormal(VimNormalAction::SetPendingOp('g'))),
        KeyCode::Char('p') => Some(UiAction::VimNormal(VimNormalAction::PasteBelow)),
        KeyCode::Char('P') => Some(UiAction::VimNormal(VimNormalAction::PasteAbove)),
        KeyCode::Char('u') => Some(UiAction::VimNormal(VimNormalAction::Undo)),
        KeyCode::Char('t') => Some(UiAction::VimNormal(VimNormalAction::ToggleTodo)),
        KeyCode::Char('?') => Some(UiAction::OpenHelp),     // stays flat until Task 6
        KeyCode::Tab       => Some(UiAction::SwitchToCapture), // stays flat until Task 6
        KeyCode::Enter     => Some(UiAction::VimNormal(VimNormalAction::BeginEditLine)),
        KeyCode::Esc       => None,
        _ => None,
    }
}
```

- [ ] **Step 3: Update `vim_normal.rs` — `execute_action`**

Replace the `execute_action` function. Also update the import line at the top of the file.

Import change:
```rust
// BEFORE:
use crate::app::input::{EventOutcome, UiAction};
// AFTER:
use crate::app::input::{EventOutcome, UiAction, VimNormalAction};
```

Replace `execute_action`:
```rust
pub(super) fn execute_action(state: &mut AppState, action: VimNormalAction) -> Result<EventOutcome> {
    match action {
        VimNormalAction::MoveLeft         => move_left(state),
        VimNormalAction::MoveRight        => move_right(state),
        VimNormalAction::MoveDown         => move_down(state),
        VimNormalAction::MoveUp           => move_up(state),
        VimNormalAction::MoveLineStart    => move_line_start(state),
        VimNormalAction::MoveLineEnd      => move_line_end(state),
        VimNormalAction::MoveFileStart    => move_file_start(state),
        VimNormalAction::MoveFileEnd      => move_file_end(state),
        VimNormalAction::MoveWordForward  => move_word_forward(state),
        VimNormalAction::MoveWordBackward => move_word_backward(state),
        VimNormalAction::MoveWordEnd      => move_word_end(state),
        VimNormalAction::SetPendingOp(op) => { state.vim.pending_op = Some(op); }
        VimNormalAction::ClearPendingOp   => { state.vim.pending_op = None; }
        VimNormalAction::EnterInsert      => enter_insert(state),
        VimNormalAction::EnterInsertAfter => enter_insert_after(state),
        VimNormalAction::EnterInsertEOL   => enter_insert_eol(state),
        VimNormalAction::InsertLineBelow  => insert_line_below(state),
        VimNormalAction::InsertLineAbove  => insert_line_above(state),
        VimNormalAction::DeleteChar       => delete_char(state),
        VimNormalAction::DeleteLine       => delete_line(state),
        VimNormalAction::YankLine         => yank_line(state),
        VimNormalAction::PasteBelow       => paste_below(state),
        VimNormalAction::PasteAbove       => paste_above(state),
        VimNormalAction::Undo             => undo(state),
        VimNormalAction::ToggleTodo       => toggle_todo(state),
        VimNormalAction::BeginEditLine    => { crate::app::actions::vim_begin_edit_line(state)?; }
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 4: Update `vim_insert.rs` arrow key production**

`vim_insert.rs` currently emits `UiAction::VimMoveLeft` etc. for arrow keys. Update only those 4 arms. The insert-specific actions (`VimExitInsert`, `VimInsertChar`, etc.) stay as flat variants until Task 4.

Update the import line:
```rust
// BEFORE:
use crate::app::input::{EventOutcome, UiAction};
// AFTER:
use crate::app::input::{EventOutcome, UiAction, VimNormalAction};
```

Update only the 4 arrow key arms in `key_to_action`:
```rust
        KeyCode::Left      => Some(UiAction::VimNormal(VimNormalAction::MoveLeft)),
        KeyCode::Right     => Some(UiAction::VimNormal(VimNormalAction::MoveRight)),
        KeyCode::Up        => Some(UiAction::VimNormal(VimNormalAction::MoveUp)),
        KeyCode::Down      => Some(UiAction::VimNormal(VimNormalAction::MoveDown)),
```

All other arms in `vim_insert.rs::key_to_action` remain unchanged (they still emit flat `VimExitInsert`, `VimInsertNewline`, etc.).

- [ ] **Step 5: Update delegation in `mod.rs::execute_action`**

```rust
// REMOVE the entire vim-normal delegation block:
        UiAction::VimMoveLeft
        | UiAction::VimMoveRight
        | UiAction::VimMoveDown
        | UiAction::VimMoveUp
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
        | UiAction::VimToggleTodo
        | UiAction::VimBeginEditLine => return vim_normal::execute_action(state, action),

// REPLACE WITH:
        UiAction::VimNormal(a) => return vim_normal::execute_action(state, a),
```

- [ ] **Step 6: Update tests in `mod.rs`**

The following tests need their `UiAction::VimXxx` patterns updated. Apply all changes before running tests.

**Tests that check `key_to_action` return values** — update the `assert_eq!` expected values:

```rust
// vimnormal_j_moves_down:
Some(UiAction::VimNormal(VimNormalAction::MoveDown))

// vimnormal_down_moves_down:
Some(UiAction::VimNormal(VimNormalAction::MoveDown))

// vimnormal_enter_emits_begin_edit_line:
Some(UiAction::VimNormal(VimNormalAction::BeginEditLine))

// vimnormal_h_moves_left:
Some(UiAction::VimNormal(VimNormalAction::MoveLeft))

// vimnormal_arrow_left_moves_left:
Some(UiAction::VimNormal(VimNormalAction::MoveLeft))

// vimnormal_dd_with_pending_deletes_line:
Some(UiAction::VimNormal(VimNormalAction::DeleteLine))

// vimnormal_gg_with_pending_moves_file_start:
Some(UiAction::VimNormal(VimNormalAction::MoveFileStart))

// vimnormal_pending_op_unknown_second_key_clears:
Some(UiAction::VimNormal(VimNormalAction::ClearPendingOp))

// viminsert_arrow_right_moves_right:
Some(UiAction::VimNormal(VimNormalAction::MoveRight))
```

**Tests that pass actions to `execute_action`** — update the action argument:

```rust
// vim_move_right_advances_cursor:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::MoveRight)).unwrap();

// vim_move_right_does_not_go_past_last_char_in_normal_mode:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::MoveRight)).unwrap();

// vim_move_down_advances_line:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::MoveDown)).unwrap();

// vim_move_up_stays_at_zero:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::MoveUp)).unwrap();

// vim_move_file_end_goes_to_last_line:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::MoveFileEnd)).unwrap();

// vim_enter_insert_sets_vim_insert_focus:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::EnterInsert)).unwrap();

// vim_enter_insert_pushes_undo_snapshot:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::EnterInsert)).unwrap();

// vim_pending_op_is_set_then_cleared:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::SetPendingOp('d'))).unwrap();
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::ClearPendingOp)).unwrap();

// vim_delete_line_removes_and_yanks:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::DeleteLine)).unwrap();

// vim_yank_line_does_not_delete:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::YankLine)).unwrap();

// vim_paste_below_inserts_after_cursor:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::PasteBelow)).unwrap();

// vim_undo_restores_snapshot — three lines in this test:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::EnterInsert)).unwrap();
// NOTE: UiAction::VimExitInsert stays flat here — updated in Task 4
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::Undo)).unwrap();

// vim_toggle_todo_checks_unchecked:
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::ToggleTodo)).unwrap();

// vim_cursor_context_updates_on_move (two calls):
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::MoveDown)).unwrap();
execute_action(&mut state, UiAction::VimNormal(VimNormalAction::MoveDown)).unwrap();
```

Add `VimNormalAction` to the test module imports. The test module already has `use super::*;` which imports everything from `mod.rs`, so `VimNormalAction` is available without any import change.

- [ ] **Step 7: Verify**

```
cargo test
```
Expected: 536 passed, 0 failed.

- [ ] **Step 8: Commit**

```bash
git add src/app/input/mod.rs src/app/input/vim_normal.rs src/app/input/vim_insert.rs
git commit -m "refactor(input): namespace vim normal actions under VimNormalAction"
```

---

## Task 4: Migrate `VimInsertAction`

**Files:** `src/app/input/mod.rs`, `src/app/input/vim_insert.rs`

6 variants. Arrow key motions already produce `VimNormal(...)` after Task 3, so they are not included here.

- [ ] **Step 1: Add `VimInsertAction` enum to `mod.rs` and update `UiAction`**

```rust
// Add after VimNormalAction:
#[derive(Debug, PartialEq, Eq)]
pub enum VimInsertAction {
    InsertChar(char),
    InsertNewline,
    InsertBackspace,
    DeleteWordBefore,
    InsertTab,
    ExitInsert,
}
```

In `UiAction`:
- **Remove**: `VimInsertChar(char)`, `VimInsertNewline`, `VimInsertBackspace`, `VimInsertDeleteWordBefore`, `VimInsertTab`, `VimExitInsert`
- **Add**: `VimInsert(VimInsertAction)`

- [ ] **Step 2: Replace `vim_insert.rs`**

```rust
use crate::app::state::{AppState, Focus};
use crate::app::input::{EventOutcome, UiAction, VimInsertAction, VimNormalAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Esc       => Some(UiAction::VimInsert(VimInsertAction::ExitInsert)),
        KeyCode::Enter     => Some(UiAction::VimInsert(VimInsertAction::InsertNewline)),
        KeyCode::Backspace => Some(UiAction::VimInsert(VimInsertAction::InsertBackspace)),
        KeyCode::Tab       => Some(UiAction::VimInsert(VimInsertAction::InsertTab)),
        KeyCode::Left      => Some(UiAction::VimNormal(VimNormalAction::MoveLeft)),
        KeyCode::Right     => Some(UiAction::VimNormal(VimNormalAction::MoveRight)),
        KeyCode::Up        => Some(UiAction::VimNormal(VimNormalAction::MoveUp)),
        KeyCode::Down      => Some(UiAction::VimNormal(VimNormalAction::MoveDown)),
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::VimInsert(VimInsertAction::DeleteWordBefore))
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
        {
            Some(UiAction::VimInsert(VimInsertAction::InsertChar(c)))
        }
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: VimInsertAction) -> Result<EventOutcome> {
    match action {
        VimInsertAction::ExitInsert       => exit_insert(state),
        VimInsertAction::InsertChar(c)    => insert_char(state, c),
        VimInsertAction::InsertNewline    => insert_newline(state),
        VimInsertAction::InsertBackspace  => insert_backspace(state),
        VimInsertAction::DeleteWordBefore => delete_word_before(state),
        VimInsertAction::InsertTab        => insert_tab(state),
    }
    Ok(EventOutcome::Continue)
}

// ── Handlers ───────────────────────────────────────────────────────────────────

fn exit_insert(state: &mut AppState) {
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

fn insert_char(state: &mut AppState, c: char) {
    let line = &mut state.doc.lines[state.vim.cursor_line];
    line.insert(state.vim.cursor_col, c);
    state.vim.cursor_col += c.len_utf8();
}

fn insert_newline(state: &mut AppState) {
    let tail = state.doc.lines[state.vim.cursor_line][state.vim.cursor_col..].to_string();
    state.doc.lines[state.vim.cursor_line].truncate(state.vim.cursor_col);
    state.vim.cursor_line += 1;
    state.doc.lines.insert(state.vim.cursor_line, tail);
    state.vim.cursor_col = 0;
}

fn insert_backspace(state: &mut AppState) {
    let col = state.vim.cursor_col;
    if col > 0 {
        let prev = super::prev_char_boundary(&state.doc.lines[state.vim.cursor_line], col);
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

fn delete_word_before(state: &mut AppState) {
    let col = state.vim.cursor_col;
    let new_col = super::prev_word_start(&state.doc.lines[state.vim.cursor_line], col);
    let line = &mut state.doc.lines[state.vim.cursor_line];
    line.drain(new_col..col);
    state.vim.cursor_col = new_col;
}

fn insert_tab(state: &mut AppState) {
    let line = &mut state.doc.lines[state.vim.cursor_line];
    line.insert_str(state.vim.cursor_col, "  ");
    state.vim.cursor_col += 2;
}
```

- [ ] **Step 3: Update delegation in `mod.rs::execute_action`**

```rust
// REMOVE:
        UiAction::VimInsertChar(_)
        | UiAction::VimInsertNewline
        | UiAction::VimInsertBackspace
        | UiAction::VimInsertDeleteWordBefore
        | UiAction::VimInsertTab
        | UiAction::VimExitInsert => return vim_insert::execute_action(state, action),

// REPLACE WITH:
        UiAction::VimInsert(a) => return vim_insert::execute_action(state, a),
```

Also update the Esc handler in `mod.rs::key_to_action`. Find the arm for `Focus::VimInsert`:
```rust
// BEFORE:
            Focus::VimInsert => Some(UiAction::VimExitInsert),
// AFTER:
            Focus::VimInsert => Some(UiAction::VimInsert(VimInsertAction::ExitInsert)),
```

- [ ] **Step 4: Update tests in `mod.rs`**

```rust
// tab_in_viminsert_inserts_tab — assertion:
Some(UiAction::VimInsert(VimInsertAction::InsertTab))

// viminsert_esc_exits_insert — assertion:
Some(UiAction::VimInsert(VimInsertAction::ExitInsert))

// viminsert_char_emits_insert_char — assertion:
Some(UiAction::VimInsert(VimInsertAction::InsertChar('a')))

// vim_insert_tab_inserts_two_spaces — execute_action call:
execute_action(&mut state, UiAction::VimInsert(VimInsertAction::InsertTab)).unwrap();

// vim_insert_char_adds_to_line — execute_action call:
execute_action(&mut state, UiAction::VimInsert(VimInsertAction::InsertChar('!'))).unwrap();

// vim_insert_newline_splits_line — execute_action call:
execute_action(&mut state, UiAction::VimInsert(VimInsertAction::InsertNewline)).unwrap();

// vim_insert_backspace_removes_char — execute_action call:
execute_action(&mut state, UiAction::VimInsert(VimInsertAction::InsertBackspace)).unwrap();

// vim_insert_backspace_at_line_start_merges_with_prev — execute_action call:
execute_action(&mut state, UiAction::VimInsert(VimInsertAction::InsertBackspace)).unwrap();

// vim_undo_restores_snapshot — the VimExitInsert line (other two lines updated in Task 3):
execute_action(&mut state, UiAction::VimInsert(VimInsertAction::ExitInsert)).unwrap();
```

Also update the import in `vim_insert.rs` for the `VimNormalAction` usage — already included in the replacement file above.

- [ ] **Step 5: Verify**

```
cargo test
```
Expected: 536 passed, 0 failed.

- [ ] **Step 6: Commit**

```bash
git add src/app/input/mod.rs src/app/input/vim_insert.rs
git commit -m "refactor(input): namespace vim insert actions under VimInsertAction"
```

---

## Task 5: Migrate `CaptureAction`

**Files:** `src/app/input/mod.rs`, `src/app/input/capture.rs`

20 variants. `CancelEdit` is currently an inline handler in `mod.rs` — this task moves it into `capture::execute_action`. `ExitCaptureMode` stays flat — it changes focus to VimNormal and is a cross-cutting concern migrated in Task 6.

- [ ] **Step 1: Add `CaptureAction` enum to `mod.rs` and update `UiAction`**

```rust
// Add after VimInsertAction:
#[derive(Debug, PartialEq, Eq)]
pub enum CaptureAction {
    TypeChar(char),
    DeleteChar,
    TypeNewline,
    TypeIndent,
    PrependIndent,
    RemoveIndent,
    SubmitInput,
    CommitEdit,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorLineStart,
    MoveCursorLineEnd,
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    ToggleSelected,
    BeginEdit,
    ResumeHeading,
    CancelEdit,
}
```

In `UiAction`:
- **Remove**: `TypeChar(char)`, `DeleteChar`, `TypeNewline`, `TypeIndent`, `PrependIndent`, `RemoveIndent`, `SubmitInput`, `CommitEdit`, `MoveCursorLeft`, `MoveCursorRight`, `MoveCursorLineStart`, `MoveCursorLineEnd`, `SelectNext`, `SelectPrev`, `SelectFirst`, `SelectLast`, `ToggleSelected`, `BeginEdit`, `ResumeHeading`, `CancelEdit`
- **Add**: `Capture(CaptureAction)`
- **Keep unchanged**: everything else (the focus/overlay/global variants migrated in Task 6)

- [ ] **Step 2: Replace `capture.rs`**

```rust
use crate::app::state::AppState;
use crate::app::input::{CaptureAction, EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Enter => {
            if state.editing.is_some() {
                Some(UiAction::Capture(CaptureAction::CommitEdit))
            } else {
                Some(UiAction::Capture(CaptureAction::SubmitInput))
            }
        }
        KeyCode::Backspace => Some(UiAction::Capture(CaptureAction::DeleteChar)),
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::Capture(CaptureAction::TypeNewline))
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
        {
            Some(UiAction::Capture(CaptureAction::TypeChar(c)))
        }
        KeyCode::Left  => Some(UiAction::Capture(CaptureAction::MoveCursorLeft)),
        KeyCode::Right => Some(UiAction::Capture(CaptureAction::MoveCursorRight)),
        KeyCode::Home  => Some(UiAction::Capture(CaptureAction::MoveCursorLineStart)),
        KeyCode::End   => Some(UiAction::Capture(CaptureAction::MoveCursorLineEnd)),
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::Capture(CaptureAction::MoveCursorLineStart))
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::Capture(CaptureAction::MoveCursorLineEnd))
        }
        KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::Capture(CaptureAction::PrependIndent))
        }
        KeyCode::Up | KeyCode::Down => None,
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: CaptureAction) -> Result<EventOutcome> {
    match action {
        CaptureAction::TypeChar(c)         => type_char(state, c),
        CaptureAction::DeleteChar          => delete_char(state),
        CaptureAction::TypeNewline         => type_newline(state),
        CaptureAction::TypeIndent          => type_indent(state),
        CaptureAction::PrependIndent       => prepend_indent(state),
        CaptureAction::RemoveIndent        => remove_indent(state),
        CaptureAction::SubmitInput         => submit_input(state)?,
        CaptureAction::CommitEdit          => crate::app::actions::commit_edit(state)?,
        CaptureAction::MoveCursorLeft      => move_cursor_left(state),
        CaptureAction::MoveCursorRight     => move_cursor_right(state),
        CaptureAction::MoveCursorLineStart => move_cursor_line_start(state),
        CaptureAction::MoveCursorLineEnd   => move_cursor_line_end(state),
        CaptureAction::SelectNext          => crate::app::actions::select_next(state),
        CaptureAction::SelectPrev          => crate::app::actions::select_prev(state),
        CaptureAction::SelectFirst         => crate::app::actions::select_first(state),
        CaptureAction::SelectLast          => crate::app::actions::select_last(state),
        CaptureAction::ToggleSelected      => crate::app::actions::toggle_selected(state),
        CaptureAction::BeginEdit           => crate::app::actions::begin_edit_selected(state),
        CaptureAction::ResumeHeading       => crate::app::actions::resume_selected_heading(state),
        CaptureAction::CancelEdit          => {
            state.editing = None;
            state.input.clear();
            state.cursor_pos = 0;
        }
    }
    Ok(EventOutcome::Continue)
}

// ── Shared helper ──────────────────────────────────────────────────────────────

/// Byte offset of the start of the line containing `cursor_pos`.
fn current_line_start(input: &str, cursor_pos: usize) -> usize {
    match input[..cursor_pos].rfind('\n') {
        Some(nl) => nl + 1,
        None => 0,
    }
}

// ── Input handlers ─────────────────────────────────────────────────────────────

fn type_char(state: &mut AppState, c: char) {
    state.input.insert(state.cursor_pos, c);
    state.cursor_pos += c.len_utf8();
}

fn delete_char(state: &mut AppState) {
    if state.cursor_pos > 0 {
        let prev = super::prev_char_boundary(&state.input, state.cursor_pos);
        state.input.remove(prev);
        state.cursor_pos = prev;
    }
}

fn type_newline(state: &mut AppState) {
    state.input.insert(state.cursor_pos, '\n');
    state.cursor_pos += 1;
}

fn type_indent(state: &mut AppState) {
    state.input.insert_str(state.cursor_pos, "->");
    state.cursor_pos += 2;
}

fn prepend_indent(state: &mut AppState) {
    let line_start = current_line_start(&state.input, state.cursor_pos);
    state.input.insert_str(line_start, "->");
    state.cursor_pos += 2;
}

fn remove_indent(state: &mut AppState) {
    let line_start = current_line_start(&state.input, state.cursor_pos);
    if state.input[line_start..].starts_with("->") {
        state.input.drain(line_start..line_start + 2);
        if state.cursor_pos > line_start {
            state.cursor_pos = state.cursor_pos.saturating_sub(2).max(line_start);
        }
    }
}

fn submit_input(state: &mut AppState) -> Result<()> {
    let cmd = crate::app::command::parse(&state.input);
    crate::app::actions::dispatch(state, cmd)?;
    state.input.clear();
    state.cursor_pos = 0;
    state.doc_anchor_line =
        crate::app::context::context_heading_line(&state.doc.lines, &state.context);
    Ok(())
}

// ── Cursor movement handlers ───────────────────────────────────────────────────

fn move_cursor_left(state: &mut AppState) {
    state.cursor_pos = super::prev_char_boundary(&state.input, state.cursor_pos);
}

fn move_cursor_right(state: &mut AppState) {
    state.cursor_pos = super::next_char_boundary(&state.input, state.cursor_pos);
}

fn move_cursor_line_start(state: &mut AppState) {
    state.cursor_pos = current_line_start(&state.input, state.cursor_pos);
}

fn move_cursor_line_end(state: &mut AppState) {
    let after = &state.input[state.cursor_pos..];
    state.cursor_pos = match after.find('\n') {
        Some(nl_offset) => state.cursor_pos + nl_offset,
        None => state.input.len(),
    };
}
```

- [ ] **Step 3: Update `mod.rs::execute_action`**

Remove the large capture delegation blob and the inline `CancelEdit` handler, replace with a single arm:

```rust
// REMOVE both of these blocks:

        UiAction::CancelEdit => {
            state.editing = None;
            state.input.clear();
            state.cursor_pos = 0;
        }

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

// REPLACE WITH:
        UiAction::Capture(a) => return capture::execute_action(state, a),
```

- [ ] **Step 4: Update `mod.rs::key_to_action`** — update all sites that produce old flat capture variants

**Tab focus cycle** (captures Tab for Capture mode):
```rust
// BEFORE:
            Focus::Capture => return Some(UiAction::TypeIndent),
// AFTER:
            Focus::Capture => return Some(UiAction::Capture(CaptureAction::TypeIndent)),
```

**BackTab focus cycle**:
```rust
// BEFORE:
            Focus::Capture => Some(UiAction::RemoveIndent),
// AFTER:
            Focus::Capture => Some(UiAction::Capture(CaptureAction::RemoveIndent)),
```

**Esc handler for Capture**:
```rust
// BEFORE:
            Focus::Capture => {
                if state.editing.is_some() {
                    Some(UiAction::CancelEdit)
                } else {
                    Some(UiAction::ExitCaptureMode)
                }
            }
// AFTER:
            Focus::Capture => {
                if state.editing.is_some() {
                    Some(UiAction::Capture(CaptureAction::CancelEdit))
                } else {
                    Some(UiAction::ExitCaptureMode)  // stays flat until Task 6
                }
            }
```

- [ ] **Step 5: Update tests in `mod.rs`**

Update all tests that reference old flat capture action variants:

```rust
// tab_in_capture_inserts_indent — assertion:
Some(UiAction::Capture(CaptureAction::TypeIndent))

// backtab_in_capture_emits_remove_indent — assertion:
Some(UiAction::Capture(CaptureAction::RemoveIndent))

// esc_in_capture_with_editing_cancels_edit — assertion:
Some(UiAction::Capture(CaptureAction::CancelEdit))

// type_char_appends_to_input — execute_action call:
execute_action(&mut state, UiAction::Capture(CaptureAction::TypeChar('a'))).unwrap();

// type_char_multiple_appends_in_order — two execute_action calls:
execute_action(&mut state, UiAction::Capture(CaptureAction::TypeChar('h'))).unwrap();
execute_action(&mut state, UiAction::Capture(CaptureAction::TypeChar('i'))).unwrap();

// type_char_inserts_at_cursor_pos:
execute_action(&mut state, UiAction::Capture(CaptureAction::TypeChar('b'))).unwrap();

// delete_char_pops_last_char:
execute_action(&mut state, UiAction::Capture(CaptureAction::DeleteChar)).unwrap();

// delete_char_removes_char_before_cursor:
execute_action(&mut state, UiAction::Capture(CaptureAction::DeleteChar)).unwrap();

// delete_char_at_start_is_noop:
execute_action(&mut state, UiAction::Capture(CaptureAction::DeleteChar)).unwrap();

// delete_char_on_empty_input_is_noop:
execute_action(&mut state, UiAction::Capture(CaptureAction::DeleteChar)).unwrap();

// type_newline_pushes_newline_char:
execute_action(&mut state, UiAction::Capture(CaptureAction::TypeNewline)).unwrap();

// type_newline_inserts_at_cursor_pos:
execute_action(&mut state, UiAction::Capture(CaptureAction::TypeNewline)).unwrap();

// type_indent_inserts_two_spaces:
execute_action(&mut state, UiAction::Capture(CaptureAction::TypeIndent)).unwrap();

// type_indent_inserts_at_cursor_pos:
execute_action(&mut state, UiAction::Capture(CaptureAction::TypeIndent)).unwrap();

// cancel_edit_clears_editing_and_input:
execute_action(&mut state, UiAction::Capture(CaptureAction::CancelEdit)).unwrap();

// cursor_pos_reset_to_zero_on_submit:
execute_action(&mut state, UiAction::Capture(CaptureAction::SubmitInput)).unwrap();

// cursor_pos_reset_to_zero_on_cancel_edit:
execute_action(&mut state, UiAction::Capture(CaptureAction::CancelEdit)).unwrap();

// remove_indent_removes_arrow_from_line_start:
execute_action(&mut state, UiAction::Capture(CaptureAction::RemoveIndent)).unwrap();

// remove_indent_adjusts_cursor_pos_past_line_start:
execute_action(&mut state, UiAction::Capture(CaptureAction::RemoveIndent)).unwrap();

// remove_indent_clamps_cursor_to_line_start:
execute_action(&mut state, UiAction::Capture(CaptureAction::RemoveIndent)).unwrap();

// remove_indent_noop_when_no_arrow:
execute_action(&mut state, UiAction::Capture(CaptureAction::RemoveIndent)).unwrap();

// remove_indent_on_second_line_uses_line_start:
execute_action(&mut state, UiAction::Capture(CaptureAction::RemoveIndent)).unwrap();

// remove_indent_cursor_at_line_start_not_adjusted:
execute_action(&mut state, UiAction::Capture(CaptureAction::RemoveIndent)).unwrap();

// submit_input_anchors_to_context_heading:
execute_action(&mut state, UiAction::Capture(CaptureAction::SubmitInput)).unwrap();
```

- [ ] **Step 6: Verify**

```
cargo test
```
Expected: 536 passed, 0 failed.

- [ ] **Step 7: Commit**

```bash
git add src/app/input/mod.rs src/app/input/capture.rs
git commit -m "refactor(input): namespace capture mode actions under CaptureAction"
```

---

## Task 6: Extract `GlobalAction`, `OverlayAction`, and `FocusAction`

**Files:** `src/app/input/mod.rs`, `src/app/input/vim_normal.rs`

After Tasks 1–5, the remaining flat variants in `UiAction` are:
`Quit`, `GoToday`, `PrevDay`, `NextDay`, `ToggleChat`, `OpenHelp`, `CloseHelp`, `ExitCaptureMode`, `ExitVimNormal`, `SwitchToCapture`, `FocusVimNormal`, `FocusRightPanel`, `RightPanelBlur`, `FocusChat`, `ChatBlur`.

This task groups them into three sub-enums and replaces all inline state mutation in `execute_action` with calls to three focused helper functions.

- [ ] **Step 1: Add the three sub-enums to `mod.rs`**

```rust
// Add after CaptureAction:
#[derive(Debug, PartialEq, Eq)]
pub enum GlobalAction {
    GoToday,
    PrevDay,
    NextDay,
    ToggleChat,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OverlayAction {
    OpenHelp,
    CloseHelp,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FocusAction {
    SwitchToCapture,
    ExitCaptureMode,
    ExitVimNormal,
    FocusVimNormal,
    FocusRightPanel,
    RightPanelBlur,
    FocusChat,
    ChatBlur,
}
```

In `UiAction`:
- **Remove all remaining flat variants** except `Quit`
- **Add**: `Global(GlobalAction)`, `Overlay(OverlayAction)`, `Focus(FocusAction)`

The final `UiAction` enum:
```rust
#[derive(Debug, PartialEq, Eq)]
pub enum UiAction {
    Quit,
    Global(GlobalAction),
    Overlay(OverlayAction),
    Focus(FocusAction),
    Capture(CaptureAction),
    VimNormal(VimNormalAction),
    VimInsert(VimInsertAction),
    RightPanel(RightPanelAction),
    Chat(ChatAction),
}
```

- [ ] **Step 2: Replace `mod.rs::execute_action` with a clean dispatcher + three helpers**

Replace the entire `execute_action` function and add three private helper functions immediately after it:

```rust
pub fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::Quit           => return Ok(EventOutcome::Quit),
        UiAction::Global(a)      => execute_global(state, a)?,
        UiAction::Overlay(a)     => execute_overlay(state, a),
        UiAction::Focus(a)       => execute_focus(state, a),
        UiAction::Capture(a)     => return capture::execute_action(state, a),
        UiAction::VimNormal(a)   => return vim_normal::execute_action(state, a),
        UiAction::VimInsert(a)   => return vim_insert::execute_action(state, a),
        UiAction::RightPanel(a)  => return right_panel::execute_action(state, a),
        UiAction::Chat(a)        => return chat::execute_action(state, a),
    }
    if state.should_quit {
        return Ok(EventOutcome::Quit);
    }
    Ok(EventOutcome::Continue)
}

fn execute_global(state: &mut AppState, action: GlobalAction) -> Result<()> {
    match action {
        GlobalAction::GoToday => {
            crate::app::actions::go_today(state)?;
            state.status.clear();
        }
        GlobalAction::PrevDay    => crate::app::actions::go_prev_day(state)?,
        GlobalAction::NextDay    => crate::app::actions::go_next_day(state)?,
        GlobalAction::ToggleChat => {
            state.chat.visible = !state.chat.visible;
            if !state.chat.visible && state.focus == Focus::Chat {
                state.focus = Focus::Capture;
            }
        }
    }
    Ok(())
}

fn execute_overlay(state: &mut AppState, action: OverlayAction) {
    match action {
        OverlayAction::OpenHelp  => state.overlay = Overlay::Help,
        OverlayAction::CloseHelp => state.overlay = Overlay::None,
    }
}

fn execute_focus(state: &mut AppState, action: FocusAction) {
    match action {
        FocusAction::ExitCaptureMode => {
            state.focus = Focus::VimNormal;
            state.vim.cursor_line = state.doc_anchor_line;
            state.vim.cursor_col = 0;
        }
        FocusAction::ExitVimNormal   => state.focus = Focus::Capture,
        FocusAction::SwitchToCapture => {
            state.focus = Focus::Capture;
            state.doc_anchor_line =
                crate::app::context::context_heading_line(&state.doc.lines, &state.context);
        }
        FocusAction::FocusVimNormal  => state.focus = Focus::VimNormal,
        FocusAction::FocusRightPanel => {
            state.right_panel_selected = 0;
            state.focus = Focus::RightPanel;
        }
        FocusAction::RightPanelBlur  => state.focus = Focus::Capture,
        FocusAction::FocusChat       => state.focus = Focus::Chat,
        FocusAction::ChatBlur        => state.focus = Focus::Capture,
    }
}
```

- [ ] **Step 3: Update `mod.rs::key_to_action` to produce new namespaced variants**

Update every site in `key_to_action` that still produces an old flat variant. Apply all of the following changes:

```rust
// Help overlay return:
// BEFORE: Some(UiAction::CloseHelp)
// AFTER:  Some(UiAction::Overlay(OverlayAction::CloseHelp))

// Global Ctrl hotkeys:
// BEFORE: KeyCode::Char('t') => return Some(UiAction::GoToday),
// AFTER:  KeyCode::Char('t') => return Some(UiAction::Global(GlobalAction::GoToday)),

// BEFORE: KeyCode::Char('l') => return Some(UiAction::ToggleChat),
// AFTER:  KeyCode::Char('l') => return Some(UiAction::Global(GlobalAction::ToggleChat)),

// Tab focus cycle:
// BEFORE: Focus::VimNormal => return Some(UiAction::FocusRightPanel),
// AFTER:  Focus::VimNormal => return Some(UiAction::Focus(FocusAction::FocusRightPanel)),

// BEFORE: Focus::Chat => return Some(UiAction::FocusRightPanel),
// AFTER:  Focus::Chat => return Some(UiAction::Focus(FocusAction::FocusRightPanel)),

// BEFORE: Focus::RightPanel => return Some(UiAction::FocusVimNormal),
// AFTER:  Focus::RightPanel => return Some(UiAction::Focus(FocusAction::FocusVimNormal)),

// BackTab focus cycle:
// BEFORE: Focus::VimNormal | Focus::VimInsert => Some(UiAction::FocusRightPanel),
// AFTER:  Focus::VimNormal | Focus::VimInsert => Some(UiAction::Focus(FocusAction::FocusRightPanel)),

// BEFORE: Focus::Chat => Some(UiAction::FocusVimNormal),
// AFTER:  Focus::Chat => Some(UiAction::Focus(FocusAction::FocusVimNormal)),

// BEFORE: Focus::RightPanel => Some(UiAction::FocusVimNormal),
// AFTER:  Focus::RightPanel => Some(UiAction::Focus(FocusAction::FocusVimNormal)),

// Esc handler:
// BEFORE: Focus::Capture => { ... Some(UiAction::ExitCaptureMode) ... }
// AFTER:  Focus::Capture => { ... Some(UiAction::Focus(FocusAction::ExitCaptureMode)) ... }

// BEFORE: Focus::VimNormal => Some(UiAction::SwitchToCapture),
// AFTER:  Focus::VimNormal => Some(UiAction::Focus(FocusAction::SwitchToCapture)),

// BEFORE: Focus::RightPanel => Some(UiAction::RightPanelBlur),
// AFTER:  Focus::RightPanel => Some(UiAction::Focus(FocusAction::RightPanelBlur)),

// BEFORE: Focus::Chat => Some(UiAction::ChatBlur),
// AFTER:  Focus::Chat => Some(UiAction::Focus(FocusAction::ChatBlur)),

// Day navigation ([ and ]):
// BEFORE: KeyCode::Char('[') => return Some(UiAction::PrevDay),
// AFTER:  KeyCode::Char('[') => return Some(UiAction::Global(GlobalAction::PrevDay)),

// BEFORE: KeyCode::Char(']') => return Some(UiAction::NextDay),
// AFTER:  KeyCode::Char(']') => return Some(UiAction::Global(GlobalAction::NextDay)),
```

- [ ] **Step 4: Update `vim_normal.rs::key_to_action`** — two remaining flat variant productions

```rust
// BEFORE: KeyCode::Char('?') => Some(UiAction::OpenHelp),
// AFTER:  KeyCode::Char('?') => Some(UiAction::Overlay(OverlayAction::OpenHelp)),

// BEFORE: KeyCode::Tab => Some(UiAction::SwitchToCapture),
// AFTER:  KeyCode::Tab => Some(UiAction::Focus(FocusAction::SwitchToCapture)),
```

Also update the import in `vim_normal.rs`:
```rust
// BEFORE:
use crate::app::input::{EventOutcome, UiAction, VimNormalAction};
// AFTER:
use crate::app::input::{EventOutcome, FocusAction, OverlayAction, UiAction, VimNormalAction};
```

- [ ] **Step 5: Update tests in `mod.rs`**

```rust
// help_overlay_esc_closes — assertion:
Some(UiAction::Overlay(OverlayAction::CloseHelp))

// open_help_sets_overlay:
execute_action(&mut state, UiAction::Overlay(OverlayAction::OpenHelp)).unwrap();

// close_help_clears_overlay:
execute_action(&mut state, UiAction::Overlay(OverlayAction::CloseHelp)).unwrap();

// esc_in_capture_without_editing_exits_capture — assertion:
Some(UiAction::Focus(FocusAction::ExitCaptureMode))

// esc_in_vimnormal_switches_to_capture — assertion:
Some(UiAction::Focus(FocusAction::SwitchToCapture))

// esc_in_chat_blurs_to_capture — assertion:
Some(UiAction::Focus(FocusAction::ChatBlur))

// exit_capture_mode_switches_focus_to_navigate:
execute_action(&mut state, UiAction::Focus(FocusAction::ExitCaptureMode)).unwrap();

// exit_vimnormal_mode_switches_focus_to_capture:
execute_action(&mut state, UiAction::Focus(FocusAction::ExitVimNormal)).unwrap();

// switch_to_capture_sets_focus:
execute_action(&mut state, UiAction::Focus(FocusAction::SwitchToCapture)).unwrap();

// switch_to_capture_sets_anchor_to_context_heading:
execute_action(&mut state, UiAction::Focus(FocusAction::SwitchToCapture)).unwrap();

// focus_vimnormal_sets_focus_to_vimnormal:
execute_action(&mut state, UiAction::Focus(FocusAction::FocusVimNormal)).unwrap();

// focus_chat_sets_focus:
execute_action(&mut state, UiAction::Focus(FocusAction::FocusChat)).unwrap();

// ctrl_t_goes_today — assertion:
Some(UiAction::Global(GlobalAction::GoToday))

// tab_in_vimnormal_focuses_right_panel — assertion:
Some(UiAction::Focus(FocusAction::FocusRightPanel))

// tab_from_chat_goes_to_right_panel — assertion:
Some(UiAction::Focus(FocusAction::FocusRightPanel))

// tab_in_right_panel_wraps_to_vimnormal — assertion:
Some(UiAction::Focus(FocusAction::FocusVimNormal))

// backtab_in_vimnormal_wraps_to_right_panel — assertion:
Some(UiAction::Focus(FocusAction::FocusRightPanel))

// backtab_in_chat_goes_to_navigate — assertion:
Some(UiAction::Focus(FocusAction::FocusVimNormal))

// backtab_in_right_panel_goes_to_vimnormal — assertion:
Some(UiAction::Focus(FocusAction::FocusVimNormal))
```

- [ ] **Step 6: Verify**

```
cargo test
```
Expected: 536 passed, 0 failed.

- [ ] **Step 7: Commit**

```bash
git add src/app/input/mod.rs src/app/input/vim_normal.rs
git commit -m "refactor(input): extract GlobalAction, OverlayAction, FocusAction; flatten execute_action to 9-arm dispatcher"
```

---

## Task 7: Extract `key_to_action` Pipeline Guards

**Files:** `src/app/input/mod.rs`

Pure structural refactor — no behavior change, no test updates required. The 7 sequential guard steps in `key_to_action` become named private functions chained with `.or_else()`.

- [ ] **Step 1: Replace `key_to_action` and add the 7 helper functions**

Replace the entire `key_to_action` function and add the helpers immediately after it (before `execute_action`):

```rust
pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    global_quit(key)
        .or_else(|| overlay_keys(state, key))
        .or_else(|| global_ctrl_hotkeys(key))
        .or_else(|| focus_cycle_keys(state, key))
        .or_else(|| esc_keys(state, key))
        .or_else(|| day_navigation(state, key))
        .or_else(|| mode_dispatch(state, key))
}

fn global_quit(key: KeyEvent) -> Option<UiAction> {
    (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        .then_some(UiAction::Quit)
}

fn overlay_keys(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    if state.overlay != Overlay::Help {
        return None;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') => Some(UiAction::Overlay(OverlayAction::CloseHelp)),
        _ => None,
    }
}

fn global_ctrl_hotkeys(key: KeyEvent) -> Option<UiAction> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    match key.code {
        KeyCode::Char('t') => Some(UiAction::Global(GlobalAction::GoToday)),
        KeyCode::Char('l') => Some(UiAction::Global(GlobalAction::ToggleChat)),
        _ => None,
    }
}

fn focus_cycle_keys(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Tab => match state.focus {
            Focus::Capture   => Some(UiAction::Capture(CaptureAction::TypeIndent)),
            Focus::VimNormal => Some(UiAction::Focus(FocusAction::FocusRightPanel)),
            Focus::VimInsert => None, // falls through to vim_insert::key_to_action
            Focus::Chat      => Some(UiAction::Focus(FocusAction::FocusRightPanel)),
            Focus::RightPanel => Some(UiAction::Focus(FocusAction::FocusVimNormal)),
        },
        KeyCode::BackTab => match state.focus {
            Focus::Capture              => Some(UiAction::Capture(CaptureAction::RemoveIndent)),
            Focus::VimNormal | Focus::VimInsert => Some(UiAction::Focus(FocusAction::FocusRightPanel)),
            Focus::Chat                 => Some(UiAction::Focus(FocusAction::FocusVimNormal)),
            Focus::RightPanel           => Some(UiAction::Focus(FocusAction::FocusVimNormal)),
        },
        _ => None,
    }
}

fn esc_keys(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    if key.code != KeyCode::Esc {
        return None;
    }
    match state.focus {
        Focus::Capture => {
            if state.editing.is_some() {
                Some(UiAction::Capture(CaptureAction::CancelEdit))
            } else {
                Some(UiAction::Focus(FocusAction::ExitCaptureMode))
            }
        }
        Focus::VimNormal  => Some(UiAction::Focus(FocusAction::SwitchToCapture)),
        Focus::VimInsert  => Some(UiAction::VimInsert(VimInsertAction::ExitInsert)),
        Focus::RightPanel => Some(UiAction::Focus(FocusAction::RightPanelBlur)),
        Focus::Chat       => Some(UiAction::Focus(FocusAction::ChatBlur)),
    }
}

fn day_navigation(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    let can_navigate = matches!(state.focus, Focus::VimNormal)
        || (matches!(state.focus, Focus::Capture) && state.input.is_empty());
    if !can_navigate {
        return None;
    }
    match key.code {
        KeyCode::Char('[') => Some(UiAction::Global(GlobalAction::PrevDay)),
        KeyCode::Char(']') => Some(UiAction::Global(GlobalAction::NextDay)),
        _ => None,
    }
}

fn mode_dispatch(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match state.focus {
        Focus::Capture    => capture::key_to_action(state, key),
        Focus::RightPanel => right_panel::key_to_action(state, key),
        Focus::Chat       => chat::key_to_action(state, key),
        Focus::VimNormal  => vim_normal::key_to_action(state, key),
        Focus::VimInsert  => vim_insert::key_to_action(state, key),
    }
}
```

- [ ] **Step 2: Verify**

```
cargo test
```
Expected: 536 passed, 0 failed.

- [ ] **Step 3: Commit**

```bash
git add src/app/input/mod.rs
git commit -m "refactor(input): extract key_to_action guards into named pipeline functions"
```

---

## Self-Review

**Spec coverage:**
- ✅ Flat `UiAction` replaced with 8 namespaced sub-enums (Tasks 1–6)
- ✅ `execute_action` in `mod.rs` becomes 9-arm clean dispatcher (Task 6)
- ✅ All `_ => unreachable!()` guards in sub-modules eliminated (Tasks 1–5)
- ✅ Inline state mutation removed from `mod.rs::execute_action` into `execute_focus`/`execute_overlay`/`execute_global` (Task 6)
- ✅ `key_to_action` pipeline extracted into named functions (Task 7)
- ✅ `CancelEdit` moved from inline `mod.rs` handler into `capture::execute_action` (Task 5)

**Placeholder scan:** None found. All steps include complete code.

**Type consistency check:**
- `VimNormalAction::BeginEditLine` produced in `vim_normal::key_to_action` ✅ consumed in `vim_normal::execute_action`
- `VimInsertAction::ExitInsert` produced in `vim_insert::key_to_action` and `esc_keys()` ✅ consumed in `vim_insert::execute_action`
- `FocusAction::SwitchToCapture` produced in `esc_keys()` and `vim_normal::key_to_action` ✅ consumed in `execute_focus`
- All sub-enum names are consistent across production and consumption sites
