# Right Panel Design

**Date:** 2026-06-05  
**Status:** Approved

## Overview

Add a permanently visible right-side panel to the buff TUI. The panel holds:

1. A calendar widget (always visible, shows current month) at the top
2. A scrollable, interactive list of all incomplete todos from the last N days (default 7), grouped by day

The existing calendar popup overlay (`Overlay::Calendar`, `Ctrl-G`, `/goto` with no args) is removed — the calendar in the panel replaces it entirely.

---

## Layout

`src/ui/layout.rs` `render()` is changed to perform a horizontal split first:

- **Left column** (fills remaining width): existing vertical stack — title bar (5 lines), document area, status bar (1 line), input box (3–12 lines)
- **Right column** (fixed width, default 30 cols): `right_panel::render()`

The right panel spans the full terminal height. Within the panel:

1. Calendar (top, ~9 lines): month header + day-of-week row + up to 6 week rows
2. Horizontal rule
3. Todo list (fills remainder): `To-dos` heading, then date-grouped incomplete todo items

**Config fields** (both `u16`, with `serde(default)`):

```toml
panel_width = 30          # terminal columns; default 30
todo_lookback_days = 7    # days to scan back; default 7
```

---

## New Module: `src/ui/right_panel.rs`

A single `render(frame, area, app)` function. No persistent widget struct — just pure rendering from `AppState`.

### Calendar section

- Reuses `calendar::weeks()` and `calendar::marked()` (these stay in `calendar.rs`)
- Always shows the month containing `app.date`
- Highlights `app.date` as the selected date
- Respects `config.week_starts_on`
- Display only — no month navigation from the panel

### Todo list section

Iterates `app.panel_todos` (pre-computed in `AppState`), rendering:

```
To-dos
─ Fri Jun 05 ──────────────
☐ Write tests for parser
☐ Deploy to staging
─ Thu Jun 04 ──────────────
☐ Review PR #42
```

- Shows only incomplete todos (`SelectableKind::Todo { done: false }`)
- Most recent day first
- The item at index `app.right_panel_selected` is highlighted with `Modifier::REVERSED` when `app.focus == Focus::RightPanel`
- Scrolls if content exceeds available height (scroll offset stored in `AppState` as `right_panel_scroll`)
- Scroll rule: after each `↑`/`↓` keypress, if the selected item is outside the visible window, adjust `right_panel_scroll` to keep it visible (scroll-follow behavior)

---

## State Changes: `src/app/state.rs`

### `Focus` enum — new variant

```rust
pub enum Focus {
    Capture,
    Navigate,
    RightPanel,   // new
}
```

### `PanelTodo` struct (defined in `src/ui/right_panel.rs`, re-exported)

```rust
pub struct PanelTodo {
    pub date: NaiveDate,
    pub text: String,
    pub todo_index: usize,  // index into that day's Document::selectables()
}
```

### `AppState` — new fields

```rust
pub right_panel_selected: usize,     // highlighted index in panel_todos
pub panel_todos: Vec<PanelTodo>,     // flat list, most-recent-day first
pub right_panel_scroll: usize,       // scroll offset for todo list
```

### `AppState` — removed fields

- `calendar: Option<CalendarState>` — removed (calendar overlay is gone)

### `Overlay` enum — removed variant

- `Overlay::Calendar` — removed

### `panel_todos` refresh triggers

`panel_todos` is rebuilt (re-scanning disk) in three situations:

1. App startup (`AppState::open_day`)
2. Date navigation (any `go_to_date` call in `actions.rs`)
3. After toggling a todo from the right panel

Scanning 7 markdown files is negligible — buff already reads files on every date navigation.

### `panel_todos` builder function

A public `fn collect_panel_todos(notes_dir, date, config) -> Vec<PanelTodo>` in `right_panel.rs`:

- Iterates from `date` back `config.todo_lookback_days` days
- For each day: reads the file (skips if not found), parses with `Document::from_text()`, calls `doc.selectables()`, filters for `SelectableKind::Todo { done: false }`
- Returns the flat `Vec<PanelTodo>` sorted most-recent-day first

---

## Keyboard Navigation: `src/app/input.rs`

### From document focus (`Focus::Capture` or `Focus::Navigate`)

| Key | Action |
|-----|--------|
| `Tab` | Move focus to `Focus::RightPanel`; set `right_panel_selected = 0` |

### From `Focus::RightPanel`

| Key | Action |
|-----|--------|
| `↑` / `k` | Decrement `right_panel_selected` (clamp to 0) |
| `↓` / `j` | Increment `right_panel_selected` (clamp to `panel_todos.len() - 1`) |
| `Space` / `x` | Toggle selected todo (see below) |
| `Tab` | Return to `Focus::Capture` |
| `Escape` | Return to `Focus::Capture` |

### Toggle action (panel todo)

1. Look up `panel_todos[right_panel_selected]` → get `(date, todo_index)`
2. Load that day's file from disk
3. Call `doc.toggle_todo(todo_index)`
4. Save the file
5. If `date == app.date`: also update `app.doc` and `app.selectables` so the left document view reflects the change immediately
6. Rebuild `app.panel_todos` (the todo is now done, so it drops off the list)
7. Clamp `right_panel_selected` to the new list length

---

## Removals

| Item | Location | Action |
|------|----------|--------|
| `Overlay::Calendar` variant | `state.rs` | Remove |
| `app.calendar` field | `state.rs` | Remove |
| Calendar overlay rendering | `layout.rs` | Remove |
| `Ctrl-G` keybinding | `input.rs` | Remove |
| `/goto` (no-args case) | `command.rs` | Remove |
| `render_calendar_overlay` test | `layout.rs` | Remove or update |

The `/goto <date>` form (with an explicit date argument) is **kept** — it navigates to that date directly without opening any popup.

`CalendarState`, `weeks()`, `marked()`, and `move_selection()` in `calendar.rs` are **kept** — `right_panel.rs` reuses `weeks()` and `marked()` for its inline calendar rendering. `CalendarState` and `move_selection()` are no longer used and can be removed.

---

## Testing

- `right_panel::collect_panel_todos` — unit tests: empty dir, single day with todos, multiple days, days beyond lookback window, all-done todos (should not appear)
- `right_panel::render` — integration test using `TestBackend`: verify calendar renders, verify todo items appear, verify selected item has REVERSED style
- `layout.rs` — update `render_calendar_overlay` test (remove or replace with a panel-visible test)
- `input.rs` — unit tests for Tab focus cycle, arrow key navigation, space toggle
