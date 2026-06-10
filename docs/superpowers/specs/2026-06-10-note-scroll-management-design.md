# Note Panel Scroll Management — Design Spec

**Date:** 2026-06-10  
**Status:** Approved

---

## Problem

Two related scroll bugs exist in the notes document panel:

1. **Notes added in Capture mode are not visible.** When the document is longer than the panel, newly inserted notes appear below the visible area and the user cannot see them.
2. **Snap-to-top when entering Capture mode.** After navigating to a section in VimNormal mode, pressing Esc to enter the Capture box causes the document to snap back to the top, losing the user's place.

### Root Cause

In `src/ui/document.rs:142-150`, the document scroll offset is computed inline during render. It uses `cursor_line` in the vim-active branch; in all other modes (including `Capture`), scroll is hard-reset to `0`.

```rust
// Current code — Capture always shows top of document
let scroll_offset: usize = if vim_active {
    cursor_line.saturating_sub(visible_height.saturating_sub(1))
} else {
    0   // <-- bug: ignores context and last insertion
};
```

---

## Goals

- In **Capture mode**, the document panel scrolls to show the **current context section heading** when entering Capture (e.g., `## Notes`, `### My Meeting`).
- After each **insertion from Capture** (pressing Enter), the panel scrolls to show the **newly inserted line**.
- In **VimNormal / VimInsert**, scroll continues to follow the vim cursor at the bottom of the viewport (existing behavior preserved).

---

## Approach: Single Anchor-Line Field

The render function's signature is `render(frame, app: &AppState, area, theme)` — `app` is immutable. Rather than adding a dedicated scroll-update action, we add **one new field** to `AppState` and update it in input handlers. The render formula is extended to use this field.

---

## State Change

**File:** `src/app/state.rs`

Add one field to `AppState`:

```rust
/// The document line used as the scroll anchor.
/// - In VimNormal/VimInsert: input handlers keep this equal to `vim.cursor_line`.
/// - In Capture: set to context heading line on Esc (vim → capture transition),
///   and updated to the inserted line number after each Capture insertion.
pub doc_anchor_line: usize,
```

Initializes to `0`.

---

## Render Change

**File:** `src/ui/document.rs`

Replace the current scroll computation (lines 142–148) with:

```rust
// Scroll: use anchor line for both vim and capture modes.
let doc_anchor = app.doc_anchor_line;
let visible_height = area.height as usize;
let scroll_offset: usize = if vim_active {
    // Cursor follows near bottom of viewport (preserves existing behavior).
    doc_anchor.saturating_sub(visible_height.saturating_sub(1))
} else {
    // Capture mode: show anchor near top of viewport (3 lines of lead).
    doc_anchor.saturating_sub(3)
};
```

The cursor placement at line 159 already uses `scroll_offset` — no change needed there.

---

## Input Handler Changes

### 1. Vim cursor movement — `src/app/input/vim_normal.rs` and `vim_insert.rs`

After any cursor movement, add:

```rust
app.doc_anchor_line = app.vim.cursor_line;
```

This keeps the anchor in sync with the vim cursor so scroll behavior is identical to today.

### 2. Esc transition (VimNormal → Capture) — `src/app/input/vim_normal.rs`

When `Esc` sets `Focus::Capture`:

```rust
app.doc_anchor_line = context_heading_line(app);
```

The anchor jumps to the section heading for the current context, scrolling the document there.

### 3. After Capture insertion — `src/app/input/capture.rs`

After the insertion action completes and the document is updated:

```rust
app.doc_anchor_line = inserted_line; // returned from insertion function
```

The anchor follows the newly appended line.

---

## New Utility: `context_heading_line`

**File:** `src/app/context.rs`

```rust
/// Returns the line index in app.doc.lines of the heading for the current context.
/// Falls back to 0 if not found.
pub fn context_heading_line(app: &App) -> usize {
    match app.context {
        Context::Section { heading_line, .. } => heading_line,
        Context::Notes        => find_heading(&app.doc.lines, "## Notes").unwrap_or(0),
        Context::Todos        => find_heading(&app.doc.lines, "## To-dos").unwrap_or(0),
        Context::Meeting(n)   => find_nth_subheading(&app.doc.lines, "## Meetings", n).unwrap_or(0),
        Context::NoteBlock(n) => find_nth_subheading(&app.doc.lines, "## Notes", n).unwrap_or(0),
    }
}
```

Helper functions `find_heading` (returns the first line matching a prefix string) and `find_nth_subheading` (returns the Nth `###` line after a given `##` heading) are added in the same file.

---

## Insertion Line Tracking

**File:** `src/model/day.rs` (or whichever function inserts into the document)

The insertion function that adds a bullet/note to the document currently returns `()`. Change the return type to `usize`, returning the **0-based line index** where the new content was inserted. This value is stored to `app.doc_anchor_line` in the Capture input handler after each successful insert.

---

## Files Changed

| File | Change |
|------|--------|
| `src/app/state.rs` | Add `doc_anchor_line: usize` field, initialize to `0` |
| `src/ui/document.rs` | Replace scroll formula to use `app.doc_anchor_line`; Capture branch no longer hard-returns `0` |
| `src/app/context.rs` | Add `context_heading_line()`, `find_heading()`, `find_nth_subheading()` |
| `src/app/input/vim_normal.rs` | Keep anchor in sync with cursor on moves; update anchor to context heading on Esc |
| `src/app/input/vim_insert.rs` | Keep anchor in sync with cursor on moves |
| `src/app/input/capture.rs` | Set anchor to inserted line after each insertion |
| `src/model/day.rs` | Return inserted line index from insertion function |

---

## Out of Scope

- Page-up / page-down keybindings in Capture mode (easy to add: adjust `doc_anchor_line` by `visible_height`)
- Cursor re-center (`Ctrl-L` refresh) — trivial to add
- Right panel scroll (separate issue; `right_panel_scroll` state already exists)
