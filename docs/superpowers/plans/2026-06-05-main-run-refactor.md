# main.rs::run Refactor — Two-Stage Key Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the 247-line, complexity-81 `run()` function in `src/main.rs` into a thin orchestrator by extracting all key-handling logic into a new `src/app/input.rs` module using a two-stage pure-mapping + effectful-execution pattern.

**Architecture:** `key_to_action(state, key) -> Option<UiAction>` is a pure function that maps the current app state and a keypress to a semantic action; `execute_action(state, action) -> Result<EventOutcome>` applies that action to state. `run()` becomes a thin loop that calls both. CLI arg parsing is extracted to `parse_cli_args()`.

**Tech Stack:** Rust, ratatui 0.29, crossterm 0.29, tempfile 3 (tests)

**Spec:** `docs/superpowers/specs/2026-06-05-main-run-refactor-design.md`

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src/app/mod.rs` | Add `pub mod input;` |
| Create | `src/app/input.rs` | `EventOutcome`, `UiAction`, `key_to_action`, `execute_action`, tests |
| Modify | `src/main.rs` | Extract `parse_cli_args()`; thin `run()` loop |

---

## Task 1: Bootstrap `src/app/input.rs` — enums and module wiring

**Files:**
- Modify: `src/app/mod.rs`
- Create: `src/app/input.rs`

- [ ] **Step 1: Add `pub mod input;` to `src/app/mod.rs`**

File should read:
```rust
pub mod actions;
pub mod command;
pub mod input;
pub mod state;
```

- [ ] **Step 2: Create `src/app/input.rs` with enums only**

```rust
use crate::app::state::{AppState, Focus, Overlay};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(PartialEq, Eq, Debug)]
pub enum EventOutcome {
    Continue,
    Quit,
}

#[derive(Debug, PartialEq)]
pub enum UiAction {
    // Universal
    Quit,

    // Calendar overlay
    MoveCalendar { dx: i8, dy: i8 },
    ConfirmCalendar,
    CloseCalendar,

    // Help overlay
    CloseHelp,

    // Global hotkeys
    GoToday,
    OpenCalendar,
    PrevDay,
    NextDay,

    // Escape handling (context-dependent)
    CancelEdit,
    ExitCaptureMode,
    ExitNavigateMode,

    // Capture mode
    TypeChar(char),
    DeleteChar,
    TypeNewline,
    SubmitInput,
    CommitEdit,

    // Navigate mode
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    ToggleSelected,
    BeginEdit,
    InitiateDelete,
    ConfirmDelete,
    CancelDelete,
    ResumeHeading,
    OpenHelp,
    SwitchToCapture,
}

pub fn key_to_action(_state: &AppState, _key: KeyEvent) -> Option<UiAction> {
    todo!()
}

pub fn execute_action(_state: &mut AppState, _action: UiAction) -> Result<EventOutcome> {
    todo!()
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check
```

Expected: compiles without errors (warnings about unused imports are fine; `todo!()` stubs are expected).

- [ ] **Step 4: Commit**

```bash
git add src/app/mod.rs src/app/input.rs
git commit -m "feat: bootstrap src/app/input.rs with EventOutcome and UiAction enums"
```

---

## Task 2: Implement `key_to_action` with tests

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Add failing tests for `key_to_action`**

Append this `#[cfg(test)]` block to `src/app/input.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use chrono::NaiveDate;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn test_state(tmp: &tempfile::TempDir) -> AppState {
        AppState::open_day(
            tmp.path().to_path_buf(),
            Config::default(),
            NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
        )
        .unwrap()
    }

    // --- key_to_action tests ---

    #[test]
    fn ctrl_c_always_quits() {
        let tmp = tempfile::tempdir().unwrap();
        let state = test_state(&tmp);
        assert_eq!(key_to_action(&state, ctrl(KeyCode::Char('c'))), Some(UiAction::Quit));
    }

    #[test]
    fn navigate_j_selects_next() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('j'))),
            Some(UiAction::SelectNext)
        );
    }

    #[test]
    fn navigate_down_selects_next() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Down)),
            Some(UiAction::SelectNext)
        );
    }

    #[test]
    fn navigate_pending_delete_d_confirms() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('d'))),
            Some(UiAction::ConfirmDelete)
        );
    }

    #[test]
    fn navigate_pending_delete_other_key_cancels() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('j'))),
            Some(UiAction::CancelDelete)
        );
    }

    #[test]
    fn capture_bracket_empty_input_is_prev_day() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        // input is empty by default
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('['))),
            Some(UiAction::PrevDay)
        );
    }

    #[test]
    fn capture_bracket_nonempty_input_types_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        state.input.push('x');
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('['))),
            Some(UiAction::TypeChar('['))
        );
    }

    #[test]
    fn capture_ctrl_j_is_newline() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, ctrl(KeyCode::Char('j'))),
            Some(UiAction::TypeNewline)
        );
    }

    #[test]
    fn calendar_overlay_left_moves_calendar() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Calendar;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Left)),
            Some(UiAction::MoveCalendar { dx: -1, dy: 0 })
        );
    }

    #[test]
    fn calendar_overlay_ignores_j() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Calendar;
        assert_eq!(key_to_action(&state, make_key(KeyCode::Char('j'))), None);
    }

    #[test]
    fn help_overlay_esc_closes() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Help;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::CloseHelp)
        );
    }

    #[test]
    fn help_overlay_ignores_other_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Help;
        assert_eq!(key_to_action(&state, make_key(KeyCode::Char('j'))), None);
    }

    #[test]
    fn esc_in_capture_with_editing_cancels_edit() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        state.editing = Some(0);
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::CancelEdit)
        );
    }

    #[test]
    fn esc_in_capture_without_editing_exits_capture() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::ExitCaptureMode)
        );
    }

    #[test]
    fn esc_in_navigate_exits_navigate() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::ExitNavigateMode)
        );
    }

    #[test]
    fn ctrl_t_goes_today() {
        let tmp = tempfile::tempdir().unwrap();
        let state = test_state(&tmp);
        assert_eq!(key_to_action(&state, ctrl(KeyCode::Char('t'))), Some(UiAction::GoToday));
    }

    #[test]
    fn navigate_ctrl_combo_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        // Ctrl-X is not a recognized combo — should be None in navigate mode
        assert_eq!(key_to_action(&state, ctrl(KeyCode::Char('x'))), None);
    }
}
```

- [ ] **Step 2: Run the tests — expect failures**

```bash
cargo test key_to_action 2>&1 | head -20
```

Expected: tests fail with `not yet implemented` (panics from `todo!()`).

- [ ] **Step 3: Implement `key_to_action`**

Replace the `pub fn key_to_action(_state: &AppState, _key: KeyEvent) -> Option<UiAction>` stub with:

```rust
pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    // 1. Ctrl-C always quits regardless of mode
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(UiAction::Quit);
    }

    // 2. Calendar overlay — only handles its own keys
    if state.overlay == Overlay::Calendar {
        return match key.code {
            KeyCode::Left => Some(UiAction::MoveCalendar { dx: -1, dy: 0 }),
            KeyCode::Right => Some(UiAction::MoveCalendar { dx: 1, dy: 0 }),
            KeyCode::Up => Some(UiAction::MoveCalendar { dx: 0, dy: -1 }),
            KeyCode::Down => Some(UiAction::MoveCalendar { dx: 0, dy: 1 }),
            KeyCode::Enter => Some(UiAction::ConfirmCalendar),
            KeyCode::Esc => Some(UiAction::CloseCalendar),
            _ => None,
        };
    }

    // 3. Help overlay — only handles its own keys
    if state.overlay == Overlay::Help {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') => Some(UiAction::CloseHelp),
            _ => None,
        };
    }

    // 4. Global Ctrl hotkeys (Ctrl-J handled later in Capture mode)
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('t') => return Some(UiAction::GoToday),
            KeyCode::Char('g') => return Some(UiAction::OpenCalendar),
            _ => {} // fall through — Ctrl-J is handled in Capture; others ignored per mode
        }
    }

    // 5. Esc handling (context-dependent)
    if key.code == KeyCode::Esc {
        return match state.focus {
            Focus::Capture => {
                if state.editing.is_some() {
                    Some(UiAction::CancelEdit)
                } else {
                    Some(UiAction::ExitCaptureMode)
                }
            }
            Focus::Navigate => Some(UiAction::ExitNavigateMode),
        };
    }

    // 6. [ and ] day navigation — only when can_navigate
    let can_navigate = matches!(state.focus, Focus::Navigate)
        || (matches!(state.focus, Focus::Capture) && state.input.is_empty());
    if can_navigate {
        match key.code {
            KeyCode::Char('[') => return Some(UiAction::PrevDay),
            KeyCode::Char(']') => return Some(UiAction::NextDay),
            _ => {}
        }
    }

    // 7. Mode-specific handling
    match state.focus {
        Focus::Capture => match key.code {
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
            _ => None,
        },
        Focus::Navigate => {
            // Ignore all Ctrl combos in navigate mode (Ctrl-C/T/G already handled above)
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return None;
            }
            if state.pending_delete {
                return match key.code {
                    KeyCode::Char('d') => Some(UiAction::ConfirmDelete),
                    _ => Some(UiAction::CancelDelete),
                };
            }
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => Some(UiAction::SelectNext),
                KeyCode::Char('k') | KeyCode::Up => Some(UiAction::SelectPrev),
                KeyCode::Char('g') => Some(UiAction::SelectFirst),
                KeyCode::Char('G') => Some(UiAction::SelectLast),
                KeyCode::Char(' ') | KeyCode::Char('x') => Some(UiAction::ToggleSelected),
                KeyCode::Char('e') => Some(UiAction::BeginEdit),
                KeyCode::Char('d') => Some(UiAction::InitiateDelete),
                KeyCode::Enter => Some(UiAction::ResumeHeading),
                KeyCode::Char('?') => Some(UiAction::OpenHelp),
                KeyCode::Char('i') => Some(UiAction::SwitchToCapture),
                _ => None,
            }
        }
    }
}
```

- [ ] **Step 4: Run the tests — expect all to pass**

```bash
cargo test key_to_action
```

Expected: all `key_to_action` tests pass.

- [ ] **Step 5: Run the full test suite**

```bash
cargo test
```

Expected: all tests pass (the `execute_action` stub panics only if called, which no existing test does).

- [ ] **Step 6: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: implement key_to_action pure mapping function with tests"
```

---

## Task 3: Implement `execute_action` with tests

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Add failing tests for `execute_action`**

Add the following tests inside the existing `mod tests` block in `src/app/input.rs` (after the `key_to_action` tests):

```rust
    // --- execute_action tests (simple state mutations only) ---

    #[test]
    fn quit_returns_quit_outcome() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        let outcome = execute_action(&mut state, UiAction::Quit).unwrap();
        assert_eq!(outcome, EventOutcome::Quit);
    }

    #[test]
    fn type_char_appends_to_input() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::TypeChar('a')).unwrap();
        assert_eq!(state.input, "a");
    }

    #[test]
    fn type_char_multiple_appends_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::TypeChar('h')).unwrap();
        execute_action(&mut state, UiAction::TypeChar('i')).unwrap();
        assert_eq!(state.input, "hi");
    }

    #[test]
    fn delete_char_pops_last_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "ab".to_string();
        execute_action(&mut state, UiAction::DeleteChar).unwrap();
        assert_eq!(state.input, "a");
    }

    #[test]
    fn delete_char_on_empty_input_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::DeleteChar).unwrap();
        assert_eq!(state.input, "");
    }

    #[test]
    fn type_newline_pushes_newline_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::TypeNewline).unwrap();
        assert_eq!(state.input, "\n");
    }

    #[test]
    fn initiate_delete_sets_pending_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        assert!(!state.pending_delete);
        execute_action(&mut state, UiAction::InitiateDelete).unwrap();
        assert!(state.pending_delete);
    }

    #[test]
    fn cancel_delete_clears_pending_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.pending_delete = true;
        execute_action(&mut state, UiAction::CancelDelete).unwrap();
        assert!(!state.pending_delete);
    }

    #[test]
    fn cancel_edit_clears_editing_and_input() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.editing = Some(0);
        state.input = "hello".to_string();
        execute_action(&mut state, UiAction::CancelEdit).unwrap();
        assert!(state.editing.is_none());
        assert!(state.input.is_empty());
    }

    #[test]
    fn exit_capture_mode_switches_focus_to_navigate() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        assert_eq!(state.focus, Focus::Capture); // default focus
        execute_action(&mut state, UiAction::ExitCaptureMode).unwrap();
        assert_eq!(state.focus, Focus::Navigate);
    }

    #[test]
    fn exit_navigate_mode_switches_focus_to_capture_and_clears_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        execute_action(&mut state, UiAction::ExitNavigateMode).unwrap();
        assert_eq!(state.focus, Focus::Capture);
        assert!(!state.pending_delete);
    }

    #[test]
    fn open_help_sets_overlay_and_clears_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        execute_action(&mut state, UiAction::OpenHelp).unwrap();
        assert_eq!(state.overlay, Overlay::Help);
        assert!(!state.pending_delete);
    }

    #[test]
    fn close_help_clears_overlay() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Help;
        execute_action(&mut state, UiAction::CloseHelp).unwrap();
        assert_eq!(state.overlay, Overlay::None);
    }

    #[test]
    fn switch_to_capture_sets_focus_and_clears_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        execute_action(&mut state, UiAction::SwitchToCapture).unwrap();
        assert_eq!(state.focus, Focus::Capture);
        assert!(!state.pending_delete);
    }

    #[test]
    fn open_calendar_sets_overlay_and_clears_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.pending_delete = true;
        execute_action(&mut state, UiAction::OpenCalendar).unwrap();
        assert_eq!(state.overlay, Overlay::Calendar);
        assert!(state.calendar.is_some());
        assert!(!state.pending_delete);
    }

    #[test]
    fn close_calendar_clears_overlay_and_calendar() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Calendar;
        state.calendar = Some(crate::ui::calendar::CalendarState::new(state.date));
        execute_action(&mut state, UiAction::CloseCalendar).unwrap();
        assert_eq!(state.overlay, Overlay::None);
        assert!(state.calendar.is_none());
    }
```

- [ ] **Step 2: Run the tests — expect failures**

```bash
cargo test execute_action 2>&1 | head -20
```

Expected: tests panic with `not yet implemented`.

- [ ] **Step 3: Implement `execute_action`**

Replace the `pub fn execute_action(_state: &mut AppState, _action: UiAction) -> Result<EventOutcome>` stub with:

```rust
pub fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::Quit => return Ok(EventOutcome::Quit),

        // Calendar overlay
        UiAction::MoveCalendar { dx, dy } => {
            if let Some(cal) = state.calendar.as_mut() {
                crate::ui::calendar::move_selection(cal, dx as i64, dy as i64);
            }
        }
        UiAction::ConfirmCalendar => {
            if let Some(cal) = state.calendar.take() {
                crate::app::actions::go_to_date(state, cal.selected)?;
                state.status.clear();
                state.overlay = Overlay::None;
            }
        }
        UiAction::CloseCalendar => {
            state.overlay = Overlay::None;
            state.calendar = None;
        }

        // Help overlay
        UiAction::CloseHelp => {
            state.overlay = Overlay::None;
        }

        // Global hotkeys
        UiAction::GoToday => {
            crate::app::actions::go_today(state)?;
            state.status.clear();
        }
        UiAction::OpenCalendar => {
            state.pending_delete = false;
            state.calendar = Some(crate::ui::calendar::CalendarState::new(state.date));
            state.overlay = Overlay::Calendar;
        }
        UiAction::PrevDay => {
            crate::app::actions::go_prev_day(state)?;
        }
        UiAction::NextDay => {
            crate::app::actions::go_next_day(state)?;
        }

        // Escape handling
        UiAction::CancelEdit => {
            state.editing = None;
            state.input.clear();
        }
        UiAction::ExitCaptureMode => {
            state.focus = Focus::Navigate;
        }
        UiAction::ExitNavigateMode => {
            state.pending_delete = false;
            state.focus = Focus::Capture;
        }

        // Capture mode
        UiAction::TypeChar(c) => {
            state.input.push(c);
        }
        UiAction::DeleteChar => {
            state.input.pop();
        }
        UiAction::TypeNewline => {
            state.input.push('\n');
        }
        UiAction::SubmitInput => {
            let cmd = crate::app::command::parse(&state.input);
            crate::app::actions::dispatch(state, cmd)?;
            if state.overlay != Overlay::None {
                state.pending_delete = false;
            }
            state.input.clear();
        }
        UiAction::CommitEdit => {
            crate::app::actions::commit_edit(state)?;
        }

        // Navigate mode
        UiAction::SelectNext => {
            crate::app::actions::select_next(state);
        }
        UiAction::SelectPrev => {
            crate::app::actions::select_prev(state);
        }
        UiAction::SelectFirst => {
            crate::app::actions::select_first(state);
        }
        UiAction::SelectLast => {
            crate::app::actions::select_last(state);
        }
        UiAction::ToggleSelected => {
            crate::app::actions::toggle_selected(state);
        }
        UiAction::BeginEdit => {
            crate::app::actions::begin_edit_selected(state);
        }
        UiAction::InitiateDelete => {
            state.pending_delete = true;
        }
        UiAction::ConfirmDelete => {
            if let Err(e) = crate::app::actions::delete_selected(state) {
                state.status = e.to_string();
            }
            state.pending_delete = false;
        }
        UiAction::CancelDelete => {
            state.pending_delete = false;
        }
        UiAction::ResumeHeading => {
            crate::app::actions::resume_selected_heading(state);
        }
        UiAction::OpenHelp => {
            state.pending_delete = false;
            state.overlay = Overlay::Help;
        }
        UiAction::SwitchToCapture => {
            state.pending_delete = false;
            state.focus = Focus::Capture;
        }
    }

    if state.should_quit {
        return Ok(EventOutcome::Quit);
    }
    Ok(EventOutcome::Continue)
}
```

- [ ] **Step 4: Run the new tests — expect all to pass**

```bash
cargo test execute_action
```

Expected: all `execute_action` tests pass.

- [ ] **Step 5: Run the full test suite**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: implement execute_action effectful handler with tests"
```

---

## Task 4: Refactor `main.rs` — thin `run()` and extract `parse_cli_args()`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace `src/main.rs` with the refactored version**

The full new content of `src/main.rs`:

```rust
use anyhow::{Context, Result};
use ratatui::crossterm::event::{Event, KeyEventKind};

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

    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;

    loop {
        terminal.draw(|frame| {
            buff::ui::render(frame, &app);
        })?;

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
cargo test
```

Expected: all tests pass.

- [ ] **Step 3: Build the release binary to confirm no compile errors**

```bash
cargo build
```

Expected: compiles cleanly, no warnings about unused imports in `main.rs`.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "refactor: replace monolithic run() with thin loop using key_to_action/execute_action"
```

---

## Verification

After all four tasks are complete, confirm the refactor is correct end-to-end:

- [ ] **Run the full test suite one final time**

```bash
cargo test
```

Expected: all tests pass, including all pre-existing tests in `actions.rs`, `command.rs`, `state.rs`, `parser.rs`, `day.rs`, `storage.rs`, `config.rs`, and all new tests in `input.rs`.

- [ ] **Confirm `main.rs` line count**

```bash
wc -l src/main.rs
```

Expected: ≤ 65 lines (down from 275).

---

## Behavioral Change Note

One intentional change from the original: **`CancelDelete` consumes the keypress.** In the original code, pressing (e.g.) `j` while `pending_delete` is true cancelled the delete AND moved the cursor. After this refactor, `CancelDelete` only clears `pending_delete` — the user presses `j` again to move. This is more predictable behavior and was approved during design review.
