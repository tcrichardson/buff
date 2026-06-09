# Vim UX Fixes Design

**Date:** 2026-06-09  
**Status:** Approved

## Problem Statement

Three usability issues in the vim editing experience:

1. Switching from VimNormal to Capture requires Tab, but Esc is the conventional "escape outward" key and users expect it to behave consistently with VimInsert → VimNormal.
2. In VimNormal and VimInsert there is no visible indication of which line the cursor is on — the cursor-line background is `Color::Reset` (invisible).
3. In VimInsert there is no visible cursor in the notes pane — a bug causes `render_input` to overwrite the cursor position set by `document::render` every frame.

## Approach

Approach A — targeted fixes. Each issue addressed at its call site with minimal new structure.

---

## Section 1: Key Bindings

**Files:** `src/app/input/mod.rs`

### Mode navigation (Esc chain)

Esc moves "outward" through the mode stack:

```
VimInsert --Esc--> VimNormal --Esc--> Capture
```

- `Esc` in `Focus::VimNormal` changes from `None` (no-op) to `SwitchToCapture`.
- Entering vim modes from Capture is unchanged (Esc in Capture → VimNormal; then `i`/`a`/`o` → VimInsert).

### Pane focus cycle (Tab / BackTab)

Tab and BackTab cycle between the notes and right-panel panes only. Chat is read-only and removed from the cycle.

```
VimNormal <--Tab/BackTab--> RightPanel
```

| Handler | Current | After |
|---|---|---|
| Tab, `VimNormal`/`VimInsert` | `SwitchToCapture` | `FocusRightPanel` |
| Tab, `Chat` | `FocusRightPanel` | `FocusRightPanel` (unchanged) |
| BackTab, `RightPanel` | `FocusChat` if visible, else `FocusVimNormal` | `FocusVimNormal` unconditionally |

`FocusChat` is no longer reachable via Tab/BackTab. Chat remains accessible (visible/hidden via `Ctrl-L`); Esc from Chat still blurs to Capture.

### Test changes required

- `vimnormal_tab_switches_to_capture` → assert `FocusRightPanel`
- `backtab_in_right_panel_goes_to_chat_when_visible` → assert `FocusVimNormal`

---

## Section 2: Cursor Line Highlight

**Files:** `src/ui/theme.rs`, `src/ui/document.rs`

### Theme

Add `vim_cursor_line: Color` to the `Theme` struct and `ThemeOverrides` struct. Default values:

| Theme | Value | Notes |
|---|---|---|
| Light | `Color::Rgb(219, 234, 254)` | Pale blue tint |
| Dark | `Color::Rgb(40, 44, 52)` | Slightly elevated dark gray |

Add `apply!(vim_cursor_line)` to `resolve_theme` so it is user-overridable via config.

### Rendering

In `document::render`, the cursor-line background changes from `theme.notes_panel_bg` (currently `Color::Reset`) to `theme.vim_cursor_line`:

```rust
let bg_style = Style::default().bg(theme.vim_cursor_line);
```

The same highlight is used in both VimNormal and VimInsert — the mode line (`-- NORMAL --` / `-- INSERT --`) already distinguishes modes. Using a different highlight per mode would add noise without adding clarity.

---

## Section 3: Cursor Visibility and Shape

**Files:** `src/ui/capture.rs`, `src/main.rs`

### Fix: cursor position overwrite bug

`capture::render_input` calls `frame.set_cursor_position` unconditionally (line 80). It runs after `document::render`, overwriting the vim cursor position on every frame.

Fix: guard the call behind a focus check:

```rust
if app.focus == Focus::Capture {
    frame.set_cursor_position(...);
}
```

### Cursor shape

After `terminal.draw(...)` each frame in `main.rs`, emit a raw crossterm cursor-shape command:

```rust
use ratatui::crossterm::execute;
use ratatui::crossterm::cursor::SetCursorStyle;

match app.focus {
    Focus::VimNormal => execute!(std::io::stdout(), SetCursorStyle::SteadyBlock)?,
    Focus::VimInsert => execute!(std::io::stdout(), SetCursorStyle::SteadyBar)?,
    _                => execute!(std::io::stdout(), SetCursorStyle::DefaultUserShape)?,
}
```

- `SteadyBlock` in Normal — marks the character under the cursor (standard vim convention).
- `SteadyBar` in Insert — marks the insertion point between characters.
- `DefaultUserShape` for all other modes — restores the terminal's configured preference.

`ratatui::restore()` (called via `TerminalGuard::drop` on exit) handles terminal cleanup, including cursor state.

No new modules, no new state. `SetCursorStyle` is available through `ratatui::crossterm`.

---

## Files Changed

| File | Change |
|---|---|
| `src/app/input/mod.rs` | Esc in VimNormal → SwitchToCapture; Tab in VimNormal/VimInsert → FocusRightPanel; BackTab in RightPanel → FocusVimNormal unconditionally |
| `src/ui/theme.rs` | Add `vim_cursor_line` to `Theme`, `ThemeOverrides`, light/dark defaults, `resolve_theme` |
| `src/ui/document.rs` | Use `theme.vim_cursor_line` for cursor-line background |
| `src/ui/capture.rs` | Guard `set_cursor_position` with `Focus::Capture` check |
| `src/main.rs` | Emit `SetCursorStyle` after each `terminal.draw` |

## Test Changes

| Test | Change |
|---|---|
| `vimnormal_tab_switches_to_capture` | Assert `FocusRightPanel` |
| `backtab_in_right_panel_goes_to_chat_when_visible` | Assert `FocusVimNormal` |
| New: `vimnormal_esc_switches_to_capture` | Assert Esc in VimNormal → `SwitchToCapture` |
| New: `render_vim_normal_cursor_line_has_highlight` | Assert cursor line cell has `vim_cursor_line` background |

## Notes

- `render_navigate_mode` (in `layout.rs` tests) asserts `Modifier::REVERSED` on any cell. Ratatui's `TestBackend` applies `REVERSED` to the cursor cell when `set_cursor_position` is called. This test continues to pass unchanged — we are adding a background color to the cursor line but not removing the `set_cursor_position` call.
- `SetCursorStyle` is a crossterm type exposed via `ratatui::crossterm::cursor`. No new Cargo dependencies are required.
