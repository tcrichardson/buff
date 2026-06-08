# Design: Vim Notes Editor

**Date:** 2026-06-08  
**Status:** Approved  
**Scope:** Replace Navigate mode in the notes panel with a vim-style modal editor (normal + insert modes), keeping the capture bar for structured slash-command entry.

---

## Overview

The notes panel currently supports a coarse-grained Navigate mode: `j`/`k` jump between selectable blocks (meetings, todos, bullets), and pressing Enter opens a selected item for editing in the capture bar. This is replaced by a proper vim modal editor where the cursor moves freely through the document line by line, and `i`/`a`/`o` enter insert mode for direct text editing.

The capture bar is unchanged — it remains the primary surface for slash commands (`/meeting`, `/todo`, `/scheduled`, etc.). Its context is now derived automatically from the vim cursor position rather than being set explicitly.

---

## Focus Model

### Current

```rust
pub enum Focus {
    Capture,
    Navigate,
    Chat,
}
```

### New

```rust
pub enum Focus {
    Capture,
    VimNormal,  // replaces Navigate
    VimInsert,  // new
    Chat,
}
```

`Focus::Navigate` is removed entirely. All navigation in the notes panel goes through `VimNormal`.

### State Transitions

| From | Key | To | Notes |
|---|---|---|---|
| `VimNormal` | `i` / `a` / `A` / `o` / `O` | `VimInsert` | undo snapshot taken on entry |
| `VimInsert` | `Esc` | `VimNormal` | cursor moves left one col (vim convention) |
| `VimNormal` | `Tab` | `Capture` | context already derived from cursor |
| `Capture` | `Esc` | `VimNormal` | returns to notes panel |
| `Capture` | `Enter` | `VimNormal` | submits entry; cursor jumps to new content |
| `VimNormal` | `u` | `VimNormal` | restores last insert-session snapshot |

---

## VimState

A `VimState` struct is added to `AppState`:

```rust
pub struct VimState {
    pub cursor_line: usize,        // 0-indexed line in doc.lines
    pub cursor_col: usize,         // 0-indexed char offset in raw line text
    pub pending_op: Option<char>,  // for multi-key sequences: 'd' (dd), 'g' (gg), 'y' (yy)
    pub yank_buffer: Vec<String>,  // lines yanked via yy or deleted via dd
    pub undo_stack: Vec<UndoEntry>,
}

pub struct UndoEntry {
    pub lines: Vec<String>,   // full doc snapshot before insert session began
    pub cursor_line: usize,
    pub cursor_col: usize,
}
```

`state.selected: Option<usize>` (selectable index) is removed. `vim.cursor_line` serves as the position reference for all note-panel interactions.

---

## Key Bindings

### VimNormal

| Key(s) | Action |
|---|---|
| `h` / `←` | cursor left |
| `l` / `→` | cursor right |
| `j` / `↓` | cursor down |
| `k` / `↑` | cursor up |
| `w` | forward word |
| `b` | backward word |
| `e` | end of word |
| `0` | line start |
| `$` | line end |
| `gg` | first line (pending op `g`) |
| `G` | last line |
| `i` | enter insert before cursor |
| `a` | enter insert after cursor |
| `A` | enter insert at end of line |
| `o` | insert new line below, enter insert |
| `O` | insert new line above, enter insert |
| `x` | delete char at cursor |
| `dd` | delete current line → yank buffer (pending op `d`) |
| `yy` | yank current line → yank buffer (pending op `y`) |
| `p` | paste yank buffer below cursor line |
| `P` | paste yank buffer above cursor line |
| `u` | undo last insert session |
| `t` | toggle todo if cursor line is `- [ ]` or `- [x]`; no-op otherwise |
| `Tab` | switch focus to capture bar |
| `Esc` | no-op |

### VimInsert

| Key(s) | Action |
|---|---|
| printable chars | insert at cursor position |
| `Enter` | insert newline, cursor to start of new line |
| `Backspace` | delete char before cursor |
| `←` / `→` / `↑` / `↓` | cursor movement |
| `Ctrl-W` | delete word before cursor |
| `Esc` | exit to VimNormal; cursor moves left one col |

**Out of scope for v1:** count prefixes (`3dd`, `2w`), visual mode, `/` search, `:` command line, `r` replace-char, `.` repeat.

---

## Rendering

### Cursor Line: Raw; All Other Lines: Formatted

The notes panel keeps its current markdown rendering (bold headings, `☐`/`☑` todos, `•` bullets, etc.) for all lines **except** the cursor line, which renders as raw markdown text. This ensures the cursor column in display space matches the cursor column in raw text space exactly — no offset arithmetic needed for prefixes like `### ` or `- [ ] `.

When the cursor moves off a line, it re-renders formatted. This gives a "mostly pretty" view with a raw editing window at the cursor.

### Cursor Appearance

- **VimNormal**: block highlight (inverted colours) on the character at `(cursor_line, cursor_col)`. Cursor line background is subtly tinted.
- **VimInsert**: beam/bar cursor at `(cursor_line, cursor_col)`. Cursor line background tinted.

Terminal cursor is positioned via `frame.set_cursor_position()` in `ui/document.rs`, consistent with how the capture bar already does it.

### Mode Line

A thin status bar is added at the bottom of the notes panel box:

```
-- NORMAL --                                              ln 12/34
-- INSERT --                                              ln 12/34
```

Left side shows current mode; right side shows `ln current/total`.

### Context in Capture Bar Status

The capture bar status line already shows `context: <name>`. This continues to work — `state.context` is updated on every cursor movement in VimNormal mode via `context_at_line()`.

---

## Context Derivation

`context_at_line(lines: &[String], cursor_line: usize) -> Context` is added to `src/app/state.rs` (or `src/app/command.rs`).

### Algorithm

1. Walk **backward** from `cursor_line` until a `## ` boundary line is found. This identifies the top-level section (`## Meetings`, `## Notes`, `## To-dos`).
2. Walk **forward** from that boundary line to `cursor_line`, tracking:
   - The most recent `### ` heading (meeting or note heading)
   - The most recent `#### ` or deeper heading (section)
3. Return the most specific context found:

| Cursor location | Context |
|---|---|
| Inside `## Meetings`, no `###` yet | `Context::Meetings` |
| Inside a `### MeetingName` block | `Context::Meeting(ordinal)` |
| Inside a `#### SubSection` under a meeting | `Context::Section { heading_line, level }` |
| Inside `## Notes`, no `###` yet | `Context::Notes` |
| Inside a `### NoteName` block | `Context::NoteBlock(ordinal)` |
| Inside a `#### SubSection` under a note | `Context::Section { heading_line, level }` |
| Inside `## To-dos` | `Context::Todos` (capture bar shows hint only) |

Ordinals are computed by counting `### ` headings within the section from the section boundary up to and including the found heading (0-indexed).

`Context::Todos` is a new variant added to the `Context` enum to represent cursor inside `## To-dos`. It carries no data. The capture bar shows a hint ("use /todo to add todos") but no structured slash commands act on this context.

`context_at_line()` is called on every cursor move in VimNormal mode and on entry to VimNormal from VimInsert or Capture.

---

## Undo

### Model

Undo is snapshot-per-insert-session:

- On entry to `VimInsert`, push `UndoEntry { lines: doc.lines.clone(), cursor_line, cursor_col }` onto `vim.undo_stack`.
- `u` in `VimNormal` pops the top entry and restores `doc.lines`, `cursor_line`, `cursor_col`.
- If `undo_stack` is empty, `u` is a no-op.

This matches stock vim's default behaviour: one `u` undoes everything typed in the last insert session. Simple, predictable, no diff machinery required.

`dd` and `p`/`P` do **not** push to `undo_stack` in v1 — they are their own inverse via the yank buffer.

---

## Todo Toggling

In Navigate mode, todos were toggled by pressing `Enter` on a selected todo. In vim mode:

- **`t`** in `VimNormal`: if `doc.lines[vim.cursor_line]` begins with `- [ ]` or `- [x]`/`- [X]`, call the existing `doc.toggle_todo_at_line()` (or equivalent). Otherwise no-op.

This requires a small addition to `model/writer.rs`: a `toggle_todo_at_line(line_idx: usize)` method that operates by line index rather than selectable index.

---

## Files Changed

| File | Change |
|---|---|
| `src/app/state.rs` | Add `VimState`, `UndoEntry`; replace `Focus::Navigate` with `VimNormal`/`VimInsert`; remove `state.selected`; add `Context::Todos` variant; add `context_at_line()`; initialise `vim` to `VimState { cursor_line: 0, cursor_col: 0, .. }` on app/doc load |
| `src/app/input.rs` | Remove Navigate key handling; add VimNormal and VimInsert key handlers; arrow keys in both vim modes |
| `src/app/actions.rs` | Remove Navigate actions; add vim cursor/edit actions; wire `context_at_line()` on cursor moves; after a successful capture-bar `Enter` submit, update `vim.cursor_line` to the line index of the newly inserted content so the cursor follows the new entry |
| `src/app/command.rs` | No changes required |
| `src/ui/document.rs` | Cursor-line raw rendering; block/beam cursor rendering; mode line at panel bottom; call `frame.set_cursor_position()` |
| `src/ui/layout.rs` | Reserve one extra line at bottom of notes panel for mode line |
| `src/ui/capture.rs` | No changes — context display already works from `state.context` |
| `src/model/writer.rs` | Add `toggle_todo_at_line(line_idx)` |
| `src/model/day.rs` | No structural changes — `Document` as `Vec<String>` already supports direct line manipulation |

`doc.selectables()` is **retained** — still used by the right-panel todo collector (`ui/right_panel.rs`).

---

## Out of Scope

- Vim count prefixes (`3dd`, `2w`, `5j`)
- Visual mode (`v`, `V`)
- Search (`/pattern`, `n`/`N`)
- Command line (`:`)
- Replace char (`r`)
- Repeat last command (`.`)
- Redo (`Ctrl-R`)
- Marks and jumps (`` ` ``, `'`)

These can be added incrementally in follow-up work.
