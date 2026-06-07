# UI/UX Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve buff's TUI with deliberate keyboard modes, visual focus indicators, percentage-based pane sizing, and a restructured full-width header/footer layout.

**Architecture:** Implemented incrementally in four independent areas — keyboard (input.rs), config (config.rs), layout (layout.rs) — with each task producing working, tested, committed code before moving to the next. The layout restructure in Task 4 is the most invasive; Tasks 1–3 are lower risk and independently useful.

**Tech Stack:** Rust, ratatui 0.x, crossterm (KeyCode::BackTab for Shift+Tab), serde with custom Deserialize for PaneSize.

**Spec:** `docs/superpowers/specs/2026-06-07-ui-ux-improvements-design.md`

---

### Task 1: RemoveIndent — Shift+Tab un-indents current line in Capture mode

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write failing tests**

Add these tests to the `#[cfg(test)]` block in `src/app/input.rs`:

```rust
#[test]
fn backtab_in_capture_emits_remove_indent() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(&state, key), Some(UiAction::RemoveIndent));
}

#[test]
fn remove_indent_removes_arrow_from_line_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "->item".to_string();
    state.cursor_pos = 6;
    execute_action(&mut state, UiAction::RemoveIndent).unwrap();
    assert_eq!(state.input, "item");
}

#[test]
fn remove_indent_adjusts_cursor_pos_past_line_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "->item".to_string();
    state.cursor_pos = 6; // at end
    execute_action(&mut state, UiAction::RemoveIndent).unwrap();
    assert_eq!(state.cursor_pos, 4); // 6 - 2
}

#[test]
fn remove_indent_clamps_cursor_to_line_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "->item".to_string();
    state.cursor_pos = 1; // inside the "->"
    execute_action(&mut state, UiAction::RemoveIndent).unwrap();
    assert_eq!(state.cursor_pos, 0); // clamped to line start
}

#[test]
fn remove_indent_noop_when_no_arrow() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "item".to_string();
    state.cursor_pos = 2;
    execute_action(&mut state, UiAction::RemoveIndent).unwrap();
    assert_eq!(state.input, "item");
    assert_eq!(state.cursor_pos, 2);
}

#[test]
fn remove_indent_on_second_line_uses_line_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "parent\n->child".to_string();
    state.cursor_pos = 14; // at end of "->child"
    execute_action(&mut state, UiAction::RemoveIndent).unwrap();
    assert_eq!(state.input, "parent\nchild");
    assert_eq!(state.cursor_pos, 12); // 14 - 2
}

#[test]
fn remove_indent_cursor_at_line_start_not_adjusted() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "->item".to_string();
    state.cursor_pos = 0; // at line start
    execute_action(&mut state, UiAction::RemoveIndent).unwrap();
    assert_eq!(state.input, "item");
    assert_eq!(state.cursor_pos, 0); // at line start, no adjustment
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff -- remove_indent backtab_in_capture 2>&1 | head -30
```

Expected: compile errors (`RemoveIndent` not found) or FAIL.

- [ ] **Step 3: Add `UiAction::RemoveIndent` to the enum**

In `src/app/input.rs`, add to the `UiAction` enum after `PrependIndent`:

```rust
    PrependIndent,
    RemoveIndent,
    SubmitInput,
```

- [ ] **Step 4: Handle `KeyCode::BackTab` in `key_to_action` for Capture mode**

In `src/app/input.rs`, add a new section after the existing `Tab` block (section 4). Insert this block between the Tab handling and the Esc handling:

```rust
    // 4b. BackTab — un-indent in capture, reverse focus cycle elsewhere (handled in 4c below)
    if key.code == KeyCode::BackTab {
        if state.focus == Focus::Capture {
            return Some(UiAction::RemoveIndent);
        }
        // Other focus states handled in section 4c below after Esc
    }
```

Wait — BackTab for Navigate/Chat/RightPanel needs FocusNavigate which doesn't exist yet. Add only the Capture case for now; the rest come in Task 2. Add this block immediately after the existing Tab block (after line `};` that closes the Tab match):

```rust
    // 4b. BackTab in Capture — un-indent current line
    if key.code == KeyCode::BackTab && state.focus == Focus::Capture {
        return Some(UiAction::RemoveIndent);
    }
```

- [ ] **Step 5: Implement `execute_action` for `RemoveIndent`**

In `src/app/input.rs`, add to the `execute_action` match after the `PrependIndent` arm:

```rust
        UiAction::RemoveIndent => {
            let line_start = match state.input[..state.cursor_pos].rfind('\n') {
                Some(nl) => nl + 1,
                None => 0,
            };
            if state.input[line_start..].starts_with("->") {
                state.input.drain(line_start..line_start + 2);
                if state.cursor_pos > line_start {
                    state.cursor_pos = state.cursor_pos.saturating_sub(2).max(line_start);
                }
            }
        }
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test -p buff -- remove_indent backtab_in_capture 2>&1
```

Expected: all 7 new tests pass.

- [ ] **Step 7: Run full test suite**

```bash
cargo test -p buff 2>&1
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: add RemoveIndent action — Shift+Tab un-indents current line in capture mode"
```

---

### Task 2: FocusNavigate — complete Tab/BackTab focus cycle with wrap-around

**Files:**
- Modify: `src/app/input.rs`

The cycle left-to-right: `Navigate → Chat (if visible) → RightPanel → Navigate (wrap)`.
BackTab is the reverse. Tab from `RightPanel` currently goes to `RightPanelBlur` (→ Capture); this task changes it to `FocusNavigate` (→ Navigate).

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block in `src/app/input.rs`:

```rust
#[test]
fn tab_in_right_panel_wraps_to_navigate() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::FocusNavigate)
    );
}

#[test]
fn backtab_in_navigate_wraps_to_right_panel() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Navigate;
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(&state, key), Some(UiAction::FocusRightPanel));
}

#[test]
fn backtab_in_chat_goes_to_navigate() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Chat;
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(&state, key), Some(UiAction::FocusNavigate));
}

#[test]
fn backtab_in_right_panel_goes_to_chat_when_visible() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    state.chat.visible = true;
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(&state, key), Some(UiAction::FocusChat));
}

#[test]
fn backtab_in_right_panel_goes_to_navigate_when_chat_hidden() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    state.chat.visible = false;
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(&state, key), Some(UiAction::FocusNavigate));
}

#[test]
fn focus_navigate_sets_focus_to_navigate() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    execute_action(&mut state, UiAction::FocusNavigate).unwrap();
    assert_eq!(state.focus, Focus::Navigate);
}

#[test]
fn focus_navigate_clears_pending_delete() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::RightPanel;
    state.pending_delete = true;
    execute_action(&mut state, UiAction::FocusNavigate).unwrap();
    assert!(!state.pending_delete);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff -- backtab_in tab_in_right_panel_wraps focus_navigate 2>&1 | head -30
```

Expected: compile errors (`FocusNavigate` not found) or test failures.

- [ ] **Step 3: Add `UiAction::FocusNavigate` to the enum**

In `src/app/input.rs`, add to the `UiAction` enum in the Navigate section:

```rust
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
    FocusNavigate,
```

- [ ] **Step 4: Update Tab handling for RightPanel and add full BackTab block**

Replace the existing Tab handling block in `key_to_action` (the block starting `// 4. Tab — focus cycle`):

```rust
    // 4. Tab — focus cycle (or indent in capture mode)
    if key.code == KeyCode::Tab {
        return match state.focus {
            Focus::Capture => Some(UiAction::TypeIndent),
            Focus::Navigate => {
                if state.chat.visible {
                    Some(UiAction::FocusChat)
                } else {
                    Some(UiAction::FocusRightPanel)
                }
            }
            Focus::Chat => Some(UiAction::FocusRightPanel),
            Focus::RightPanel => Some(UiAction::FocusNavigate),
        };
    }

    // 4b. BackTab — reverse focus cycle (or un-indent in capture mode)
    if key.code == KeyCode::BackTab {
        return match state.focus {
            Focus::Capture => Some(UiAction::RemoveIndent),
            Focus::Navigate => Some(UiAction::FocusRightPanel),
            Focus::Chat => Some(UiAction::FocusNavigate),
            Focus::RightPanel => {
                if state.chat.visible {
                    Some(UiAction::FocusChat)
                } else {
                    Some(UiAction::FocusNavigate)
                }
            }
        };
    }
```

Note: this replaces the temporary `BackTab` block added in Task 1.

- [ ] **Step 5: Implement `execute_action` for `FocusNavigate`**

In `src/app/input.rs`, add to `execute_action` after the `SwitchToCapture` arm:

```rust
        UiAction::FocusNavigate => {
            state.pending_delete = false;
            state.focus = Focus::Navigate;
        }
```

- [ ] **Step 6: Run new tests**

```bash
cargo test -p buff -- backtab_in tab_in_right_panel_wraps focus_navigate 2>&1
```

Expected: all 7 new tests pass.

- [ ] **Step 7: Run full test suite**

```bash
cargo test -p buff 2>&1
```

Expected: all tests pass. Note: the existing `tab_in_right_panel_blurs` test will now fail because Tab from RightPanel now goes to `FocusNavigate` instead of `RightPanelBlur`. Update that test:

```rust
#[test]
fn tab_in_right_panel_wraps_to_navigate() {
    // (already added above — delete the old tab_in_right_panel_blurs test)
```

Find and delete the test `tab_in_right_panel_blurs` (which expected `RightPanelBlur`). It is replaced by `tab_in_right_panel_wraps_to_navigate` added in Step 1.

- [ ] **Step 8: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: add FocusNavigate action and complete wrapping Tab/BackTab focus cycle"
```

---

### Task 3: PaneSize config type — percentage-based panel sizing

**Files:**
- Modify: `src/config.rs`

`chat_width` is removed. `panel_width` becomes `PaneSize` which accepts either an integer (column count) or a quoted `"25%"` string. Default behavior is unchanged (`PaneSize::Columns(30)`).

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block in `src/config.rs`:

```rust
#[test]
fn panel_width_default_is_columns_30() {
    let config = Config::default();
    assert_eq!(config.panel_width, PaneSize::Columns(30));
}

#[test]
fn panel_width_parses_as_integer() {
    let toml = r#"panel_width = 40"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.panel_width, PaneSize::Columns(40));
}

#[test]
fn panel_width_parses_as_percentage_string() {
    let toml = r#"panel_width = "25%""#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.panel_width, PaneSize::Percent(25));
}

#[test]
fn panel_width_percentage_100_is_valid() {
    let toml = r#"panel_width = "100%""#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.panel_width, PaneSize::Percent(100));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff -- panel_width_default_is_columns panel_width_parses 2>&1 | head -20
```

Expected: compile errors (`PaneSize` not found).

- [ ] **Step 3: Add `PaneSize` type with custom serde**

Add this to `src/config.rs` before the `Config` struct:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneSize {
    Columns(u16),
    Percent(u16),
}

impl<'de> serde::Deserialize<'de> for PaneSize {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PaneSizeVisitor;
        impl<'de> serde::de::Visitor<'de> for PaneSizeVisitor {
            type Value = PaneSize;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "an integer column count or a percentage string like \"25%\"")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<PaneSize, E> {
                Ok(PaneSize::Columns(v as u16))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<PaneSize, E> {
                Ok(PaneSize::Columns(v as u16))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<PaneSize, E> {
                if let Some(digits) = v.strip_suffix('%') {
                    digits.parse::<u16>()
                        .map(PaneSize::Percent)
                        .map_err(|_| E::custom(format!("invalid percentage: {}", v)))
                } else {
                    Err(E::custom(format!("expected integer or \"N%\" string, got: {}", v)))
                }
            }
        }
        d.deserialize_any(PaneSizeVisitor)
    }
}
```

- [ ] **Step 4: Update `Config` struct — replace `panel_width`, remove `chat_width`**

Replace the existing `Config` struct and its `Default` impl:

```rust
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub notes_dir: Option<String>,
    pub timestamp_entries: bool,
    pub week_starts_on: WeekStart,
    pub date_format: String,
    pub panel_width: PaneSize,
    pub todo_lookback_days: u16,
    pub capture_height: u16,
    pub llm_base_url: String,
    pub llm_model: String,
    pub llm_system_prompt: String,
    pub chat_visible: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            notes_dir: None,
            timestamp_entries: false,
            week_starts_on: WeekStart::Sunday,
            date_format: "%Y-%m-%d-%a".to_string(),
            panel_width: PaneSize::Columns(30),
            todo_lookback_days: 7,
            capture_height: 5,
            llm_base_url: "http://localhost:1234/v1".to_string(),
            llm_model: "google/gemma-4-12b-qat".to_string(),
            llm_system_prompt: String::new(),
            chat_visible: true,
        }
    }
}
```

- [ ] **Step 5: Fix existing config tests that referenced `chat_width` or the old `panel_width` type**

In `src/config.rs` tests, update `panel_width_default_is_30` — replace it with the new test from Step 1 (it's now `panel_width_default_is_columns_30`). Delete the old one.

Update `parse_panel_fields_from_toml` to use `PaneSize::Columns`:

```rust
#[test]
fn parse_panel_fields_from_toml() {
    let toml = r#"
        panel_width = 40
        todo_lookback_days = 14
    "#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.panel_width, PaneSize::Columns(40));
    assert_eq!(config.todo_lookback_days, 14);
}
```

Update `panel_fields_use_defaults_when_absent`:

```rust
#[test]
fn panel_fields_use_defaults_when_absent() {
    let toml = r#"timestamp_entries = true"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.panel_width, PaneSize::Columns(30));
    assert_eq!(config.todo_lookback_days, 7);
}
```

Replace `llm_and_chat_defaults` (removes the buggy `chat_width == 40` assertion):

```rust
#[test]
fn llm_and_chat_defaults() {
    let config = Config::default();
    assert_eq!(config.llm_base_url, "http://localhost:1234/v1");
    assert_eq!(config.llm_model, "google/gemma-4-12b-qat");
    assert_eq!(config.llm_system_prompt, "");
    assert!(config.chat_visible);
}
```

Replace `parse_llm_and_chat_fields_from_toml` (removes `chat_width`):

```rust
#[test]
fn parse_llm_and_chat_fields_from_toml() {
    let toml = r#"
        llm_base_url = "http://127.0.0.1:9999/v1"
        llm_model = "my-model"
        llm_system_prompt = "be terse"
        chat_visible = false
    "#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.llm_base_url, "http://127.0.0.1:9999/v1");
    assert_eq!(config.llm_model, "my-model");
    assert_eq!(config.llm_system_prompt, "be terse");
    assert!(!config.chat_visible);
}
```

- [ ] **Step 6: Run new tests**

```bash
cargo test -p buff -- panel_width 2>&1
```

Expected: all 4 new PaneSize tests pass.

- [ ] **Step 7: Run full test suite**

```bash
cargo test -p buff 2>&1
```

Expected: all tests pass. The only code that references `config.chat_width` is `src/ui/layout.rs` (the `Constraint::Length(app.config.chat_width)` line in the current render function). Task 4 replaces `layout.rs` entirely, so after Task 4 that reference disappears. At the Task 3 stage, the compiler will flag this reference — fix it by temporarily removing the chat constraint from the layout constraints vector and making the outer split two-element (notes+panel only), as Task 4 will replace the whole function anyway. Alternatively, do Task 3 and Task 4 in a single commit if you prefer to avoid the intermediate broken state.

- [ ] **Step 8: Commit**

```bash
git add src/config.rs
git commit -m "feat: add PaneSize type for panel_width; remove chat_width from config"
```

---

### Task 4: Layout restructuring — global header/footer, borders, 50/50 split

**Files:**
- Modify: `src/ui/layout.rs`

This replaces the layout's left-column structure with a `main_area` (header + content_row + status + input) plus an independent full-height `panel_area`. Border blocks are added for the three content panes. Notes and chat split the content row 50/50 when chat is visible.

- [ ] **Step 1: Write failing tests for the new behaviors**

Add to the `#[cfg(test)]` block in `src/ui/layout.rs`:

```rust
#[test]
fn notes_pane_has_cyan_border_in_capture_mode() {
    use ratatui::style::Color;
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let app = test_app(doc, Focus::Capture, 0);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let buffer = terminal.backend().buffer();
    let has_cyan_border = buffer.content.iter().any(|cell| {
        cell.style().fg == Some(Color::Cyan)
            && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
    });
    assert!(has_cyan_border, "expected cyan border for focused notes pane");
}

#[test]
fn right_panel_has_dark_border_when_notes_focused() {
    use ratatui::style::Color;
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let app = test_app(doc, Focus::Capture, 0);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let buffer = terminal.backend().buffer();
    let has_dark_border = buffer.content.iter().any(|cell| {
        cell.style().fg == Some(Color::DarkGray)
            && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
    });
    assert!(has_dark_border, "expected dark border for unfocused right panel");
}

#[test]
fn chat_pane_has_cyan_border_when_focused() {
    use ratatui::style::Color;
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::Chat, 0);
    app.chat.visible = true;
    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let buffer = terminal.backend().buffer();
    let has_cyan_border = buffer.content.iter().any(|cell| {
        cell.style().fg == Some(Color::Cyan)
            && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
    });
    assert!(has_cyan_border, "expected cyan border for focused chat pane");
}

#[test]
fn right_panel_has_full_height_independently() {
    // With chat visible, right panel should still span full height.
    // Verify it renders content (calendar header) in the same location
    // regardless of main_area layout.
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::Capture, 0);
    app.chat.visible = true;
    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("June 2026"), "calendar header should be present with chat visible");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff -- notes_pane_has_cyan right_panel_has_dark chat_pane_has_cyan right_panel_has_full 2>&1 | head -30
```

Expected: all 4 tests fail (no borders exist yet).

- [ ] **Step 3: Replace the `render` function in `src/ui/layout.rs`**

Replace the entire `render` function and add the `pane_size_to_constraint` helper at the bottom of the non-test code. The new file content (keeping existing `use` imports and `#[cfg(test)]` block) is:

```rust
use crate::app::state::{AppState, Focus, Overlay};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

const FOCUSED_BORDER: Color = Color::Cyan;
const UNFOCUSED_BORDER: Color = Color::DarkGray;

pub fn render(frame: &mut ratatui::Frame, app: &AppState) {
    // Outer horizontal split: main (notes + chat) | right panel (full height)
    let panel_constraint = pane_size_to_constraint(&app.config.panel_width);
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), panel_constraint])
        .split(frame.area());
    let main_area = outer[0];
    let panel_area = outer[1];

    // main_area vertical split: header | content_row | status | input
    let input_line_count = app.input.split('\n').count().max(1) as u16;
    let input_height = (input_line_count + 2).clamp(app.config.capture_height, 12);
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),             // header
            Constraint::Min(0),                // content_row
            Constraint::Length(1),             // status
            Constraint::Length(input_height),  // input (footer)
        ])
        .split(main_area);
    let header_area = main_chunks[0];
    let content_row = main_chunks[1];
    let status_area = main_chunks[2];
    let input_area = main_chunks[3];

    // Header: buff ASCII art (left) + date/context (right), spans full main_area width
    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)])
        .split(header_area);
    let ascii_art = r#"_             __  __
| |__  _   _  / _|/ _|
| '_ \| | | || |_ | |_
| |_) | |_| ||  _||  _|
|_.__/ \__,_||_|  |_|"#;
    let art_widget = ratatui::widgets::Paragraph::new(ascii_art);
    frame.render_widget(art_widget, title_chunks[0]);
    let meta = format!("{}\n{}", app.date.format("%Y-%m-%d (%a)"), app.context_display);
    let meta_widget = ratatui::widgets::Paragraph::new(meta);
    frame.render_widget(meta_widget, title_chunks[1]);

    // content_row horizontal split: notes | chat (optional, 50/50)
    let notes_focused = matches!(app.focus, Focus::Capture | Focus::Navigate);
    let chat_focused = app.focus == Focus::Chat;
    let panel_focused = app.focus == Focus::RightPanel;

    let (notes_area, chat_area_opt) = if app.chat.visible {
        let row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_row);
        (row[0], Some(row[1]))
    } else {
        (content_row, None)
    };

    // Notes pane with border
    let notes_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if notes_focused { FOCUSED_BORDER } else { UNFOCUSED_BORDER }));
    let notes_inner = notes_block.inner(notes_area);
    frame.render_widget(notes_block, notes_area);
    super::document::render(frame, app, notes_inner);

    // Status bar (footer chrome, no border)
    super::capture::render_status(frame, app, status_area);

    // Input box (footer, spans full main_area width; render_input draws its own Block)
    super::capture::render_input(frame, app, input_area);

    // Chat pane with border (when visible)
    if let Some(chat_area) = chat_area_opt {
        let chat_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if chat_focused { FOCUSED_BORDER } else { UNFOCUSED_BORDER }));
        let chat_inner = chat_block.inner(chat_area);
        frame.render_widget(chat_block, chat_area);
        super::chat_panel::render(frame, chat_inner, app);
    }

    // Right panel with border — full terminal height
    let panel_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if panel_focused { FOCUSED_BORDER } else { UNFOCUSED_BORDER }));
    let panel_inner = panel_block.inner(panel_area);
    frame.render_widget(panel_block, panel_area);
    super::right_panel::render(frame, panel_inner, app);

    // Overlays (always on top of full frame)
    if app.overlay == Overlay::Help {
        super::help::render(frame, frame.area());
    }
}

fn pane_size_to_constraint(size: &crate::config::PaneSize) -> Constraint {
    match size {
        crate::config::PaneSize::Columns(n) => Constraint::Length(*n),
        crate::config::PaneSize::Percent(p) => Constraint::Percentage(*p),
    }
}
```

- [ ] **Step 4: Run the new tests**

```bash
cargo test -p buff -- notes_pane_has_cyan right_panel_has_dark chat_pane_has_cyan right_panel_has_full 2>&1
```

Expected: all 4 new tests pass.

- [ ] **Step 5: Run the full test suite**

```bash
cargo test -p buff 2>&1
```

Expected: all tests pass. If any existing layout test fails, investigate which assertion broke and whether it is testing behavior that was preserved (content visible) vs. incidental details (exact cell positions). Update assertions if the test was checking position-specific details rather than behavior.

Most likely breakage: `chat_panel_renders_when_visible` — verify "paneltext" is still found with the new 50/50 split on a 120-column terminal (chat inner area ≈ 28 cols after panel=30, border=2; "paneltext" is 9 chars, fits).

- [ ] **Step 6: Commit**

```bash
git add src/ui/layout.rs
git commit -m "feat: restructure layout with global header/footer, pane borders, and 50/50 notes/chat split"
```

---

## Verification

After all four tasks are committed, run the full suite one final time:

```bash
cargo test -p buff 2>&1
```

Expected: all tests pass with no warnings.

The implemented features are:
- **ESC** — documented deliberate toggle: Capture↔Navigate, Chat/RightPanel→Capture
- **Shift+Tab in Capture** — un-indents current line (removes leading `->`)
- **Tab/Shift+Tab in Navigate** — wrapping focus cycle: Notes→Chat→Panel→Notes / reverse
- **Focus borders** — cyan border on active pane, dark gray on inactive panes
- **`panel_width` as `%`** — e.g. `panel_width = "25%"` in config.toml
- **Global header** — buff logo + date + context span notes+chat width
- **Global footer** — input box spans notes+chat width
- **Right panel full height** — independent of main area layout
- **50/50 notes/chat split** — Ctrl+L hides chat to give notes full remaining width
