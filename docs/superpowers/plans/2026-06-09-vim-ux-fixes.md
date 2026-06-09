# Vim UX Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix three vim-mode usability issues: Esc navigation consistency, visible cursor-line highlight, and cursor visibility/shape in insert mode.

**Architecture:** Targeted fixes at each call site — key bindings in `input/mod.rs`, theme color in `config.rs`+`theme.rs`, cursor line background in `document.rs`, cursor position guard in `capture.rs`, and cursor shape commands emitted from the main loop in `main.rs`.

**Tech Stack:** Rust, ratatui 0.29, crossterm (via `ratatui::crossterm`)

---

## File Map

| File | Change |
|---|---|
| `src/app/input/mod.rs` | Esc in VimNormal → `SwitchToCapture`; Tab in VimNormal/VimInsert → `FocusRightPanel`; BackTab in RightPanel → `FocusVimNormal` unconditionally |
| `src/config.rs` | Add `vim_cursor_line: Option<String>` to `ThemeOverrides` |
| `src/ui/theme.rs` | Add `vim_cursor_line: Color` to `Theme`, set light/dark defaults, add `apply!(vim_cursor_line)` to `resolve_theme` |
| `src/ui/document.rs` | Replace `theme.notes_panel_bg` with `theme.vim_cursor_line` for cursor-line background |
| `src/ui/capture.rs` | Guard `frame.set_cursor_position` behind `if app.focus == Focus::Capture` |
| `src/main.rs` | Emit `SetCursorStyle` crossterm command after each `terminal.draw` call |

Tests live inline in their source files:
- Key binding tests: `src/app/input/mod.rs` `#[cfg(test)]` block
- Render tests: `src/ui/layout.rs` `#[cfg(test)]` block
- Theme tests: `src/ui/theme.rs` `#[cfg(test)]` block

---

## Task 1: Key Bindings

**Files:**
- Modify: `src/app/input/mod.rs`

### Step 1.1 — Establish baseline

- [ ] Run the full test suite to confirm current state:

```bash
cargo test 2>&1 | tail -5
```

Expected: all tests pass. If any are already failing, note them — they are pre-existing failures and not caused by our changes.

### Step 1.2 — Update existing tests to reflect desired behaviour

- [ ] In `src/app/input/mod.rs`, find the test `vimnormal_tab_switches_to_capture` (around line 1109) and change the assertion:

```rust
#[test]
fn vimnormal_tab_switches_to_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Tab)),
        Some(UiAction::FocusRightPanel)   // was SwitchToCapture
    );
}
```

- [ ] Find the test `backtab_in_right_panel_goes_to_chat_when_visible` (around line 971) and change the assertion:

```rust
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
    assert_eq!(key_to_action(&state, key), Some(UiAction::FocusVimNormal));  // was FocusChat
}
```

### Step 1.3 — Add new test for Esc in VimNormal

- [ ] Add this test to the `#[cfg(test)]` block in `src/app/input/mod.rs`, after the existing `vimnormal_esc_is_noop` test:

```rust
#[test]
fn vimnormal_esc_switches_to_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::VimNormal;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Esc)),
        Some(UiAction::SwitchToCapture)
    );
}
```

### Step 1.4 — Run tests to confirm they fail

- [ ] Run:

```bash
cargo test --lib 2>&1 | grep -E "FAILED|test result"
```

Expected: the three changed/new tests fail. All others pass.

### Step 1.5 — Update the Tab handler

- [ ] In `src/app/input/mod.rs`, find the Tab handler block (around line 218):

```rust
// 4. Tab — focus cycle (or indent in capture mode)
if key.code == KeyCode::Tab {
    return match state.focus {
        Focus::Capture => Some(UiAction::TypeIndent),
        Focus::VimNormal | Focus::VimInsert => Some(UiAction::FocusRightPanel),
        Focus::Chat => Some(UiAction::FocusRightPanel),
        Focus::RightPanel => Some(UiAction::FocusVimNormal),
    };
}
```

The `VimNormal | VimInsert` arm changes from `SwitchToCapture` to `FocusRightPanel`. `Chat` and `RightPanel` arms are unchanged.

### Step 1.6 — Update the Esc handler

- [ ] In `src/app/input/mod.rs`, find the Esc handler block (around line 244):

```rust
// 5. Esc handling (context-dependent)
if key.code == KeyCode::Esc {
    return match state.focus {
        Focus::Capture => {
            if state.editing.is_some() {
                Some(UiAction::CancelEdit)
            } else {
                Some(UiAction::ExitCaptureMode)
            }
        }
        Focus::VimNormal => Some(UiAction::SwitchToCapture),
        Focus::VimInsert => Some(UiAction::VimExitInsert),
        Focus::RightPanel => Some(UiAction::RightPanelBlur),
        Focus::Chat => Some(UiAction::ChatBlur),
    };
}
```

The `VimNormal` arm changes from `None` to `Some(UiAction::SwitchToCapture)`.

### Step 1.7 — Update the BackTab handler

- [ ] In `src/app/input/mod.rs`, find the BackTab handler block (around line 228):

```rust
// 4b. BackTab — reverse focus cycle (or un-indent in capture mode)
if key.code == KeyCode::BackTab {
    return match state.focus {
        Focus::Capture => Some(UiAction::RemoveIndent),
        Focus::VimNormal | Focus::VimInsert => Some(UiAction::FocusRightPanel),
        Focus::Chat => Some(UiAction::FocusVimNormal),
        Focus::RightPanel => Some(UiAction::FocusVimNormal),
    };
}
```

The `RightPanel` arm simplifies from the `if state.chat.visible { ... } else { ... }` conditional to unconditionally `FocusVimNormal`.

### Step 1.8 — Run tests to confirm they pass

- [ ] Run:

```bash
cargo test --lib 2>&1 | grep -E "FAILED|test result"
```

Expected: all tests pass.

### Step 1.9 — Commit

- [ ] Commit:

```bash
git add src/app/input/mod.rs
git commit -m "feat: remap Esc→Capture and Tab→RightPanel in vim normal mode"
```

---

## Task 2: vim_cursor_line Theme Colour

**Files:**
- Modify: `src/config.rs`
- Modify: `src/ui/theme.rs`

### Step 2.1 — Add field to ThemeOverrides in config.rs

- [ ] In `src/config.rs`, add `vim_cursor_line` to the `ThemeOverrides` struct (after `todo_overdue`):

```rust
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct ThemeOverrides {
    pub heading1: Option<String>,
    pub heading2: Option<String>,
    pub heading3: Option<String>,
    pub heading4: Option<String>,
    pub heading5: Option<String>,
    pub heading6: Option<String>,
    pub border_focused: Option<String>,
    pub border_unfocused: Option<String>,
    pub notes_panel_bg: Option<String>,
    pub panel_bg: Option<String>,
    pub chat_panel_bg: Option<String>,
    pub quote_marker: Option<String>,
    pub code: Option<String>,
    pub todo_done: Option<String>,
    pub todo_overdue: Option<String>,
    pub vim_cursor_line: Option<String>,
}
```

### Step 2.2 — Add tests for the new field

- [ ] In `src/ui/theme.rs`, add these two tests to the `#[cfg(test)]` block:

```rust
#[test]
fn light_theme_has_vim_cursor_line() {
    let theme = light();
    assert_eq!(theme.vim_cursor_line, Color::Rgb(219, 234, 254));
}

#[test]
fn dark_theme_has_vim_cursor_line() {
    let theme = dark();
    assert_eq!(theme.vim_cursor_line, Color::Rgb(40, 44, 52));
}
```

### Step 2.3 — Run tests to confirm they fail

- [ ] Run:

```bash
cargo test --lib 2>&1 | grep -E "FAILED|test result|error"
```

Expected: compile error because `vim_cursor_line` doesn't exist on `Theme` yet.

### Step 2.4 — Add field to Theme struct and defaults

- [ ] In `src/ui/theme.rs`, add `vim_cursor_line: Color` to the `Theme` struct (after `todo_overdue`):

```rust
pub struct Theme {
    pub heading1: Color,
    pub heading2: Color,
    pub heading3: Color,
    pub heading4: Color,
    pub heading5: Color,
    pub heading6: Color,
    pub border_focused: Color,
    pub border_unfocused: Color,
    pub notes_panel_bg: Color,
    pub panel_bg: Color,
    pub chat_panel_bg: Color,
    pub quote_marker: Color,
    pub code: Color,
    pub todo_done: Color,
    pub todo_overdue: Color,
    pub vim_cursor_line: Color,
}
```

- [ ] In the `light()` function, add the default value (after `todo_overdue`):

```rust
pub fn light() -> Theme {
    Theme {
        heading1: Color::Black,
        heading2: Color::Rgb(2, 119, 189),
        heading3: Color::Rgb(230, 81, 0),
        heading4: Color::Rgb(106, 27, 154),
        heading5: Color::Rgb(46, 125, 50),
        heading6: Color::DarkGray,
        border_focused: Color::Rgb(2, 119, 189),
        border_unfocused: Color::DarkGray,
        notes_panel_bg: Color::Reset,
        panel_bg: Color::Rgb(221, 232, 245),
        chat_panel_bg: Color::Rgb(230, 230, 240),
        quote_marker: Color::Rgb(123, 31, 162),
        code: Color::DarkGray,
        todo_done: Color::Green,
        todo_overdue: Color::Red,
        vim_cursor_line: Color::Rgb(219, 234, 254),
    }
}
```

- [ ] In the `dark()` function, add the default value (after `todo_overdue`):

```rust
pub fn dark() -> Theme {
    Theme {
        heading1: Color::White,
        heading2: Color::Cyan,
        heading3: Color::Yellow,
        heading4: Color::Magenta,
        heading5: Color::Green,
        heading6: Color::Gray,
        border_focused: Color::Cyan,
        border_unfocused: Color::DarkGray,
        notes_panel_bg: Color::Reset,
        panel_bg: Color::Rgb(220, 220, 220),
        chat_panel_bg: Color::Rgb(230, 230, 240),
        quote_marker: Color::Magenta,
        code: Color::DarkGray,
        todo_done: Color::Green,
        todo_overdue: Color::Red,
        vim_cursor_line: Color::Rgb(40, 44, 52),
    }
}
```

- [ ] In `resolve_theme`, add `apply!(vim_cursor_line);` after `apply!(todo_overdue);`:

```rust
apply!(todo_done);
apply!(todo_overdue);
apply!(vim_cursor_line);
```

### Step 2.5 — Run tests to confirm they pass

- [ ] Run:

```bash
cargo test --lib 2>&1 | grep -E "FAILED|test result"
```

Expected: all tests pass.

### Step 2.6 — Commit

- [ ] Commit:

```bash
git add src/config.rs src/ui/theme.rs
git commit -m "feat: add vim_cursor_line theme colour"
```

---

## Task 3: Cursor Line Highlight

**Files:**
- Modify: `src/ui/document.rs`
- Modify: `src/ui/layout.rs` (test only)

### Step 3.1 — Write the failing render test

- [ ] In `src/ui/layout.rs`, add this test to the `#[cfg(test)]` block:

```rust
#[test]
fn render_vim_normal_cursor_line_uses_vim_cursor_line_bg() {
    use ratatui::style::Color;
    let doc = Document::from_text("cursor line\nother line\n");
    let mut app = test_app(doc, Focus::VimNormal, 0);
    app.vim.cursor_line = 0;

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();

    let buffer = terminal.backend().buffer();
    // light theme vim_cursor_line = Color::Rgb(219, 234, 254)
    let has_highlight = buffer.content.iter().any(|cell| {
        cell.style().bg == Some(Color::Rgb(219, 234, 254))
    });
    assert!(
        has_highlight,
        "expected vim_cursor_line background on the cursor line, got none"
    );
}
```

### Step 3.2 — Run the test to confirm it fails

- [ ] Run:

```bash
cargo test render_vim_normal_cursor_line_uses_vim_cursor_line_bg 2>&1
```

Expected: FAIL — the cursor line currently uses `Color::Reset` so no cell will have `Rgb(219, 234, 254)`.

### Step 3.3 — Update document.rs to use vim_cursor_line

- [ ] In `src/ui/document.rs`, find the cursor-line background block (around line 24) and change the background:

```rust
if vim_active && i == cursor_line {
    in_code = false; // reset; raw line shown anyway
    let bg_style = Style::default()
        .bg(theme.vim_cursor_line);
    return Line::from(Span::styled(line.as_str(), bg_style));
}
```

The only change is `.bg(theme.notes_panel_bg)` → `.bg(theme.vim_cursor_line)`.

### Step 3.4 — Run tests to confirm they pass

- [ ] Run:

```bash
cargo test --lib 2>&1 | grep -E "FAILED|test result"
```

Expected: all tests pass. If `render_navigate_mode` fails (it checks `Modifier::REVERSED`, which ratatui's TestBackend applies to the cursor-position cell independently of the background colour), it is a test that is no longer meaningful after the vim render refactor. Update it:

```rust
#[test]
fn render_navigate_mode() {
    let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    doc.add_todo("First", None);
    doc.add_todo("Second", None);

    let mut app = test_app(doc, Focus::VimNormal, 1);
    app.vim.cursor_line = 0;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render(frame, &app, &test_theme());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    // Cursor line should have the vim_cursor_line highlight background
    let has_highlight = buffer.content.iter().any(|cell| {
        cell.style().bg == Some(ratatui::style::Color::Rgb(219, 234, 254))
    });
    assert!(
        has_highlight,
        "Expected cursor line highlight in vim normal mode"
    );
}
```

Re-run after any update:

```bash
cargo test --lib 2>&1 | grep -E "FAILED|test result"
```

### Step 3.5 — Commit

- [ ] Commit:

```bash
git add src/ui/document.rs src/ui/layout.rs
git commit -m "feat: highlight cursor line in vim modes"
```

---

## Task 4: Fix Cursor Position Bug

**Files:**
- Modify: `src/ui/capture.rs`

The bug: `render_input` calls `frame.set_cursor_position` unconditionally, which overwrites the vim cursor set by `document::render` on every frame.

### Step 4.1 — Guard set_cursor_position in render_input

- [ ] In `src/ui/capture.rs`, find the `render_input` function. The cursor-setting block at the end (around line 56–83) currently ends with an unconditional `frame.set_cursor_position(...)`. Add a focus guard around it.

The complete updated `render_input` function:

```rust
pub fn render_input(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    use ratatui::text::Text;
    use crate::app::state::Focus;

    let (title, prefix) = if app.editing.is_some() {
        ("Edit", "Edit: › ")
    } else {
        ("Capture", "› ")
    };
    let block = Block::default().title(title).borders(Borders::ALL);

    let input_lines: Vec<&str> = app.input.split('\n').collect();
    let rendered: Vec<Line> = input_lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                Line::from(format!("{}{}", prefix, l))
            } else {
                Line::from((*l).to_string())
            }
        })
        .collect();

    let inner_height = area.height.saturating_sub(2);
    let overflow = input_lines.len().saturating_sub(inner_height as usize);

    let paragraph = Paragraph::new(Text::from(rendered))
        .block(block)
        .scroll((overflow as u16, 0));
    frame.render_widget(paragraph, area);

    // Only place the terminal cursor in the input box when Capture is active.
    // In vim modes, document::render is responsible for cursor placement.
    if app.focus == Focus::Capture {
        // Compute cursor (row, col) from cursor_pos byte offset
        let mut remaining = app.cursor_pos;
        let mut cursor_row = 0;
        let mut cursor_col = 0usize; // character count within the line
        for (i, line) in input_lines.iter().enumerate() {
            let line_bytes = line.len();
            if remaining <= line_bytes {
                cursor_col = line[..remaining].chars().count();
                cursor_row = i;
                break;
            }
            remaining -= line_bytes + 1; // +1 for the '\n' separator
            // Fallback if cursor_pos == input.len() and input ends with '\n'
            cursor_row = i + 1;
            cursor_col = 0;
        }

        let col = if cursor_row == 0 {
            prefix.chars().count() + cursor_col
        } else {
            cursor_col
        };

        let inner_x = area.x + 1;
        let inner_y = area.y + 1;
        frame.set_cursor_position(ratatui::layout::Position::new(
            inner_x + col as u16,
            inner_y + (cursor_row.saturating_sub(overflow)) as u16,
        ));
    }
}
```

### Step 4.2 — Run tests

- [ ] Run:

```bash
cargo test --lib 2>&1 | grep -E "FAILED|test result"
```

Expected: all tests pass.

### Step 4.3 — Commit

- [ ] Commit:

```bash
git add src/ui/capture.rs
git commit -m "fix: only set cursor position in input box when Capture is focused"
```

---

## Task 5: Cursor Shape

**Files:**
- Modify: `src/main.rs`

### Step 5.1 — Add SetCursorStyle call after terminal.draw

- [ ] In `src/main.rs`, add two `use` imports at the top of the file (after the existing `use anyhow` and `use ratatui` lines):

```rust
use ratatui::crossterm::{execute, cursor::SetCursorStyle};
use buff::app::state::Focus;
```

- [ ] In the main loop, add the cursor shape command immediately after `terminal.draw(...)`:

```rust
loop {
    terminal.draw(|frame| {
        buff::ui::render(frame, &app, &theme);
    })?;

    // Set cursor shape to match the current vim mode.
    match app.focus {
        Focus::VimNormal => {
            execute!(std::io::stdout(), SetCursorStyle::SteadyBlock)?;
        }
        Focus::VimInsert => {
            execute!(std::io::stdout(), SetCursorStyle::SteadyBar)?;
        }
        _ => {
            execute!(std::io::stdout(), SetCursorStyle::DefaultUserShape)?;
        }
    }

    // Drain any LLM events that arrived since the last iteration.
    while let Ok(event) = llm_rx.try_recv() {
        app.handle_llm_event(event);
    }

    if let Some(key) = read_key()? {
        if let Some(action) = buff::app::input::key_to_action(&app, key) {
            if buff::app::input::execute_action(&mut app, action)?
                == buff::app::input::EventOutcome::Quit
            {
                break;
            }
        }
    }
}
```

### Step 5.2 — Build to verify compilation

- [ ] Run:

```bash
cargo build 2>&1 | grep -E "error|warning.*unused"
```

Expected: clean build (no errors). There may be warnings about unused imports from dead code elsewhere — ignore those if pre-existing.

### Step 5.3 — Run full test suite

- [ ] Run:

```bash
cargo test 2>&1 | grep -E "FAILED|test result"
```

Expected: all tests pass.

### Step 5.4 — Commit

- [ ] Commit:

```bash
git add src/main.rs
git commit -m "feat: set block/bar cursor shape in vim normal/insert modes"
```

---

## Final Verification

- [ ] Run the full test suite one last time:

```bash
cargo test 2>&1 | tail -5
```

Expected: all tests pass, no failures.

- [ ] Build a release binary to confirm clean compile:

```bash
cargo build --release 2>&1 | grep "error"
```

Expected: no output (no errors).
