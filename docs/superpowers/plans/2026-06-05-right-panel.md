# Right Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a permanently visible right-side panel to the buff TUI showing an inline calendar and a scrollable, interactive list of incomplete todos from the last N days.

**Architecture:** Single new module `src/ui/right_panel.rs` holds `PanelTodo`, `collect_panel_todos()`, and `render()`. `AppState` gains three new fields (`panel_todos`, `right_panel_selected`, `right_panel_scroll`) and loses the calendar overlay fields. Layout gets a horizontal split; the left column is the existing stack, the right column is the new panel.

**Tech Stack:** Rust, ratatui 0.30, crossterm, chrono — all already in use.

---

## File Map

| Action | File | Change |
|--------|------|--------|
| Create | `src/ui/right_panel.rs` | `PanelTodo`, `collect_panel_todos()`, `render()` |
| Modify | `src/ui/mod.rs` | expose `right_panel` module |
| Modify | `src/config.rs` | add `panel_width: u16`, `todo_lookback_days: u16` |
| Modify | `src/app/state.rs` | add `Focus::RightPanel`; add `panel_todos`, `right_panel_selected`, `right_panel_scroll`; remove `calendar` and `Overlay::Calendar`; call `collect_panel_todos` in `open_day` |
| Modify | `src/app/input.rs` | add `FocusRightPanel`, `RightPanelUp`, `RightPanelDown`, `RightPanelToggle`, `RightPanelBlur` UiActions; handle `Focus::RightPanel` in `key_to_action`; handle Tab; remove all calendar UiActions and Ctrl-G; update/remove calendar tests |
| Modify | `src/app/actions.rs` | add `toggle_panel_todo()`; update `go_to_date` to refresh `panel_todos`; remove `Goto(None)` calendar code |
| Modify | `src/ui/layout.rs` | horizontal split; call `right_panel::render`; remove `Overlay::Calendar` branch; update tests |
| Modify | `src/ui/calendar.rs` | remove `CalendarState` and `move_selection` (now unused) |

---

## Task 1: Config — panel_width + todo_lookback_days

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block at the bottom of `src/config.rs`:

```rust
#[test]
fn panel_width_default_is_30() {
    let config = Config::default();
    assert_eq!(config.panel_width, 30);
}

#[test]
fn todo_lookback_days_default_is_7() {
    let config = Config::default();
    assert_eq!(config.todo_lookback_days, 7);
}

#[test]
fn parse_panel_fields_from_toml() {
    let toml = r#"
        panel_width = 40
        todo_lookback_days = 14
    "#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.panel_width, 40);
    assert_eq!(config.todo_lookback_days, 14);
}

#[test]
fn panel_fields_use_defaults_when_absent() {
    let toml = r#"timestamp_entries = true"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.panel_width, 30);
    assert_eq!(config.todo_lookback_days, 7);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -q 2>&1 | grep -E "FAILED|error"
```

Expected: compile error — `panel_width` and `todo_lookback_days` not found on `Config`.

- [ ] **Step 3: Add fields to Config**

In `src/config.rs`, update the `Config` struct and its `Default` impl:

```rust
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub notes_dir: Option<String>,
    pub timestamp_entries: bool,
    pub week_starts_on: WeekStart,
    pub date_format: String,
    pub panel_width: u16,
    pub todo_lookback_days: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            notes_dir: None,
            timestamp_entries: false,
            week_starts_on: WeekStart::Sunday,
            date_format: "%Y-%m-%d-%a".to_string(),
            panel_width: 30,
            todo_lookback_days: 7,
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -q config
```

Expected: all config tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: add panel_width and todo_lookback_days to Config"
```

---

## Task 2: PanelTodo struct + collect_panel_todos

**Files:**
- Create: `src/ui/right_panel.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Expose the module in mod.rs**

Replace the contents of `src/ui/mod.rs` with:

```rust
pub mod calendar;
mod capture;
mod document;
pub mod help;
pub mod layout;
pub mod right_panel;

pub use layout::render;
```

- [ ] **Step 2: Create right_panel.rs with struct and tests**

Create `src/ui/right_panel.rs` with the struct, the function, and tests (leave render as a stub for now):

```rust
use crate::config::Config;
use crate::model::day::{Document, SelectableKind};
use crate::storage;
use chrono::NaiveDate;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelTodo {
    pub date: NaiveDate,
    pub text: String,
    pub todo_index: usize, // index into that day's Document::selectables()
}

/// Strip "- [ ] " or "- [x] " prefix and any trailing " _(Tag)_" meeting tag.
/// Returns the first line only (multi-line todos are shown truncated).
fn display_text(raw: &str) -> String {
    let stripped = raw
        .strip_prefix("- [ ] ")
        .or_else(|| raw.strip_prefix("- [x] "))
        .unwrap_or(raw);
    let first_line = stripped.lines().next().unwrap_or(stripped);
    // Strip trailing " _(Tag)_"
    if let Some(tag_start) = first_line.rfind(" _(") {
        if first_line.ends_with(")_") {
            return first_line[..tag_start].to_string();
        }
    }
    first_line.to_string()
}

/// Collect all incomplete todos from the last `config.todo_lookback_days` days
/// (including `date` itself), most-recent-day first.
pub fn collect_panel_todos(notes_dir: &Path, date: NaiveDate, config: &Config) -> Vec<PanelTodo> {
    let mut todos = Vec::new();
    for offset in 0..config.todo_lookback_days {
        let day = date - chrono::Duration::days(offset as i64);
        let path = storage::path_for(notes_dir, day, &config.date_format);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let doc = Document::from_text(&text);
        for (sel_index, sel) in doc.selectables().iter().enumerate() {
            if matches!(sel.kind, SelectableKind::Todo { done: false }) {
                todos.push(PanelTodo {
                    date: day,
                    text: display_text(&sel.text),
                    todo_index: sel_index,
                });
            }
        }
    }
    todos
}

pub fn render(
    _frame: &mut ratatui::Frame,
    _area: ratatui::layout::Rect,
    _app: &crate::app::state::AppState,
) {
    // stub — implemented in Task 7 and 8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use chrono::NaiveDate;

    fn jun5() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()
    }

    fn write_day(dir: &std::path::Path, date: NaiveDate, content: &str) {
        let config = Config::default();
        let path = storage::path_for(dir, date, &config.date_format);
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn empty_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert!(todos.is_empty());
    }

    #[test]
    fn done_todos_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        let content = "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [x] done thing\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert!(todos.is_empty());
    }

    #[test]
    fn incomplete_todo_is_included() {
        let tmp = tempfile::tempdir().unwrap();
        let content = "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] buy milk\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].text, "buy milk");
        assert_eq!(todos[0].date, jun5());
    }

    #[test]
    fn mixed_todos_only_incomplete_returned() {
        let tmp = tempfile::tempdir().unwrap();
        let content =
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] buy milk\n- [x] done\n- [ ] call bank\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].text, "buy milk");
        assert_eq!(todos[1].text, "call bank");
    }

    #[test]
    fn multiple_days_most_recent_first() {
        let tmp = tempfile::tempdir().unwrap();
        let jun4 = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        write_day(
            tmp.path(),
            jun5(),
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] today task\n",
        );
        write_day(
            tmp.path(),
            jun4,
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] yesterday task\n",
        );
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].date, jun5());
        assert_eq!(todos[1].date, jun4);
    }

    #[test]
    fn days_beyond_lookback_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        // 8 days ago — outside default window of 7
        let old = jun5() - chrono::Duration::days(8);
        write_day(
            tmp.path(),
            old,
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] old task\n",
        );
        let config = Config::default(); // todo_lookback_days = 7
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert!(todos.is_empty());
    }

    #[test]
    fn lookback_boundary_is_inclusive() {
        let tmp = tempfile::tempdir().unwrap();
        // 6 days ago — inside default window of 7 (offsets 0..7 = 0,1,2,3,4,5,6)
        let border = jun5() - chrono::Duration::days(6);
        write_day(
            tmp.path(),
            border,
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] border task\n",
        );
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].text, "border task");
    }

    #[test]
    fn meeting_tag_stripped_from_display() {
        let tmp = tempfile::tempdir().unwrap();
        let content =
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] follow up _(Standup)_\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].text, "follow up");
    }

    #[test]
    fn todo_index_matches_selectable_position() {
        let tmp = tempfile::tempdir().unwrap();
        // A doc with a bullet then a todo — todo should be at selectable index 1
        let content =
            "# Day\n\n## Meetings\n\n## Notes\n- some note\n\n## To-dos\n- [ ] the task\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 1);

        // Verify the stored todo_index actually points to the right selectable
        let doc = Document::from_text(
            "# Day\n\n## Meetings\n\n## Notes\n- some note\n\n## To-dos\n- [ ] the task\n",
        );
        let selectables = doc.selectables();
        assert!(
            matches!(selectables[todos[0].todo_index].kind, SelectableKind::Todo { done: false }),
            "expected todo at todo_index {}, got {:?}",
            todos[0].todo_index,
            selectables[todos[0].todo_index].kind
        );
    }
}
```

- [ ] **Step 3: Run tests to verify they pass**

```bash
cargo test -q right_panel
```

Expected: all right_panel tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/ui/right_panel.rs src/ui/mod.rs
git commit -m "feat: add PanelTodo and collect_panel_todos"
```

---

## Task 3: Add Focus::RightPanel + update all match arms

**Files:**
- Modify: `src/app/state.rs`
- Modify: `src/app/input.rs`

- [ ] **Step 1: Add Focus::RightPanel to state.rs**

In `src/app/state.rs`, update the `Focus` enum:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Capture,
    Navigate,
    RightPanel,
}
```

- [ ] **Step 2: Fix non-exhaustive matches in input.rs**

The Esc handler and mode-specific handler in `src/app/input.rs` will fail to compile. Add stub handling:

In the Esc block:
```rust
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
        Focus::RightPanel => None, // replaced in Task 9
    };
}
```

In the mode-specific match at the end, add a stub arm:
```rust
Focus::RightPanel => None, // replaced in Task 9
```

- [ ] **Step 3: Verify the project compiles and all tests pass**

```bash
cargo test -q
```

Expected: all tests pass, no errors.

- [ ] **Step 4: Commit**

```bash
git add src/app/state.rs src/app/input.rs
git commit -m "feat: add Focus::RightPanel variant with stub input handling"
```

---

## Task 4: Add panel_todos / right_panel_selected / right_panel_scroll to AppState

**Files:**
- Modify: `src/app/state.rs`
- Modify: `src/ui/layout.rs` (only the test helper `test_app` to supply new fields)

- [ ] **Step 1: Write a failing test**

Add to `src/app/actions.rs` tests:

```rust
#[test]
fn open_day_populates_panel_todos_from_past_files() {
    let tmp = tempfile::tempdir().unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let past = date - chrono::Duration::days(2);
    let config = Config::default();
    let past_path = crate::storage::path_for(tmp.path(), past, &config.date_format);
    std::fs::write(
        &past_path,
        "# 2026-06-03 (Wed)\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] past task\n",
    )
    .unwrap();

    let state = AppState::open_day(tmp.path().to_path_buf(), config, date).unwrap();
    assert_eq!(state.panel_todos.len(), 1);
    assert_eq!(state.panel_todos[0].text, "past task");
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -q open_day_populates_panel_todos 2>&1 | head -5
```

Expected: compile error — `panel_todos` not found on `AppState`.

- [ ] **Step 3: Add fields to AppState and update open_day**

In `src/app/state.rs`:

1. Add import at top:
```rust
use crate::ui::right_panel::{self, PanelTodo};
```

2. Add fields to `AppState`:
```rust
pub struct AppState {
    // ... existing fields ...
    pub right_panel_selected: usize,
    pub right_panel_scroll: usize,
    pub panel_todos: Vec<PanelTodo>,
}
```

3. Update `open_day` to populate the new fields (add just before the closing `Ok(Self { ... })`):
```rust
let panel_todos = right_panel::collect_panel_todos(&notes_dir, date, &config);
Ok(Self {
    doc,
    date,
    notes_dir,
    config,
    context: Context::Notes,
    focus: Focus::Capture,
    selected: 0,
    status: String::new(),
    input: String::new(),
    overlay: Overlay::None,
    editing: None,
    should_quit: false,
    selectables,
    context_display,
    pending_delete: false,
    calendar: None,
    dates_with_notes,
    right_panel_selected: 0,
    right_panel_scroll: 0,
    panel_todos,
})
```

- [ ] **Step 4: Update layout.rs test_app helper**

In `src/ui/layout.rs`, the `test_app` helper function needs the new fields. Add them:

```rust
fn test_app(doc: Document, focus: Focus, selected: usize) -> AppState {
    let selectables = doc.selectables();
    AppState {
        doc,
        date: NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
        notes_dir: PathBuf::from("/tmp"),
        config: Config::default(),
        context: Context::Notes,
        focus,
        selected,
        status: String::new(),
        input: String::new(),
        overlay: Overlay::None,
        editing: None,
        should_quit: false,
        selectables,
        context_display: "context: Notes".to_string(),
        pending_delete: false,
        calendar: None,
        dates_with_notes: std::collections::BTreeSet::new(),
        right_panel_selected: 0,
        right_panel_scroll: 0,
        panel_todos: Vec::new(),
    }
}
```

- [ ] **Step 5: Run all tests to verify they pass**

```bash
cargo test -q
```

Expected: all tests pass, including the new `open_day_populates_panel_todos_from_past_files`.

- [ ] **Step 6: Commit**

```bash
git add src/app/state.rs src/app/actions.rs src/ui/layout.rs
git commit -m "feat: add panel_todos, right_panel_selected, right_panel_scroll to AppState"
```

---

## Task 5: Stub right_panel render + horizontal split in layout.rs

**Files:**
- Modify: `src/ui/layout.rs`

At this point `right_panel::render` is a stub that does nothing. We add the horizontal split so the layout compiles and the app runs with an empty right column.

- [ ] **Step 1: Write a failing layout test**

Add to `src/ui/layout.rs` tests:

```rust
#[test]
fn right_panel_column_present_in_layout() {
    // The render function should not panic on an 80×24 terminal with the right panel split.
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let app = test_app(doc, Focus::Capture, 0);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    // Just verify it doesn't panic.
    terminal.draw(|frame| render(frame, &app)).unwrap();
}
```

This test will pass even before our change (it's a smoke test), but we write it now to establish the invariant.

- [ ] **Step 2: Update render() in layout.rs**

Replace the body of `render()` in `src/ui/layout.rs` with:

```rust
pub fn render(frame: &mut ratatui::Frame, app: &AppState) {
    use ratatui::layout::{Constraint, Direction, Layout};

    // Outer horizontal split: left = doc+chrome, right = panel
    let panel_width = app.config.panel_width;
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(panel_width)])
        .split(frame.area());

    let left_area = outer[0];
    let panel_area = outer[1];

    // Left column: existing vertical stack
    let input_line_count = app.input.split('\n').count().max(1) as u16;
    let input_height = (input_line_count + 2).clamp(3, 12);
    let title_height = 5u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(title_height),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(input_height),
        ])
        .split(left_area);

    let title_area = chunks[0];
    let document_area = chunks[1];
    let status_area = chunks[2];
    let input_area = chunks[3];

    // Split title area into left (ASCII art) and right (date + context)
    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)])
        .split(title_area);

    let ascii_art = r#"_             __  __
| |__  _   _  / _|/ _|
| '_ \| | | || |_ | |_
| |_) | |_| ||  _||  _|
|_.__/ \__,_||_|  |_|"#;
    let art_widget = ratatui::widgets::Paragraph::new(ascii_art);
    frame.render_widget(art_widget, title_chunks[0]);

    let meta = format!(
        "{}\n{}",
        app.date.format("%Y-%m-%d (%a)"),
        app.context_display
    );
    let meta_widget = ratatui::widgets::Paragraph::new(meta);
    frame.render_widget(meta_widget, title_chunks[1]);

    super::document::render(frame, app, document_area);
    super::capture::render_status(frame, app, status_area);
    super::capture::render_input(frame, app, input_area);

    // Right panel (stub — filled in Tasks 7 and 8)
    super::right_panel::render(frame, panel_area, app);

    // Overlays — keep the existing match unchanged for now; Calendar removed in Task 6
    match app.overlay {
        Overlay::Calendar => {
            super::calendar::render(frame, app, frame.area());
        }
        Overlay::Help => {
            super::help::render(frame, frame.area());
        }
        Overlay::None => {}
    }
}
```

Also remove the now-redundant top-level imports that are moved inside the function body:
```rust
// Remove these top-level imports (they are now inside render()):
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::Paragraph;
```
Keep:
```rust
use crate::app::state::{AppState, Overlay};
```

- [ ] **Step 3: Run tests**

```bash
cargo test -q
```

Expected: all tests pass, app renders without panic. The right panel is a blank area.

- [ ] **Step 4: Commit**

```bash
git add src/ui/layout.rs
git commit -m "feat: add horizontal split for right panel in layout (stub)"
```

---

## Task 6: Remove calendar overlay

Remove `Overlay::Calendar`, the `app.calendar` field, Ctrl-G, and all calendar UiActions. Update `actions.rs` and failing tests.

**Files:**
- Modify: `src/app/state.rs`
- Modify: `src/app/input.rs`
- Modify: `src/app/actions.rs`
- Modify: `src/ui/layout.rs` (remove test)
- Modify: `src/ui/calendar.rs` (remove CalendarState + move_selection)

- [ ] **Step 1: Remove Overlay::Calendar and app.calendar from state.rs**

In `src/app/state.rs`:

1. Remove `Overlay::Calendar` from the enum — leave only `None` and `Help`:
```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Overlay {
    None,
    Help,
}
```

2. Remove the `calendar` field from `AppState` struct and from `open_day`.

3. Remove the import: `use crate::ui::calendar::CalendarState;` (it was on `pub calendar: Option<crate::ui::calendar::CalendarState>`).

- [ ] **Step 2: Fix the `test_app` helper in layout.rs**

Remove `calendar: None,` from the `test_app` function in `src/ui/layout.rs`.

- [ ] **Step 3: Remove calendar UiActions from input.rs**

In `src/app/input.rs`:

1. Remove from the `UiAction` enum:
   - `MoveCalendar { dx: i8, dy: i8 }`
   - `ConfirmCalendar`
   - `CloseCalendar`
   - `OpenCalendar`

2. Remove the calendar overlay block in `key_to_action` (the `if state.overlay == Overlay::Calendar { ... }` block).

3. Remove Ctrl-G handling from the global hotkeys block:
```rust
// REMOVE this arm:
KeyCode::Char('g') => return Some(UiAction::OpenCalendar),
```

4. Remove the `execute_action` match arms for those four removed actions:
   - `UiAction::MoveCalendar { dx, dy } => { ... }`
   - `UiAction::ConfirmCalendar => { ... }`
   - `UiAction::CloseCalendar => { ... }`
   - `UiAction::OpenCalendar => { ... }`

5. Delete the four tests that reference the removed actions:
   - `calendar_overlay_left_moves_calendar`
   - `calendar_overlay_ignores_j`
   - `open_calendar_sets_overlay_and_clears_pending`
   - `close_calendar_clears_overlay_and_calendar`

- [ ] **Step 4: Fix actions.rs — Goto(None) case**

In `src/app/actions.rs`, find `Command::Goto(None)` in `dispatch()` and replace it with an error message:

```rust
Command::Goto(None) => {
    state.status = "usage: /goto YYYY-MM-DD".to_string();
}
```

Also remove the now-unused import (if present):
```rust
// Remove if present:
use crate::ui::calendar::CalendarState;
```

- [ ] **Step 5: Update layout.rs — remove calendar test and fix overlay match**

In `src/ui/layout.rs`:

1. Delete the `render_calendar_overlay` test.

2. Replace the overlay `match` in `render()` (which currently has three arms: `Calendar`, `Help`, `None`) with a simple `if` now that only `Help` and `None` remain:

```rust
// Replace the existing match app.overlay { ... } block with:
if app.overlay == Overlay::Help {
    super::help::render(frame, frame.area());
}
```

- [ ] **Step 6: Remove CalendarState and move_selection from calendar.rs**

In `src/ui/calendar.rs`:

1. Remove the `CalendarState` struct and its `impl` block.
2. Remove the `move_selection` function.
3. Remove unused imports that were only needed for those (check: `Block`, `Borders`, `Clear`, `Paragraph`, `Direction` may now be unused in the `render` function which is also being removed). Actually the `render` function in `calendar.rs` becomes dead — remove it too.
4. Remove the `use crate::app::state::AppState;` import if it's only used in `render`.
5. Keep: `weeks()`, `marked()`, and their tests. These are reused by `right_panel.rs`.

The file should end up with only: imports needed for `weeks`/`marked`, the `weeks()` function, the `marked()` function, and the existing tests for them.

- [ ] **Step 7: Run all tests**

```bash
cargo test -q
```

Expected: all tests pass. If `Overlay::Calendar` is referenced anywhere, the compiler will tell you — fix each reference.

- [ ] **Step 8: Commit**

```bash
git add src/app/state.rs src/app/input.rs src/app/actions.rs src/ui/layout.rs src/ui/calendar.rs
git commit -m "feat: remove calendar overlay, replace with right panel (stub)"
```

---

## Task 7: Calendar rendering in right_panel::render

**Files:**
- Modify: `src/ui/right_panel.rs`

- [ ] **Step 1: Write a failing render test**

Add to the `#[cfg(test)]` block in `src/ui/right_panel.rs`:

```rust
#[test]
fn render_shows_current_month_header() {
    use crate::app::state::{AppState, Context, Focus, Overlay};
    use crate::config::Config;
    use crate::model::day::Document;
    use chrono::NaiveDate;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let doc = Document::new_for_date(date);
    let selectables = doc.selectables();
    let app = AppState {
        doc,
        date,
        notes_dir: PathBuf::from("/tmp"),
        config: Config::default(),
        context: Context::Notes,
        focus: Focus::Capture,
        selected: 0,
        status: String::new(),
        input: String::new(),
        overlay: Overlay::None,
        editing: None,
        should_quit: false,
        selectables,
        context_display: "context: Notes".to_string(),
        pending_delete: false,
        dates_with_notes: std::collections::BTreeSet::new(),
        right_panel_selected: 0,
        right_panel_scroll: 0,
        panel_todos: Vec::new(),
    };

    let backend = TestBackend::new(30, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render(frame, frame.area(), &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("June 2026"), "expected 'June 2026', got: {}", content);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -q render_shows_current_month_header 2>&1 | head -10
```

Expected: FAIL (render is a stub).

- [ ] **Step 3: Implement calendar rendering in right_panel::render**

Replace the stub `render` function in `src/ui/right_panel.rs`:

```rust
use crate::app::state::AppState;
use crate::config::WeekStart;
use crate::ui::calendar;
use chrono::Datelike;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};

pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState) {
    use crate::app::state::Focus;

    // Split the panel: calendar top (fixed 9 lines) + todo list (rest)
    let calendar_height = 9u16; // header + day-names + up to 6 weeks (some months need 6)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(calendar_height), Constraint::Min(0)])
        .split(area);

    render_calendar(frame, chunks[0], app);
    render_todo_list(frame, chunks[1], app);
}

fn render_calendar(frame: &mut ratatui::Frame, area: Rect, app: &AppState) {
    use chrono::NaiveDate;

    let visible_month = (app.date.year(), app.date.month());
    let weeks_grid = calendar::weeks(visible_month, app.config.week_starts_on);
    let dates_with_notes = &app.dates_with_notes;

    // Month header (centered)
    let (year, month) = visible_month;
    let month_name = NaiveDate::from_ymd_opt(year, month, 1)
        .map(|d| d.format("%B %Y").to_string())
        .unwrap_or_default();

    // Split calendar area: header (1) + day-names (1) + weeks (rest)
    let cal_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    let header_widget = Paragraph::new(month_name).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(header_widget, cal_chunks[0]);

    let day_names: Vec<&str> = match app.config.week_starts_on {
        WeekStart::Sunday => vec!["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
        WeekStart::Monday => vec!["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    };
    let names_row = Row::new(day_names);
    let names_table =
        Table::new(vec![names_row], [Constraint::Length(3); 7]);
    frame.render_widget(names_table, cal_chunks[1]);

    let mut rows = Vec::new();
    for week in &weeks_grid {
        let mut cells = Vec::new();
        for day_opt in week {
            match day_opt {
                Some(date) => {
                    let is_today = *date == app.date;
                    let has_note = calendar::marked(*date, dates_with_notes);
                    let marker = if has_note { "·" } else { " " };
                    let text = format!("{:>2}{}", date.day(), marker);
                    let style = if is_today {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    cells.push(Cell::from(text).style(style));
                }
                None => {
                    cells.push(Cell::from("   "));
                }
            }
        }
        rows.push(Row::new(cells));
    }

    let table = Table::new(rows, [Constraint::Length(3); 7]);
    frame.render_widget(table, cal_chunks[2]);
}

fn render_todo_list(frame: &mut ratatui::Frame, area: Rect, app: &AppState) {
    use crate::app::state::Focus;

    // Build lines: header + date groups + todo items
    // We compute the full virtual list then apply scroll_offset.
    let mut virtual_lines: Vec<(bool, usize, Line)> = Vec::new();
    // (is_todo_item, flat_todo_index, line)

    virtual_lines.push((false, 0, Line::from("To-dos")));

    let mut current_date = None;
    for (flat_idx, todo) in app.panel_todos.iter().enumerate() {
        if Some(todo.date) != current_date {
            current_date = Some(todo.date);
            let header = todo.date.format("%a %b %d").to_string();
            virtual_lines.push((false, flat_idx, Line::from(format!("─ {} ", header))));
        }
        let is_selected =
            app.focus == Focus::RightPanel && flat_idx == app.right_panel_selected;
        let item_text = format!("☐ {}", todo.text);
        let style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        virtual_lines.push((true, flat_idx, Line::styled(item_text, style)));
    }

    let scroll = app.right_panel_scroll.min(virtual_lines.len().saturating_sub(1));
    let visible: Vec<Line> = virtual_lines
        .into_iter()
        .skip(scroll)
        .take(area.height as usize)
        .map(|(_, _, line)| line)
        .collect();

    let widget = Paragraph::new(visible);
    frame.render_widget(widget, area);
}
```

Also add these `use` statements at the top of the file (merging with existing ones):
```rust
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Cell, Paragraph, Row, Table};
```

- [ ] **Step 4: Run tests**

```bash
cargo test -q
```

Expected: all tests pass including `render_shows_current_month_header`.

- [ ] **Step 5: Commit**

```bash
git add src/ui/right_panel.rs
git commit -m "feat: render inline calendar in right panel"
```

---

## Task 8: Todo list rendering with selected-item highlight

Add a second render test verifying todos appear and selection is highlighted.

**Files:**
- Modify: `src/ui/right_panel.rs` (tests only — the render code from Task 7 already handles this)

- [ ] **Step 1: Write tests**

Add to the `#[cfg(test)]` block in `src/ui/right_panel.rs`:

```rust
#[test]
fn render_shows_todo_text() {
    use crate::app::state::{AppState, Context, Focus, Overlay};
    use crate::config::Config;
    use crate::model::day::Document;
    use chrono::NaiveDate;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let doc = Document::new_for_date(date);
    let selectables = doc.selectables();
    let panel_todos = vec![PanelTodo {
        date,
        text: "buy milk".to_string(),
        todo_index: 0,
    }];
    let app = AppState {
        doc,
        date,
        notes_dir: PathBuf::from("/tmp"),
        config: Config::default(),
        context: Context::Notes,
        focus: Focus::Capture,
        selected: 0,
        status: String::new(),
        input: String::new(),
        overlay: Overlay::None,
        editing: None,
        should_quit: false,
        selectables,
        context_display: "context: Notes".to_string(),
        pending_delete: false,
        dates_with_notes: std::collections::BTreeSet::new(),
        right_panel_selected: 0,
        right_panel_scroll: 0,
        panel_todos,
    };

    let backend = TestBackend::new(30, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render(frame, frame.area(), &app))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("buy milk"), "expected 'buy milk', got: {}", content);
}

#[test]
fn render_selected_item_has_reversed_modifier() {
    use crate::app::state::{AppState, Context, Focus, Overlay};
    use crate::config::Config;
    use crate::model::day::Document;
    use chrono::NaiveDate;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;
    use std::path::PathBuf;

    let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let doc = Document::new_for_date(date);
    let selectables = doc.selectables();
    let panel_todos = vec![PanelTodo {
        date,
        text: "buy milk".to_string(),
        todo_index: 0,
    }];
    let app = AppState {
        doc,
        date,
        notes_dir: PathBuf::from("/tmp"),
        config: Config::default(),
        context: Context::Notes,
        focus: Focus::RightPanel, // panel is focused
        selected: 0,
        status: String::new(),
        input: String::new(),
        overlay: Overlay::None,
        editing: None,
        should_quit: false,
        selectables,
        context_display: "context: Notes".to_string(),
        pending_delete: false,
        dates_with_notes: std::collections::BTreeSet::new(),
        right_panel_selected: 0,
        right_panel_scroll: 0,
        panel_todos,
    };

    let backend = TestBackend::new(30, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render(frame, frame.area(), &app))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let has_reversed = buffer
        .content
        .iter()
        .any(|cell| cell.style().add_modifier.contains(Modifier::REVERSED));
    assert!(has_reversed, "expected REVERSED modifier for selected todo");
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -q right_panel
```

Expected: all right_panel tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/ui/right_panel.rs
git commit -m "test: add render tests for todo list and selection highlight"
```

---

## Task 9: Right panel keyboard input

Wire up Tab focus switching, arrow navigation, and Escape/Tab blur.

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write failing tests**

Add to `src/app/input.rs` tests:

```rust
#[test]
fn tab_in_capture_focuses_right_panel() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::FocusRightPanel)
    );
}

#[test]
fn tab_in_navigate_focuses_right_panel() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Navigate;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::FocusRightPanel)
    );
}

#[test]
fn tab_in_right_panel_blurs() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::RightPanelBlur)
    );
}

#[test]
fn esc_in_right_panel_blurs() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Esc)),
        Some(UiAction::RightPanelBlur)
    );
}

#[test]
fn right_panel_down_moves_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Down)),
        Some(UiAction::RightPanelDown)
    );
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char('j'))),
        Some(UiAction::RightPanelDown)
    );
}

#[test]
fn right_panel_up_moves_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Up)),
        Some(UiAction::RightPanelUp)
    );
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char('k'))),
        Some(UiAction::RightPanelUp)
    );
}

#[test]
fn right_panel_space_triggers_toggle() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char(' '))),
        Some(UiAction::RightPanelToggle)
    );
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Char('x'))),
        Some(UiAction::RightPanelToggle)
    );
}

#[test]
fn focus_right_panel_sets_focus_and_resets_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    state.right_panel_selected = 3;
    execute_action(&mut state, UiAction::FocusRightPanel).unwrap();
    assert_eq!(state.focus, Focus::RightPanel);
    assert_eq!(state.right_panel_selected, 0);
}

#[test]
fn right_panel_blur_returns_to_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    execute_action(&mut state, UiAction::RightPanelBlur).unwrap();
    assert_eq!(state.focus, Focus::Capture);
}

#[test]
fn right_panel_down_increments_selected() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    state.right_panel_selected = 0;
    // Need at least 2 items in panel_todos to move
    state.panel_todos = vec![
        crate::ui::right_panel::PanelTodo {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            text: "a".to_string(),
            todo_index: 0,
        },
        crate::ui::right_panel::PanelTodo {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            text: "b".to_string(),
            todo_index: 1,
        },
    ];
    execute_action(&mut state, UiAction::RightPanelDown).unwrap();
    assert_eq!(state.right_panel_selected, 1);
}

#[test]
fn right_panel_down_clamps_at_last() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    state.panel_todos = vec![crate::ui::right_panel::PanelTodo {
        date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
        text: "only".to_string(),
        todo_index: 0,
    }];
    state.right_panel_selected = 0;
    execute_action(&mut state, UiAction::RightPanelDown).unwrap();
    assert_eq!(state.right_panel_selected, 0);
}

#[test]
fn right_panel_up_decrements_selected() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    state.right_panel_selected = 1;
    state.panel_todos = vec![
        crate::ui::right_panel::PanelTodo {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            text: "a".to_string(),
            todo_index: 0,
        },
        crate::ui::right_panel::PanelTodo {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            text: "b".to_string(),
            todo_index: 1,
        },
    ];
    execute_action(&mut state, UiAction::RightPanelUp).unwrap();
    assert_eq!(state.right_panel_selected, 0);
}

#[test]
fn right_panel_up_clamps_at_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    state.right_panel_selected = 0;
    execute_action(&mut state, UiAction::RightPanelUp).unwrap();
    assert_eq!(state.right_panel_selected, 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -q 2>&1 | grep FAILED | head -10
```

Expected: compile errors for missing `UiAction` variants and missing fields.

- [ ] **Step 3: Add new UiAction variants**

In `src/app/input.rs`, add to the `UiAction` enum:

```rust
// Right panel
FocusRightPanel,
RightPanelUp,
RightPanelDown,
RightPanelToggle,
RightPanelBlur,
```

- [ ] **Step 4: Update key_to_action**

In `key_to_action` in `src/app/input.rs`:

1. **Add Tab handling** — insert before the Esc block (after Ctrl hotkeys):

```rust
// Tab — focus cycle
if key.code == KeyCode::Tab {
    return match state.focus {
        Focus::RightPanel => Some(UiAction::RightPanelBlur),
        _ => Some(UiAction::FocusRightPanel),
    };
}
```

2. **Update the Esc block** — replace the `Focus::RightPanel => None` stub:

```rust
Focus::RightPanel => Some(UiAction::RightPanelBlur),
```

3. **Replace the `Focus::RightPanel => None` stub** at the bottom mode-specific match:

```rust
Focus::RightPanel => match key.code {
    KeyCode::Down | KeyCode::Char('j') => Some(UiAction::RightPanelDown),
    KeyCode::Up | KeyCode::Char('k') => Some(UiAction::RightPanelUp),
    KeyCode::Char(' ') | KeyCode::Char('x') => Some(UiAction::RightPanelToggle),
    _ => None,
},
```

- [ ] **Step 5: Update execute_action**

Add match arms for the new actions to `execute_action` in `src/app/input.rs`:

```rust
UiAction::FocusRightPanel => {
    state.right_panel_selected = 0;
    state.focus = Focus::RightPanel;
}
UiAction::RightPanelBlur => {
    state.focus = Focus::Capture;
}
UiAction::RightPanelUp => {
    if state.right_panel_selected > 0 {
        state.right_panel_selected -= 1;
    }
}
UiAction::RightPanelDown => {
    let max = state.panel_todos.len().saturating_sub(1);
    if state.right_panel_selected < max {
        state.right_panel_selected += 1;
    }
}
UiAction::RightPanelToggle => {
    crate::app::actions::toggle_panel_todo(state)?;
}
```

- [ ] **Step 6: Run all tests**

```bash
cargo test -q
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: add right panel keyboard navigation (Tab, arrows, Esc, toggle)"
```

---

## Task 10: toggle_panel_todo action + go_to_date panel refresh

**Files:**
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Write failing tests**

Add to `src/app/actions.rs` tests:

```rust
#[test]
fn toggle_panel_todo_marks_past_todo_done() {
    let tmp = tempfile::tempdir().unwrap();
    let config = Config::default();
    let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let past = today - chrono::Duration::days(1);

    // Write a past day file with one open todo
    let past_path = crate::storage::path_for(tmp.path(), past, &config.date_format);
    std::fs::write(
        &past_path,
        "# 2026-06-04 (Thu)\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] past task\n",
    )
    .unwrap();

    let mut state =
        AppState::open_day(tmp.path().to_path_buf(), config.clone(), today).unwrap();
    state.focus = crate::app::state::Focus::RightPanel;
    state.right_panel_selected = 0;

    // panel_todos should include the past task
    assert_eq!(state.panel_todos.len(), 1, "should have 1 open todo");

    toggle_panel_todo(&mut state).unwrap();

    // past task should now be done in the file
    let saved = std::fs::read_to_string(&past_path).unwrap();
    assert!(
        saved.contains("- [x] past task"),
        "past task should be checked: {}",
        saved
    );
    // panel_todos should be empty now (todo is done)
    assert!(
        state.panel_todos.is_empty(),
        "panel_todos should be empty after toggle"
    );
}

#[test]
fn toggle_panel_todo_today_updates_app_doc() {
    let tmp = tempfile::tempdir().unwrap();
    let config = Config::default();
    let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();

    let mut state =
        AppState::open_day(tmp.path().to_path_buf(), config.clone(), today).unwrap();

    // Add a todo to today's doc
    dispatch(&mut state, Command::Todo("today task".to_string())).unwrap();

    // Refresh panel_todos manually to pick up the new todo
    state.panel_todos =
        crate::ui::right_panel::collect_panel_todos(&state.notes_dir, state.date, &state.config);

    state.focus = crate::app::state::Focus::RightPanel;
    state.right_panel_selected = 0;
    assert_eq!(state.panel_todos.len(), 1);

    toggle_panel_todo(&mut state).unwrap();

    // app.doc should reflect the toggle
    let text = state.doc.to_text();
    assert!(
        text.contains("- [x] today task"),
        "today doc should be updated: {}",
        text
    );
    // panel_todos should be empty
    assert!(state.panel_todos.is_empty());
}

#[test]
fn toggle_panel_todo_noop_when_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = crate::app::state::Focus::RightPanel;
    state.right_panel_selected = 0;
    // panel_todos is empty by default in test_state
    assert!(toggle_panel_todo(&mut state).is_ok());
}

#[test]
fn go_to_date_refreshes_panel_todos() {
    let tmp = tempfile::tempdir().unwrap();
    let config = Config::default();
    let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let yesterday = today - chrono::Duration::days(1);

    // Write yesterday's file with a todo
    let yest_path = crate::storage::path_for(tmp.path(), yesterday, &config.date_format);
    std::fs::write(
        &yest_path,
        "# 2026-06-04 (Thu)\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] old task\n",
    )
    .unwrap();

    let mut state =
        AppState::open_day(tmp.path().to_path_buf(), config.clone(), today).unwrap();
    let initial_count = state.panel_todos.len();

    // Navigate to a different date
    go_to_date(&mut state, yesterday).unwrap();

    // panel_todos should be refreshed (now viewing yesterday, old task is in window)
    assert_eq!(
        state.panel_todos.len(),
        1,
        "panel_todos should be refreshed after navigation, had {} before",
        initial_count
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -q toggle_panel_todo 2>&1 | head -10
```

Expected: compile error — `toggle_panel_todo` not found.

- [ ] **Step 3: Implement toggle_panel_todo**

Add to `src/app/actions.rs`:

```rust
pub fn toggle_panel_todo(state: &mut AppState) -> anyhow::Result<()> {
    let Some(todo) = state.panel_todos.get(state.right_panel_selected).cloned() else {
        return Ok(()); // empty panel — nothing to do
    };

    let path = crate::storage::path_for(&state.notes_dir, todo.date, &state.config.date_format);
    let text = std::fs::read_to_string(&path)?;
    let mut doc = crate::model::day::Document::from_text(&text);
    doc.toggle_todo(todo.todo_index)?;

    // Write back to disk
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, doc.to_text())?;
    std::fs::rename(&tmp_path, &path)?;

    // If this is today's date, also refresh app.doc so the left view stays in sync
    if todo.date == state.date {
        state.doc = doc;
        state.selectables = state.doc.selectables();
    }

    // Rebuild panel_todos (the toggled item is now done, drops off the list)
    state.panel_todos =
        crate::ui::right_panel::collect_panel_todos(&state.notes_dir, state.date, &state.config);

    // Clamp selection to new list length
    let new_len = state.panel_todos.len();
    if new_len == 0 {
        state.right_panel_selected = 0;
    } else {
        state.right_panel_selected = state.right_panel_selected.min(new_len - 1);
    }

    Ok(())
}
```

- [ ] **Step 4: Update go_to_date to refresh panel_todos**

In `src/app/actions.rs`, `go_to_date` currently calls `AppState::open_day` which already calls `collect_panel_todos`. But for clarity, verify `open_day` includes the call (it was added in Task 4). The `go_to_date` implementation replaces `*state` entirely with the result of `open_day`, so `panel_todos` is automatically refreshed. No change needed here.

Run the `go_to_date_refreshes_panel_todos` test to confirm:

```bash
cargo test -q go_to_date_refreshes_panel_todos
```

Expected: PASS.

- [ ] **Step 5: Run all tests**

```bash
cargo test -q
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat: add toggle_panel_todo and refresh panel_todos on date navigation"
```

---

## Final Verification

- [ ] **Build and run the app**

```bash
cargo build && cargo run -- --notes-dir /tmp/buff-test
```

- Verify the right panel appears on the right side
- Verify the calendar shows the current month with today highlighted
- Verify incomplete todos from the last 7 days appear in the panel
- Press `Tab` to focus the panel, `j`/`k` to navigate, `Space` to toggle a todo
- Press `Esc` or `Tab` to return to the main document
- Confirm the toggled todo disappears from the panel and (if today's) updates in the left document view

- [ ] **Run the full test suite one final time**

```bash
cargo test
```

Expected: all tests pass, no warnings about unused code.

- [ ] **Final commit**

```bash
git add -A
git commit -m "feat: right panel with inline calendar and todo list"
```
