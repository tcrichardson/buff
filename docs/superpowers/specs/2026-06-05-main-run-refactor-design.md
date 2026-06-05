# Design: Refactor `main.rs::run` — Two-Stage Key Dispatch

**Date:** 2026-06-05  
**Status:** Approved  
**Context:** `src/main.rs::run` has cyclomatic complexity 81, 247 lines, nesting depth 7 — the dominant complexity hotspot in the codebase.

---

## Goals

- Reduce `run()` to a thin orchestrator (~30 lines)
- Enable unit testing of key-handling logic without a live terminal
- Make adding new keybindings a localized, safe change
- Both testability and maintainability are first-class requirements

---

## Approach: Two-Stage Key Dispatch

All key-handling logic moves to `src/app/input.rs`. The design has two stages:

1. **Pure mapping** — `key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction>`  
   Reads state, returns a semantic action or nothing. Zero side effects. Trivially unit-testable.

2. **Effectful execution** — `execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome>`  
   Mutates state. Delegates complex operations to `actions.rs`. Testable at the state-mutation level.

---

## Module Structure

### New: `src/app/input.rs`

Contains:
- `EventOutcome` enum
- `UiAction` enum
- `key_to_action` function (pure)
- `execute_action` function (effectful)
- Unit tests for both functions

### Shrunk: `src/main.rs`

Contains only:
- `main()` — unchanged
- `TerminalGuard` + `Drop` impl — unchanged
- `read_key()` — unchanged
- `parse_cli_args()` — extracted from `run()` (new helper)
- `run()` — thin loop, ~30 lines

### Unchanged: `src/app/actions.rs`

`execute_action` delegates complex mutations here. No changes needed.

### Modified: `src/app/mod.rs`

Gains `pub mod input;`.

---

## `EventOutcome`

```rust
#[derive(PartialEq, Eq)]
pub enum EventOutcome {
    Continue,
    Quit,
}
```

---

## `UiAction` Enum

```rust
pub enum UiAction {
    // Universal
    Quit,                            // Ctrl-C

    // Calendar overlay
    MoveCalendar { dx: i8, dy: i8 }, // arrow keys in calendar
    ConfirmCalendar,                 // Enter in calendar
    CloseCalendar,                   // Esc in calendar

    // Help overlay
    CloseHelp,                       // Esc or '?' in help

    // Global hotkeys (work in any non-overlay state)
    GoToday,                         // Ctrl-T
    OpenCalendar,                    // Ctrl-G
    PrevDay,                         // '[' when can_navigate
    NextDay,                         // ']' when can_navigate

    // Escape handling (context-dependent)
    CancelEdit,                      // Esc in Capture mode while editing
    ExitCaptureMode,                 // Esc in Capture mode, not editing
    ExitNavigateMode,                // Esc in Navigate mode

    // Capture mode
    TypeChar(char),                  // printable char, no Ctrl
    DeleteChar,                      // Backspace
    TypeNewline,                     // Ctrl-J
    SubmitInput,                     // Enter (not editing)
    CommitEdit,                      // Enter while editing

    // Navigate mode
    SelectNext,                      // 'j' or Down
    SelectPrev,                      // 'k' or Up
    SelectFirst,                     // 'g'
    SelectLast,                      // 'G'
    ToggleSelected,                  // Space or 'x'
    BeginEdit,                       // 'e'
    InitiateDelete,                  // 'd' (sets pending_delete)
    ConfirmDelete,                   // 'd' while pending_delete=true
    CancelDelete,                    // any other key while pending_delete=true (key consumed)
    ResumeHeading,                   // Enter
    OpenHelp,                        // '?'
    SwitchToCapture,                 // 'i'
}
```

---

## `key_to_action` — Pure Mapping Function

**Signature:** `pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction>`

**Priority order (first match wins):**

1. Ctrl-C → `Some(Quit)`
2. If `overlay == Calendar`:
   - Left → `MoveCalendar { dx: -1, dy: 0 }`
   - Right → `MoveCalendar { dx: 1, dy: 0 }`
   - Up → `MoveCalendar { dx: 0, dy: -1 }`
   - Down → `MoveCalendar { dx: 0, dy: 1 }`
   - Enter → `ConfirmCalendar`
   - Esc → `CloseCalendar`
   - _ → `None`
3. If `overlay == Help`:
   - Esc or `?` → `CloseHelp`
   - _ → `None`
4. Ctrl-T → `GoToday`
5. Ctrl-G → `OpenCalendar`
6. Esc → depends on `focus` and `editing`:
   - Capture + `editing.is_some()` → `CancelEdit`
   - Capture + not editing → `ExitCaptureMode`
   - Navigate → `ExitNavigateMode`
7. `[` or `]` when `can_navigate`:
   - `can_navigate` = `focus == Navigate || (focus == Capture && input.is_empty())`
   - `[` → `PrevDay`; `]` → `NextDay`
8. If `focus == Capture`:
   - Enter + `editing.is_some()` → `CommitEdit`
   - Enter + not editing → `SubmitInput`
   - Backspace → `DeleteChar`
   - Ctrl-J → `TypeNewline`
   - Printable char (no Ctrl modifier, not control char) → `TypeChar(c)`
   - _ → `None`
9. If `focus == Navigate`:
   - Ctrl key held → `None` (ignored)
   - `pending_delete && key == 'd'` → `ConfirmDelete`
   - `pending_delete && key != 'd'` → `CancelDelete`
   - `j` or Down → `SelectNext`
   - `k` or Up → `SelectPrev`
   - `g` → `SelectFirst`
   - `G` → `SelectLast`
   - Space or `x` → `ToggleSelected`
   - `e` → `BeginEdit`
   - `d` → `InitiateDelete`
   - Enter → `ResumeHeading`
   - `?` → `OpenHelp`
   - `i` → `SwitchToCapture`
   - _ → `None`

---

## `execute_action` — Effectful Execution

**Signature:** `pub fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome>`

**Behavioral notes:**

- **Simple direct mutations** (no delegation needed):
  - `TypeChar(c)` → `state.input.push(c)`
  - `DeleteChar` → `state.input.pop()`
  - `TypeNewline` → `state.input.push('\n')`
  - `InitiateDelete` → `state.pending_delete = true`
  - `CancelDelete` → `state.pending_delete = false`
  - `OpenCalendar` → set `state.calendar` and `state.overlay = Overlay::Calendar`, clear `pending_delete`
  - `CloseCalendar` → `state.overlay = Overlay::None; state.calendar = None`
  - `CloseHelp` → `state.overlay = Overlay::None`
  - `OpenHelp` → `state.pending_delete = false; state.overlay = Overlay::Help`
  - `SwitchToCapture` → `state.pending_delete = false; state.focus = Focus::Capture`
  - `ExitCaptureMode` → `state.focus = Focus::Navigate`
  - `ExitNavigateMode` → `state.pending_delete = false; state.focus = Focus::Capture`
  - `CancelEdit` → `state.editing = None; state.input.clear()`
  - `MoveCalendar { dx, dy }` → delegate to `ui::calendar::move_selection`

- **Delegated to `actions.rs`**:
  - `GoToday` → `actions::go_today(state)?; state.status.clear()`
  - `PrevDay` → `actions::go_prev_day(state)?`
  - `NextDay` → `actions::go_next_day(state)?`
  - `ConfirmCalendar` → take `state.calendar` (sets it to `None`), call `actions::go_to_date(state, cal.selected)?`, then `state.status.clear(); state.overlay = Overlay::None`
  - `CommitEdit` → `actions::commit_edit(state)?`
  - `SubmitInput` → `command::parse(&state.input)` then `actions::dispatch(state, cmd)?; state.input.clear()`; after dispatch, if `state.overlay != Overlay::None` then `state.pending_delete = false`
  - `SelectNext/Prev/First/Last` → `actions::select_*`
  - `ToggleSelected` → `actions::toggle_selected`
  - `BeginEdit` → `actions::begin_edit_selected`
  - `ConfirmDelete` → `actions::delete_selected(state)` (error → `state.status`); `state.pending_delete = false`
  - `ResumeHeading` → `actions::resume_selected_heading`

- **Returns:**
  - `Quit` → returns `Ok(EventOutcome::Quit)` immediately
  - After all other actions: if `state.should_quit`, return `Ok(EventOutcome::Quit)`, else `Ok(EventOutcome::Continue)`

---

## New `parse_cli_args`

```rust
struct CliArgs {
    notes_dir: Option<String>,
}

fn parse_cli_args() -> Result<CliArgs>
```

Handles `--notes-dir <value>`, `--help` (prints usage and exits), `--version` (prints version and exits), and unknown flags (returns error). Mirrors the logic currently in `run()`.

---

## New `run()`

```rust
fn run() -> Result<()> {
    let cli = parse_cli_args()?;
    let (config, notes_dir) = buff::config::load(cli.notes_dir).context("Config error")?;
    let mut app = buff::app::state::AppState::open_day(
        notes_dir, config, chrono::Local::now().date_naive()
    ).context("Failed to open day")?;

    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;

    loop {
        terminal.draw(|frame| buff::ui::render(frame, &app))?;
        if let Some(key) = read_key()? {
            if let Some(action) = buff::app::input::key_to_action(&app, key) {
                if buff::app::input::execute_action(&mut app, action)?
                    == EventOutcome::Quit
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

## Testing Strategy

### `key_to_action` tests (in `src/app/input.rs`)

Tests construct minimal `AppState` via `AppState::open_day` or a test builder, set the relevant fields (`focus`, `overlay`, `pending_delete`, `editing`, `input`), and assert on the returned `Option<UiAction>`. No terminal, no I/O.

Example cases:
- Navigate mode + `j` key → `SelectNext`
- Navigate mode + `pending_delete=true` + `j` key → `CancelDelete`
- Navigate mode + `pending_delete=true` + `d` key → `ConfirmDelete`
- Capture mode + non-empty input + `[` key → `TypeChar('[')` (not PrevDay)
- Capture mode + empty input + `[` key → `PrevDay`
- Calendar overlay + Left → `MoveCalendar { dx: -1, dy: 0 }`
- Calendar overlay + `j` → `None`

### `execute_action` tests (in `src/app/input.rs`)

Tests for simple direct-mutation actions only (complex delegations are covered by `actions.rs` tests):
- `TypeChar('a')` → `state.input == "a"`
- `DeleteChar` on non-empty input → pops last char
- `CancelDelete` → `pending_delete = false`
- `InitiateDelete` → `pending_delete = true`
- `CancelEdit` → `editing = None, input = ""`

---

## Behavioral Changes

One intentional behavior change from the current implementation:

**`CancelDelete` consumes the keypress.** Currently, pressing (e.g.) `j` while `pending_delete` is true cancels the delete AND moves the cursor. After this refactor, it cancels the delete only, and the user must press `j` again. This is more predictable (vim-style) and simplifies the dispatch model.

---

## Out of Scope

- Changes to `src/app/actions.rs` (no structural changes, only delegation calls added)
- Changes to `src/ui/` modules
- The `post_mutation_sync` helper for `actions.rs` (separate improvement)
- Any changes to the model layer
