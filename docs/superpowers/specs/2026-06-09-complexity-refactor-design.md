# Complexity Refactor Design

**Date:** 2026-06-09  
**Scope:** Issues 1–4 from complexity report (`complexity_20260609_0742.md`)  
**Files affected:** `src/app/input.rs`, `src/app/actions.rs`, `src/app/state.rs`

---

## Problem Statement

The complexity report identified four interrelated hotspots in the input/dispatch pipeline:

| Issue | Location | Complexity | Lines | Root cause |
|-------|----------|-----------|-------|------------|
| 1 | `src/app/input.rs` `execute_action` | 116 | 473 | Monolithic mode dispatch |
| 2 | `src/app/input.rs` `key_to_action` | 107 | 190 | Monolithic key mapping |
| 3 | `src/app/actions.rs` `dispatch` | 58 | 218 | Inline command handling |
| 4 | `src/app/state.rs` `context_at_line` | 18 | 68 | Unnamed phases, nesting depth 5 |

Issues 1–3 are in the same event-handling pipeline (`key_to_action` → `execute_action` → `dispatch`). Issue 4 is called throughout that pipeline. All four are addressed in a coordinated refactor.

---

## Decisions

### Keep the two-stage input pipeline

`key_to_action` (KeyEvent → UiAction) and `execute_action` (UiAction → state mutation) remain as separate stages. The `UiAction` enum is a valuable testable seam that enables:
- Independent verification of key bindings and state mutations
- Future configurable keybindings (remap key → UiAction without touching handlers)
- Future command palette / macro recording (dispatch UiAction directly)
- Multiple keys mapping to the same action without logic duplication

### Split input by mode into a module (Approach 2)

`input.rs` becomes `input/mod.rs` + one file per focus mode. Rejected alternatives:
- **In-place extraction (Approach 1):** Files remain large; no boundary prevents future accretion.
- **Trait-based dispatch (Approach 3):** Over-engineering at current scale; adds Rust trait object complexity.

### Thin-coordinator dispatch for commands (Option A)

`dispatch` becomes a ~20-line match calling named `handle_*` functions in the same file. Rejected alternative:
- **Domain-grouped sub-dispatchers (Option B):** Adds taxonomy decisions when adding new commands; current command count doesn't justify the organizational overhead. Natural groupings can be extracted later when they emerge.

### Named helpers for context detection (Option B)

`context_at_line` is refactored by extracting named helper functions that correspond to the three phases of the algorithm. Rejected alternative:
- **Early returns only (Option A):** Flattens nesting but doesn't add comprehensibility. Names document intent and become reusable vocabulary for future callers.

---

## Architecture

### Section 1: Input pipeline (Issues 1 & 2)

`src/app/input.rs` is renamed to `src/app/input/mod.rs`. Five mode files are added.

#### `src/app/input/mod.rs`

Owns the public API and thin coordinator functions. Nothing is added or removed from the public surface.

```
// Public types (unchanged)
pub enum UiAction { ... }
pub enum EventOutcome { Continue, Quit }

// Thin coordinator — handles universal concerns, delegates by focus mode
pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction>
  1. Ctrl-C → Quit (universal, checked first)
  2. Help overlay active → help_overlay key handling
  3. Global Ctrl hotkeys (Ctrl-T, Ctrl-L, etc.)
  4. Tab / BackTab (cross-mode, dispatch depends on state.focus)
  5. Esc (cross-mode, dispatch depends on state.focus + state.editing)
  6. Delegate by state.focus:
       Capture    → capture::key_to_action(state, key)
       VimNormal  → vim_normal::key_to_action(state, key)
       VimInsert  → vim_insert::key_to_action(state, key)
       RightPanel → right_panel::key_to_action(state, key)
       Chat       → chat::key_to_action(state, key)

// Thin coordinator — routes by action variant, delegates to mode handlers
pub fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome>
  1. Universal/focus-transition actions handled directly in coordinator:
       Quit, GoToday, PrevDay, NextDay, GoToDate,
       CloseHelp, OpenHelp,
       ToggleChat, FocusChat, ChatBlur,          // chat visibility + focus
       FocusVimNormal, SwitchToCapture,           // focus transitions
       ExitVimNormal, ExitCaptureMode, CancelEdit // mode exits / focus transitions
  2. Route by action variant to mode handler:
       TypeChar / DeleteChar / Submit* / Commit* /
       MoveCursor* / TypeNewline / TypeIndent / RemoveIndent /
       SelectNext / SelectPrev / SelectFirst / SelectLast /
       ToggleSelected / BeginEdit / ResumeHeading / PrependIndent
                                    → capture::execute_action(state, action)
       VimMove* / VimDelete* / VimYank* / VimPaste* /
       VimUndo / VimToggleTodo / VimEnterInsert* /
       VimSetPendingOp / VimClearPendingOp / VimDeleteChar /
       VimInsertLineBelow / VimInsertLineAbove / VimMoveWord*
                                    → vim_normal::execute_action(state, action)
       VimInsertChar / VimInsertNewline / VimInsertBackspace /
       VimInsertDeleteWordBefore / VimExitInsert
                                    → vim_insert::execute_action(state, action)
       RightPanelUp / RightPanelDown / RightPanelToggle / RightPanelBlur
                                    → right_panel::execute_action(state, action)
       ChatScrollUp / ChatScrollDown
                                    → chat::execute_action(state, action)
```

Routing in `execute_action` works by action variant name without inspecting `state.focus` — the `UiAction` naming convention already encodes mode.

#### Mode files

Each mode file exports two `pub(super)` functions only visible within the `input` module:

| File | Est. lines | `key_to_action` handles | `execute_action` handles |
|------|-----------|------------------------|--------------------------|
| `capture.rs` | ~90 | char input, backspace, arrow keys, Ctrl-J | TypeChar, DeleteChar, TypeNewline, TypeIndent, RemoveIndent, SubmitInput, CommitEdit, CancelEdit, ExitCaptureMode, MoveCursor*, SelectNext/Prev/First/Last, ToggleSelected, BeginEdit, ResumeHeading |
| `vim_normal.rs` | ~130 | h/j/k/l, arrow keys, w/b/e, gg/G, i/I/a/A/o/O, dd/yy/p, u, x, ~, pending ops | VimMove*, VimEnterInsert*, VimDeleteLine, VimYankLine, VimPasteBelow/Above, VimUndo, VimToggleTodo, VimSetPendingOp, VimClearPendingOp, VimDeleteChar |
| `vim_insert.rs` | ~60 | printable chars, backspace, Ctrl-W, arrows, Esc | VimInsertChar, VimInsertNewline, VimInsertBackspace, VimInsertDeleteWordBefore, VimExitInsert |
| `right_panel.rs` | ~40 | j/k, arrow keys, space/enter, Esc | RightPanelUp, RightPanelDown, RightPanelToggle, RightPanelBlur |
| `chat.rs` | ~40 | j/k, arrow keys, Esc, Ctrl-L | ChatScrollUp, ChatScrollDown, ChatBlur |

**Test impact:** None. All existing tests use the public `key_to_action` and `execute_action` signatures and test through `UiAction` variants. No test changes required.

---

### Section 2: Command dispatch (Issue 3)

`src/app/actions.rs` keeps its location. The change is internal only.

#### Current structure
```
pub fn dispatch(state, cmd) — 218-line match with logic inline in each arm
```

#### Refactored structure
```
pub fn dispatch(state, cmd) — ~20-line match, each arm calls a named fn

Private handler functions (in the same file):
  fn handle_entry(state, text)
  fn handle_meeting(state, name)
  fn handle_note(state, name)
  fn handle_todo(state, text)
  fn handle_leave(state)
  fn handle_goto(state, date)       ← delegates to existing go_to_date()
  fn handle_today(state)            ← delegates to existing go_today()
  fn handle_summarize(state)
  fn handle_ask(state, text)
  fn handle_clear(state)
  fn handle_start(state)
  fn handle_end(state)
  fn handle_scheduled(state, time)
  fn handle_section(state, name)
  fn handle_resume(state, target)
  fn handle_unknown(state, input)
```

The existing public functions (`go_to_date`, `go_today`, `go_prev_day`, `go_next_day`, `vim_update_context`, `after_vim_edit`, `vim_jump_to_new_content`) are unchanged. `handle_goto` and `handle_today` delegate to them, preserving all existing callers in `input.rs`.

**Adding a new command (post-refactor):**
1. Add variant to `Command` enum in `command.rs`
2. Add parse rule in `command.rs`
3. Write `fn handle_mycommand(state, ...)` in `actions.rs`
4. Add one arm to `dispatch` match

No other files touched.

---

### Section 3: Context detection (Issue 4)

`context_at_line` moves from `src/app/state.rs` to a new `src/app/context.rs`. The `Context` enum stays in `state.rs` (it is structurally part of `AppState`). `context.rs` imports `Context` from `state.rs`. Callers update their `use` path from `crate::app::state::context_at_line` to `crate::app::context::context_at_line`.

#### Named helper functions

```rust
/// Backward scan: index of the nearest "## " heading at or before cursor_line.
fn enclosing_l2_heading(lines: &[String], cursor_line: usize) -> Option<usize>

/// Forward scan: index of the last "### " heading in lines[start..=end].
/// Stops at any "## " heading encountered (crossed section boundary).
fn last_l3_heading(lines: &[String], start: usize, end: usize) -> Option<usize>

/// Forward scan: index and level of the last "####"+ heading in lines[l3_line..=end]
/// that follows the given l3 heading. Returns None if none found.
fn last_l4plus_heading(lines: &[String], l3_line: usize, end: usize) -> Option<(usize, u8)>

/// Count of "### " headings in lines[start..=end].
/// Used to compute Meeting/NoteBlock ordinal.
fn count_l3_headings(lines: &[String], start: usize, end: usize) -> usize
```

`last_l4plus_heading` reuses `heading_level()` from `src/model/parser.rs` instead of re-implementing the `chars().take_while(|c| c == '#').count()` pattern. This eliminates an existing subtle duplication.

#### Refactored `context_at_line`

```
pub fn context_at_line(lines, cursor_line) -> Context:
  guard: empty or out-of-bounds → Notes
  boundary = enclosing_l2_heading(...)  or → Notes
  "## To-dos"                           → Todos
  not "## Meetings" or "## Notes"       → Notes
  l3 = last_l3_heading(...)             or → Notes
  (l4, level) = last_l4plus_heading(...)  → Section { heading_line: l4, level }
  ordinal = count_l3_headings(...) - 1
  in_meetings                             → Meeting(ordinal)
  else                                    → NoteBlock(ordinal)
```

Reduces from 68 lines / complexity 18 / nesting 5 to approximately 25 lines / complexity ~6 / nesting 2.

**Test impact:** The seven `cursor_*` tests in `state.rs` move to `context.rs`. Their assertions are unchanged — same public function, same behaviour.

---

## File Change Summary

| File | Change |
|------|--------|
| `src/app/input.rs` | Renamed → `src/app/input/mod.rs` |
| `src/app/input/mod.rs` | Add `mod capture; mod vim_normal; mod vim_insert; mod right_panel; mod chat;` |
| `src/app/input/capture.rs` | New |
| `src/app/input/vim_normal.rs` | New |
| `src/app/input/vim_insert.rs` | New |
| `src/app/input/right_panel.rs` | New |
| `src/app/input/chat.rs` | New |
| `src/app/actions.rs` | Internal refactor only (dispatch thinned, handlers extracted) |
| `src/app/context.rs` | New (contains `context_at_line` + helpers, moved from `state.rs`) |
| `src/app/state.rs` | Remove `context_at_line` + its tests; `Context` enum stays |
| `src/app/mod.rs` | Add `pub mod context;` |

---

## Out of Scope

- Issues 5 (`parse_color` in `theme.rs`) and 6 (test dedup + `writer.rs` parse failure) are independent one-offs handled separately.
- No behaviour changes. This is a pure structural refactor. All existing tests must pass without modification (except moving the `cursor_*` tests to `context.rs`).
- No new features.
- No changes to `UiAction` variants, `Command` variants, `Context` variants, or any public API surface.
