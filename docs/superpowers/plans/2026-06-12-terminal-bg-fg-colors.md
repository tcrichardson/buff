# Terminal Background and Foreground Colors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `terminal_bg` and `terminal_fg` color fields to buff's theme system so the entire terminal canvas has a configurable background and default text color, making the dark theme work correctly even on light-background terminals.

**Architecture:** Two new fields are added to the `Theme` struct and `ThemeOverrides` config struct. At the start of every frame render, a full-area Background block is painted using these colors, establishing the canvas that all panels render on top of. Light theme defaults to `Color::Reset` (no behavior change); dark theme defaults to an explicit near-black background and white text.

**Tech Stack:** Rust, Ratatui (`ratatui::style::Color`, `ratatui::widgets::Block`, `ratatui::style::Style`), TOML config via serde

---

### Task 1: Add `terminal_bg` and `terminal_fg` to `Theme` struct and built-in themes

**Files:**
- Modify: `src/ui/theme.rs`

- [ ] **Step 1: Write failing tests**

Open `src/ui/theme.rs` and add these tests inside the existing `#[cfg(test)]` block at the bottom:

```rust
#[test]
fn light_theme_terminal_bg_is_reset() {
    let theme = light();
    assert_eq!(theme.terminal_bg, Color::Reset);
}

#[test]
fn light_theme_terminal_fg_is_reset() {
    let theme = light();
    assert_eq!(theme.terminal_fg, Color::Reset);
}

#[test]
fn dark_theme_terminal_bg_is_dark() {
    let theme = dark();
    assert_eq!(theme.terminal_bg, Color::Rgb(18, 18, 18));
}

#[test]
fn dark_theme_terminal_fg_is_white() {
    let theme = dark();
    assert_eq!(theme.terminal_fg, Color::White);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff light_theme_terminal_bg_is_reset light_theme_terminal_fg_is_reset dark_theme_terminal_bg_is_dark dark_theme_terminal_fg_is_white 2>&1 | head -30
```

Expected: compile error — `Theme` has no `terminal_bg` or `terminal_fg` fields.

- [ ] **Step 3: Add fields to `Theme` struct**

In `src/ui/theme.rs`, find the `Theme` struct (lines 4–24) and add two fields at the end:

```rust
#[derive(Clone, Debug)]
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
    pub capture_bg: Color,
    pub metadata: Color,
    pub terminal_bg: Color,
    pub terminal_fg: Color,
}
```

- [ ] **Step 4: Update `light()` function**

Find the `light()` function and add `terminal_bg` and `terminal_fg` at the end of the struct literal:

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
        capture_bg: Color::Reset,
        metadata: Color::DarkGray,
        terminal_bg: Color::Reset,
        terminal_fg: Color::Reset,
    }
}
```

- [ ] **Step 5: Update `dark()` function**

Find the `dark()` function and add `terminal_bg` and `terminal_fg` at the end of the struct literal:

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
        capture_bg: Color::Reset,
        metadata: Color::Gray,
        terminal_bg: Color::Rgb(18, 18, 18),
        terminal_fg: Color::White,
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test -p buff light_theme_terminal_bg_is_reset light_theme_terminal_fg_is_reset dark_theme_terminal_bg_is_dark dark_theme_terminal_fg_is_white 2>&1
```

Expected: 4 tests pass.

- [ ] **Step 7: Run full test suite to check no regressions**

```bash
cargo test 2>&1 | tail -20
```

Expected: all existing tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/ui/theme.rs
git commit -m "feat: add terminal_bg and terminal_fg fields to Theme struct"
```

---

### Task 2: Add override support in `resolve_theme()`

**Files:**
- Modify: `src/ui/theme.rs` (the `resolve_theme` function and its tests)

- [ ] **Step 1: Write failing tests**

Add these tests inside the existing `#[cfg(test)]` block in `src/ui/theme.rs`:

```rust
#[test]
fn resolve_applies_terminal_bg_override() {
    let mut overrides = ThemeOverrides::default();
    overrides.terminal_bg = Some("#1e1e1e".to_string());
    let theme = resolve_theme("light", &overrides);
    assert_eq!(theme.terminal_bg, Color::Rgb(30, 30, 30));
}

#[test]
fn resolve_applies_terminal_fg_override() {
    let mut overrides = ThemeOverrides::default();
    overrides.terminal_fg = Some("cyan".to_string());
    let theme = resolve_theme("light", &overrides);
    assert_eq!(theme.terminal_fg, Color::Cyan);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff resolve_applies_terminal_bg_override resolve_applies_terminal_fg_override 2>&1 | head -30
```

Expected: compile error — `ThemeOverrides` has no `terminal_bg` or `terminal_fg` fields.

- [ ] **Step 3: Add fields to `ThemeOverrides` in `src/config.rs`**

Open `src/config.rs`. Find the `ThemeOverrides` struct (lines 133–153) and add two fields at the end:

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
    pub capture_bg: Option<String>,
    pub metadata: Option<String>,
    pub terminal_bg: Option<String>,
    pub terminal_fg: Option<String>,
}
```

- [ ] **Step 4: Add `apply!` calls in `resolve_theme()`**

In `src/ui/theme.rs`, find the `resolve_theme()` function. The `apply!` macro block ends with `apply!(metadata);`. Add two more lines immediately after:

```rust
    apply!(metadata);
    apply!(terminal_bg);
    apply!(terminal_fg);
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p buff resolve_applies_terminal_bg_override resolve_applies_terminal_fg_override 2>&1
```

Expected: 2 tests pass.

- [ ] **Step 6: Run full test suite**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/ui/theme.rs src/config.rs
git commit -m "feat: add terminal_bg/terminal_fg to ThemeOverrides and resolve_theme"
```

---

### Task 3: Add config parse test for new override fields

**Files:**
- Modify: `src/config.rs` (tests section)

- [ ] **Step 1: Write the test**

In `src/config.rs`, add this test inside the existing `#[cfg(test)]` block:

```rust
#[test]
fn parse_terminal_bg_and_fg_overrides_from_toml() {
    let toml = r##"
        theme = "dark"
        [theme_overrides]
        terminal_bg = "#1e1e1e"
        terminal_fg = "white"
    "##;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.theme_overrides.terminal_bg, Some("#1e1e1e".to_string()));
    assert_eq!(config.theme_overrides.terminal_fg, Some("white".to_string()));
}

#[test]
fn terminal_overrides_absent_are_none() {
    let toml = r#"theme = "dark""#;
    let config: Config = toml::from_str(toml).unwrap();
    assert!(config.theme_overrides.terminal_bg.is_none());
    assert!(config.theme_overrides.terminal_fg.is_none());
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test -p buff parse_terminal_bg_and_fg_overrides_from_toml terminal_overrides_absent_are_none 2>&1
```

Expected: 2 tests pass (fields already added in Task 2).

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "test: add config parse tests for terminal_bg/terminal_fg overrides"
```

---

### Task 4: Paint the terminal canvas in `layout.rs::render()`

**Files:**
- Modify: `src/ui/layout.rs`

- [ ] **Step 1: Write failing test**

In `src/ui/layout.rs`, add this test inside the existing `#[cfg(test)]` block. Place it after the last test:

```rust
#[test]
fn render_terminal_bg_paints_canvas() {
    use ratatui::style::Color;
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let app = test_app(doc, Focus::Capture, 0);

    // Build a custom theme with a distinctive terminal_bg and terminal_fg.
    let mut theme = crate::ui::theme::light();
    theme.terminal_bg = Color::Rgb(99, 0, 99);
    theme.terminal_fg = Color::Rgb(200, 200, 0);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &theme)).unwrap();

    let buffer = terminal.backend().buffer();
    // At least one cell should carry the terminal_bg as its background.
    let has_canvas_bg = buffer
        .content
        .iter()
        .any(|cell| cell.style().bg == Some(Color::Rgb(99, 0, 99)));
    assert!(
        has_canvas_bg,
        "expected terminal_bg to appear in at least one canvas cell"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p buff render_terminal_bg_paints_canvas 2>&1
```

Expected: FAIL — the test passes but `has_canvas_bg` is false, because no canvas background is painted yet. (Or it may compile and fail the assertion.)

- [ ] **Step 3: Paint the canvas in `render()`**

Open `src/ui/layout.rs`. At the very top of the `render()` function body, before the `let input_line_count` line, add:

```rust
pub fn render(frame: &mut ratatui::Frame, app: &AppState, theme: &crate::ui::theme::Theme) {
    // Paint the terminal canvas with the theme's base background and foreground.
    // All panel widgets render on top of this layer.
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.terminal_bg).fg(theme.terminal_fg)),
        frame.area(),
    );

    // Full-width outer vertical split: header | middle (panels) | status | input
    let input_line_count = app.input.split('\n').count().max(1) as u16;
    // ... rest unchanged
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p buff render_terminal_bg_paints_canvas 2>&1
```

Expected: PASS.

- [ ] **Step 5: Run full test suite**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/ui/layout.rs
git commit -m "feat: paint terminal canvas with terminal_bg/terminal_fg at frame start"
```

---

### Task 5: Update README.md

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add two rows to the theme overrides table**

In `README.md`, find the theme overrides table (the one that starts with `| Override key | Default (light) | Example values |`). Add two rows at the end of the table, after the `metadata` row:

```markdown
| `terminal_bg` | `reset` | `"black"`, `"#121212"` |
| `terminal_fg` | `reset` | `"white"`, `"#e0e0e0"` |
```

The dark theme's built-in default (`#121212`) should also be noted. Update the note below the table (or the `Themes` section description) to mention that the `dark` theme sets `terminal_bg = #121212` and `terminal_fg = white` by default:

After the table, find the line that begins "Colors can be specified as:" and add a note before it:

```markdown
> **Note:** The `dark` theme sets `terminal_bg` to `#121212` and `terminal_fg` to `white` by default, so it renders correctly on terminals with a light background. Override these in `[theme_overrides]` to customise or restore terminal-inherited colours (`reset`).

```

- [ ] **Step 2: Verify the file looks right**

```bash
grep -A 5 "terminal_bg" README.md
```

Expected: Both rows visible in the table and the note visible below it.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document terminal_bg and terminal_fg theme fields in README"
```

---

### Task 6: Smoke-test the full build

- [ ] **Step 1: Build in release mode**

```bash
cargo build --release 2>&1 | tail -20
```

Expected: `Finished release [optimized]` with no errors or warnings about unused fields.

- [ ] **Step 2: Run the complete test suite one final time**

```bash
cargo test 2>&1 | tail -30
```

Expected: all tests pass, zero failures.

- [ ] **Step 3: Tag the work complete**

```bash
git log --oneline -6
```

Expected output should show the 5 commits from Tasks 1–5 on top of the spec commit.
