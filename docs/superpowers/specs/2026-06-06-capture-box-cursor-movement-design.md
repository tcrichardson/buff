# Capture Box Cursor Movement — Design

**Date:** 2026-06-06  
**Status:** Approved

## Summary

Add free cursor movement within the capture box. Currently `app.input` is a flat `String` with the cursor permanently pinned to the end. This design adds a `cursor_pos` byte offset so the user can move within input text, fix typos mid-line, and jump to the start/end of the current line.

## Scope

- Left/right arrow key movement within the input string
- Backspace deletes the character to the left of the cursor (not always the tail)
- Typing inserts at the cursor, not always at the end
- `Ctrl+A` / `Home` — jump to start of current line
- `Ctrl+E` / `End` — jump to end of current line
- No up/down line navigation (most entries are single-line; multi-line is uncommon)
- No forward-delete (`Delete` key) — not requested

## Out of Scope

- Up/down cursor movement between lines
- Word-jump (`Ctrl+Left`/`Ctrl+Right`)
- Selection / clipboard
- Undo/redo
- Scroll-to-cursor (if cursor is above the scroll viewport it will not be visible — acceptable for now)

---

## Section 1: State

**File:** `src/app/state.rs`

Add one field to `AppState`:

```rust
pub cursor_pos: usize,  // byte offset into `input`; always <= input.len(), always on a UTF-8 char boundary
```

Initialize to `0` in `AppState::open_day`.

### Invariant

`cursor_pos` must always satisfy:
- `cursor_pos <= input.len()`
- `input.is_char_boundary(cursor_pos)`

Every mutation must maintain this invariant.

### Reset sites

Any code that clears or replaces `input` must also update `cursor_pos`:

| Location | Action |
|---|---|
| `execute_action`: `SubmitInput` | reset to `0` after `input.clear()` |
| `execute_action`: `CancelEdit` | reset to `0` after `input.clear()` |
| `actions::commit_edit` | reset to `0` after `input.clear()` |
| `actions::begin_edit_selected` | set to `input.len()` (cursor starts at end of loaded text) |
| `actions::go_to_date` | naturally `0` — replaces the whole `AppState` via `open_day` |

---

## Section 2: Key Bindings and UiAction Variants

**File:** `src/app/input.rs`

### New UiAction variants

```rust
MoveCursorLeft,
MoveCursorRight,
MoveCursorLineStart,   // Ctrl+A or Home
MoveCursorLineEnd,     // Ctrl+E or End
```

### Key mapping (added to `Focus::Capture` branch of `key_to_action`)

| Key | Action |
|---|---|
| `←` (Left) | `MoveCursorLeft` |
| `→` (Right) | `MoveCursorRight` |
| `Home` | `MoveCursorLineStart` |
| `End` | `MoveCursorLineEnd` |
| `Ctrl+A` | `MoveCursorLineStart` |
| `Ctrl+E` | `MoveCursorLineEnd` |

No existing bindings conflict. `Ctrl+A`/`Ctrl+E` follow readline/Emacs convention.
`Ctrl+A` and `Ctrl+E` are added as guard-matched arms alongside the existing `Ctrl+J` arm.
`Left`/`Right` currently fall through to `_ => None` in Capture mode.

These bindings are **only active in `Focus::Capture`**. Navigate mode already ignores all Ctrl combos and all unrecognised keys.

---

## Section 3: Mutation Logic

**File:** `src/app/input.rs` (`execute_action`) and optionally a helper module

### UTF-8 boundary helpers

Two private helper functions in `src/app/input.rs`:

```rust
/// Step back one Unicode scalar value from `pos`. Returns 0 if already at start.
fn prev_char_boundary(s: &str, pos: usize) -> usize;

/// Step forward one Unicode scalar value from `pos`. Returns s.len() if already at end.
fn next_char_boundary(s: &str, pos: usize) -> usize;
```

### Mutation table

| Action | Behaviour |
|---|---|
| `TypeChar(c)` | `input.insert(cursor_pos, c)` then `cursor_pos += c.len_utf8()` |
| `DeleteChar` (Backspace) | `prev = prev_char_boundary(input, cursor_pos)`; `input.remove(prev)`; `cursor_pos = prev` |
| `TypeNewline` | `input.insert(cursor_pos, '\n')` then `cursor_pos += 1` |
| `MoveCursorLeft` | `cursor_pos = prev_char_boundary(input, cursor_pos)` |
| `MoveCursorRight` | `cursor_pos = next_char_boundary(input, cursor_pos)` |
| `MoveCursorLineStart` | scan left from `cursor_pos` for `\n`; set to position immediately after (or `0`) |
| `MoveCursorLineEnd` | scan right from `cursor_pos` for `\n`; set to that position (or `input.len()`) |

`DeleteChar` when `cursor_pos == 0` is a no-op (no char to the left).

---

## Section 4: Cursor Rendering

**File:** `src/ui/capture.rs` — `render_input`

Replace the current "cursor always at end of last line" logic with a `cursor_pos`-aware calculation:

1. Split `input` on `'\n'` to get lines.
2. Walk lines, consuming bytes (plus 1 for each `\n` separator) until the accumulated byte count reaches `cursor_pos`. The current line index is `cursor_row`; the remaining byte count within that line is the byte column.
3. Convert the byte column to a **character count** (UTF-8 safe) for correct terminal display width.
4. Add the prefix width to `cursor_col` when `cursor_row == 0`:
   - Normal mode prefix `"› "` → 2 chars
   - Edit mode prefix `"Edit: › "` → 8 chars
5. Apply the existing `overflow` scroll offset when computing visual row: `visual_row = cursor_row.saturating_sub(overflow)`.

The existing scroll logic (scroll to keep the last line visible, computed from total line count) is unchanged.

### Known limitation

If the cursor is on a line that has scrolled above the viewport, it will not be visible. This is acceptable for now since the overflow case arises only in multi-line entries, and single-line is by far the common case.

---

## Files Changed

| File | Change |
|---|---|
| `src/app/state.rs` | Add `cursor_pos: usize` field, initialize to `0` |
| `src/app/input.rs` | Add `UiAction` variants; add key mappings; add UTF-8 helpers; update all `execute_action` mutation arms; reset `cursor_pos` on clear |
| `src/app/actions.rs` | `begin_edit_selected` sets `cursor_pos = input.len()`; `commit_edit` resets `cursor_pos = 0` |
| `src/ui/capture.rs` | Replace end-of-last-line cursor calculation with `cursor_pos`-aware row/col logic |

---

## Testing

Extend existing `#[cfg(test)]` block in `src/app/input.rs`:

- `type_char_inserts_at_cursor_pos` — cursor mid-string, TypeChar inserts correctly
- `delete_char_removes_before_cursor` — cursor mid-string, Backspace removes char to left
- `delete_char_at_start_is_noop` — cursor at 0, no panic
- `move_cursor_left_decrements` / `move_cursor_right_increments`
- `move_cursor_left_clamps_at_zero` / `move_cursor_right_clamps_at_end`
- `move_line_start_finds_newline` / `move_line_end_finds_newline`
- `move_line_start_at_bol_is_noop` — already at start of line
- `cursor_pos_reset_on_submit` — `cursor_pos` is `0` after `SubmitInput`
- `cursor_pos_set_to_end_on_begin_edit`
- UTF-8 multibyte character boundary tests for left/right movement
