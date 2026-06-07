# UI/UX Improvements Design

**Date:** 2026-06-07
**Status:** Approved

## Overview

Seven related UI/UX improvements to buff's TUI, implemented incrementally in four feature areas: keyboard behaviour, focus borders, configurable pane sizing, and layout restructuring.

Implementation order (lowest risk first):

1. Keyboard changes
2. Focus borders
3. Config percentage sizing
4. Layout restructuring

---

## 1. Keyboard Changes

### 1a. ESC — deliberate toggle between input and navigation

ESC is the consistent toggle between Capture (input) mode and all other interaction states. No code changes required; this documents the existing intended model.

| Current focus | ESC result |
|---|---|
| `Capture` (edit in progress) | Cancel edit, stay `Capture` |
| `Capture` (no active edit) | → `Navigate` |
| `Navigate` | → `Capture` |
| `RightPanel` | → `Capture` |
| `Chat` | → `Capture` |

ESC means "return to input". Tab/Shift+Tab means "move between panes".

### 1b. Shift+Tab in Capture mode — un-indent (`RemoveIndent`)

New `UiAction::RemoveIndent`, triggered by `KeyCode::BackTab` when `focus == Focus::Capture`.

**Logic:**
1. Find the byte offset of the start of the current line (identical to `PrependIndent`'s line-start calculation).
2. If the line starts with `->`, remove those 2 bytes.
3. Adjust `cursor_pos`: if cursor was past the line start, subtract 2 (clamped to line start).
4. If the line does not start with `->`, no-op.

Mirrors `PrependIndent` (Ctrl+.) as its inverse.

### 1c. Tab / Shift+Tab focus cycle in Navigate mode

New `UiAction::FocusNavigate` added to handle the wrap-around return to the notes pane.

**Cycle order (left to right):** Notes (Navigate) → Chat (if visible) → RightPanel → Notes (wrap)

**Tab mappings by focus:**

| Current focus | Tab result |
|---|---|
| `Capture` | `TypeIndent` (insert `->`) — unchanged |
| `Navigate` | `FocusChat` (if chat visible) or `FocusRightPanel` |
| `Chat` | `FocusRightPanel` |
| `RightPanel` | `FocusNavigate` (wrap back to notes) |

**Shift+Tab mappings by focus:**

| Current focus | Shift+Tab result |
|---|---|
| `Capture` | `RemoveIndent` (un-indent current line) |
| `Navigate` | `FocusRightPanel` — reverse wrap (always jumps to rightmost pane) |
| `Chat` | `FocusNavigate` |
| `RightPanel` | `FocusChat` (if chat visible) or `FocusNavigate` |

**`FocusNavigate` execute behavior:** sets `state.focus = Focus::Navigate` and clears `pending_delete`.

**Note:** `RightPanelBlur` (ESC from RightPanel → Capture) and `ChatBlur` (ESC from Chat → Capture) are unchanged. Only Tab-driven movement uses `FocusNavigate`.

---

## 2. Focus Borders

Each content pane gets a `Block::borders(Borders::ALL)` wrapper. Border color signals focus state.

| Border state | Color |
|---|---|
| Focused pane | `Color::Cyan` |
| Unfocused panes | `Color::DarkGray` |

**Focus → pane mapping:**

| `app.focus` | Focused pane |
|---|---|
| `Focus::Capture` | Notes |
| `Focus::Navigate` | Notes |
| `Focus::Chat` | Chat |
| `Focus::RightPanel` | Right Panel |

Both `Capture` and `Navigate` highlight the Notes pane. The distinction between those two modes is input vs. selection within the pane, not which pane is active.

**Layout impact:** Each border consumes 1 cell on each side. Content is rendered on the `block.inner(area)` rect. No explicit size adjustment needed — ratatui handles this automatically.

**Scope:** Header, footer (input), and status bar do not get borders. Borders apply to the three content panes only: notes document area, chat panel, and right panel.

---

## 3. Config — Percentage Sizing

`chat_width` is removed from `Config`. A new `PaneSize` type is introduced for `panel_width` only.

### PaneSize type

```rust
#[derive(Clone, Debug)]
pub enum PaneSize {
    Columns(u16),   // fixed terminal column count
    Percent(u16),   // 0–100, percentage of total terminal width
}
```

Custom `serde::Deserialize` impl accepts:
- Integer `30` → `PaneSize::Columns(30)`
- Quoted string `"25%"` → `PaneSize::Percent(25)`

Default: `PaneSize::Columns(30)` — preserves backward compatibility for existing configs.

### Config changes

```rust
pub struct Config {
    // ...
    pub panel_width: PaneSize,   // was u16, now PaneSize
    // chat_width removed
}
```

### Layout logic

```rust
fn pane_size_to_constraint(size: &PaneSize) -> Constraint {
    match size {
        PaneSize::Columns(n) => Constraint::Length(*n),
        PaneSize::Percent(p) => Constraint::Percentage(*p),
    }
}
```

**Pane allocation:**
- Right panel → `pane_size_to_constraint(&config.panel_width)`
- Notes + Chat area → `Constraint::Min(0)` (takes everything remaining)
  - Chat visible: notes and chat each get `Constraint::Percentage(50)` of that area
  - Chat hidden: notes gets `Constraint::Min(0)` (100% of remaining)

`Ctrl+L` toggling chat is now a genuine "give notes more space" action.

---

## 4. Layout Restructuring

### Current structure

```
frame.area() [horizontal]
├── left_area [vertical]
│   ├── title_area (Length 5)
│   ├── document_area (Min 0)
│   ├── status_area (Length 1)
│   └── input_area (Length dynamic)
├── chat_area [optional, Length chat_width]
└── panel_area [Length panel_width]
```

### New structure

```
frame.area() [horizontal]
├── main_area (Min 0)  [vertical]
│   ├── header_area (Length 5)       ← buff logo, date, context — spans notes+chat
│   ├── content_row (Min 0)          ← horizontal split
│   │   ├── notes_area               ← Percentage(50) if chat visible, else Min(0)
│   │   └── chat_area                ← Percentage(50), only when chat visible
│   ├── status_area (Length 1)       ← spans notes+chat
│   └── input_area (Length dynamic)  ← spans notes+chat
└── panel_area                       ← PaneSize constraint, full terminal height
```

The outer split is horizontal (main + panel). The right panel spans the full terminal height independently. The inner split of `main_area` is vertical (header → content → status → input). The content row is a further horizontal split (notes + optional chat).

### Header content

The header spans the full `main_area` width. Layout within the header:
- Left side: buff ASCII art (unchanged)
- Right side (Length 30): date + context display (unchanged)

### Footer (input) content

The input box spans the full `main_area` width. Behaviour and rendering logic unchanged — only the area it occupies changes.

### Status bar

Sits between content row and input, spans full `main_area` width. Unchanged.

### Test impact

Existing layout tests use `TestBackend::new(80, 24)`. The structural assertions (ASCII art present, date present, todo checkboxes present) continue to pass. Tests that rely on specific cell positions within the notes area may need adjustment due to the 1-cell border inset on each side of the notes pane. The `render_navigate_mode` test (checks for `REVERSED` modifier) is unaffected.

---

## Affected Files

| File | Change |
|---|---|
| `src/app/input.rs` | Add `RemoveIndent`, `FocusNavigate` actions; update `key_to_action` for `BackTab`; update Tab cycle for `RightPanel` |
| `src/app/state.rs` | No changes required |
| `src/config.rs` | Add `PaneSize` type with custom serde; replace `panel_width: u16` with `panel_width: PaneSize`; remove `chat_width` |
| `src/ui/layout.rs` | Full restructure; outer horizontal split (main + panel); inner vertical split; nested content row; draw border `Block` for each content pane; pass `block.inner(area)` to render functions |
| `src/ui/document.rs` | No logic changes; receives a smaller inner area from layout — remove any existing `Block` wrapper if present to avoid double borders |
| `src/ui/chat_panel.rs` | No logic changes; receives a smaller inner area from layout — remove any existing `Block` wrapper if present |
| `src/ui/right_panel.rs` | No logic changes; receives a smaller inner area from layout — remove any existing `Block` wrapper if present |
| `src/ui/capture.rs` | No logic changes; input and status areas are passed from layout (now spanning `main_area` width) |
