# Theme Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a hybrid theme system (`light` default + `dark`) with named theme selection, per-element TOML overrides, configurable h1–h6 heading colors, a "Notes" bold pane title, and bold to-do items in the right panel.

**Architecture:** New `src/ui/theme.rs` holds the `Theme` struct and built-in theme fns; `Config` gains `theme: String` + `theme_overrides: ThemeOverrides`; `resolve_theme()` builds a concrete `Theme` at startup; `&Theme` is threaded through the render pipeline, replacing all hardcoded `Color::*` constants.

**Tech Stack:** Rust (edition 2024), ratatui 0.29, crossterm, serde + toml.

---

## File Map

| Action | File | Responsibility |
|---|---|---|
| **Create** | `src/ui/theme.rs` | `Theme` struct, `light()`, `dark()`, `parse_color()`, `resolve_theme()` |
| **Modify** | `src/config.rs` | Add `theme: String`, `ThemeOverrides` struct |
| **Modify** | `src/ui/mod.rs` | Export `pub mod theme` |
| **Modify** | `src/main.rs` | Build theme from config; pass `&theme` to `render()` |
| **Modify** | `src/ui/layout.rs` | Accept `&Theme`; use theme colors for borders; add "Notes" bold title |
| **Modify** | `src/ui/document.rs` | Accept `&Theme`; replace hardcoded colors; add h4–h6 rendering |
| **Modify** | `src/ui/right_panel.rs` | Accept `&Theme`; replace `PANEL_BG`; bold todo items |
| **Modify** | `src/ui/chat_panel.rs` | Accept `&Theme`; replace `PANEL_BG` |

---

## Task 1: Create `src/ui/theme.rs`

**Files:**
- Create: `src/ui/theme.rs`

- [ ] **Step 1: Write the failing tests**

Add to the bottom of the new file (write the file with tests first, using stubs):

```rust
// src/ui/theme.rs
use ratatui::style::Color;
use crate::config::ThemeOverrides;

// stubs so tests compile — real impl comes next
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
}
pub fn light() -> Theme { todo!() }
pub fn dark() -> Theme { todo!() }
pub fn parse_color(_s: &str) -> Result<Color, String> { todo!() }
pub fn resolve_theme(_name: &str, _overrides: &ThemeOverrides) -> Theme { todo!() }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ThemeOverrides;

    #[test]
    fn parse_named_color_cyan() {
        assert_eq!(parse_color("cyan").unwrap(), Color::Cyan);
    }

    #[test]
    fn parse_named_color_case_insensitive() {
        assert_eq!(parse_color("CYAN").unwrap(), Color::Cyan);
        assert_eq!(parse_color("DarkGray").unwrap(), Color::DarkGray);
    }

    #[test]
    fn parse_named_dark_gray_variants() {
        assert_eq!(parse_color("dark_gray").unwrap(), Color::DarkGray);
        assert_eq!(parse_color("darkgray").unwrap(), Color::DarkGray);
    }

    #[test]
    fn parse_hex_color() {
        assert_eq!(parse_color("#00bcd4").unwrap(), Color::Rgb(0, 188, 212));
        assert_eq!(parse_color("#ffffff").unwrap(), Color::Rgb(255, 255, 255));
        assert_eq!(parse_color("#020277bd").is_err(), true); // wrong length
    }

    #[test]
    fn parse_invalid_hex_returns_err() {
        assert!(parse_color("#gggggg").is_err());
    }

    #[test]
    fn parse_unknown_name_returns_err() {
        assert!(parse_color("notacolor").is_err());
    }

    #[test]
    fn light_theme_heading2_is_blue() {
        let theme = light();
        assert_eq!(theme.heading2, Color::Rgb(2, 119, 189));
    }

    #[test]
    fn dark_theme_heading1_is_white() {
        let theme = dark();
        assert_eq!(theme.heading1, Color::White);
    }

    #[test]
    fn dark_theme_heading2_is_cyan() {
        let theme = dark();
        assert_eq!(theme.heading2, Color::Cyan);
    }

    #[test]
    fn resolve_light_theme() {
        let theme = resolve_theme("light", &ThemeOverrides::default());
        assert_eq!(theme.heading2, Color::Rgb(2, 119, 189));
        assert_eq!(theme.border_focused, Color::Rgb(2, 119, 189));
    }

    #[test]
    fn resolve_dark_theme() {
        let theme = resolve_theme("dark", &ThemeOverrides::default());
        assert_eq!(theme.heading1, Color::White);
        assert_eq!(theme.heading2, Color::Cyan);
        assert_eq!(theme.border_focused, Color::Cyan);
    }

    #[test]
    fn resolve_unknown_theme_falls_back_to_light() {
        let theme = resolve_theme("bogus", &ThemeOverrides::default());
        assert_eq!(theme.heading2, Color::Rgb(2, 119, 189));
    }

    #[test]
    fn resolve_applies_valid_override() {
        let mut overrides = ThemeOverrides::default();
        overrides.heading1 = Some("red".to_string());
        let theme = resolve_theme("light", &overrides);
        assert_eq!(theme.heading1, Color::Red);
    }

    #[test]
    fn resolve_ignores_invalid_override_uses_base() {
        let mut overrides = ThemeOverrides::default();
        overrides.heading1 = Some("notacolor".to_string());
        let theme = resolve_theme("light", &overrides);
        // light default for heading1 is Black
        assert_eq!(theme.heading1, Color::Black);
    }

    #[test]
    fn resolve_hex_override() {
        let mut overrides = ThemeOverrides::default();
        overrides.border_focused = Some("#ff0000".to_string());
        let theme = resolve_theme("light", &overrides);
        assert_eq!(theme.border_focused, Color::Rgb(255, 0, 0));
    }
}
```

- [ ] **Step 2: Add `ThemeOverrides` stub to `src/config.rs`** (needed for the above to compile — just the struct, no Config changes yet):

```rust
// Add to the bottom of src/config.rs, before the #[cfg(test)] block:
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
}
```

- [ ] **Step 3: Run tests to verify they fail with `todo!()`**

```bash
cargo test -p buff theme
```

Expected: FAIL — tests panic at `todo!()`.

- [ ] **Step 4: Implement `theme.rs` fully**

Replace the stub implementations in `src/ui/theme.rs` with the real code:

```rust
use ratatui::style::Color;
use crate::config::ThemeOverrides;

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
}

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
    }
}

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
    }
}

pub fn parse_color(s: &str) -> Result<Color, String> {
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| format!("invalid hex color: #{}", hex))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| format!("invalid hex color: #{}", hex))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| format!("invalid hex color: #{}", hex))?;
            return Ok(Color::Rgb(r, g, b));
        }
        return Err(format!("invalid hex color: #{}", hex));
    }
    match s.to_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" | "grey" => Ok(Color::Gray),
        "dark_gray" | "darkgray" | "dark_grey" | "darkgrey" => Ok(Color::DarkGray),
        "light_red" | "lightred" => Ok(Color::LightRed),
        "light_green" | "lightgreen" => Ok(Color::LightGreen),
        "light_yellow" | "lightyellow" => Ok(Color::LightYellow),
        "light_blue" | "lightblue" => Ok(Color::LightBlue),
        "light_magenta" | "lightmagenta" => Ok(Color::LightMagenta),
        "light_cyan" | "lightcyan" => Ok(Color::LightCyan),
        "white" => Ok(Color::White),
        "reset" => Ok(Color::Reset),
        _ => Err(format!("unknown color: {}", s)),
    }
}

pub fn resolve_theme(name: &str, overrides: &ThemeOverrides) -> Theme {
    let mut theme = match name {
        "dark" => dark(),
        "light" => light(),
        _ => {
            eprintln!("buff: unknown theme '{}', falling back to 'light'", name);
            light()
        }
    };

    macro_rules! apply {
        ($field:ident) => {
            if let Some(ref s) = overrides.$field {
                match parse_color(s) {
                    Ok(c) => theme.$field = c,
                    Err(e) => eprintln!("buff: theme_overrides.{}: {}", stringify!($field), e),
                }
            }
        };
    }

    apply!(heading1);
    apply!(heading2);
    apply!(heading3);
    apply!(heading4);
    apply!(heading5);
    apply!(heading6);
    apply!(border_focused);
    apply!(border_unfocused);
    apply!(notes_panel_bg);
    apply!(panel_bg);
    apply!(chat_panel_bg);
    apply!(quote_marker);
    apply!(code);
    apply!(todo_done);
    apply!(todo_overdue);

    theme
}
```

- [ ] **Step 5: Export the module in `src/ui/mod.rs`**

```rust
// src/ui/mod.rs — full file:
mod calendar;
mod capture;
mod chat_panel;
mod document;
pub mod help;
pub mod layout;
pub mod right_panel;
pub mod theme;

pub use layout::render;
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test -p buff theme
```

Expected: all theme tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/ui/theme.rs src/ui/mod.rs src/config.rs
git commit -m "feat: add Theme struct, built-in light/dark themes, parse_color, resolve_theme"
```

---

## Task 2: Add `theme` + `theme_overrides` to `Config`

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests** (add to the `#[cfg(test)]` block in `src/config.rs`):

```rust
#[test]
fn theme_defaults_to_light() {
    let config = Config::default();
    assert_eq!(config.theme, "light");
}

#[test]
fn parse_theme_name_from_toml() {
    let toml = r#"theme = "dark""#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.theme, "dark");
}

#[test]
fn parse_theme_overrides_from_toml() {
    let toml = r#"
        theme = "light"
        [theme_overrides]
        heading1 = "red"
        border_focused = "#0000ff"
    "#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.theme, "light");
    assert_eq!(config.theme_overrides.heading1, Some("red".to_string()));
    assert_eq!(config.theme_overrides.border_focused, Some("#0000ff".to_string()));
}

#[test]
fn missing_theme_overrides_all_none() {
    let toml = r#"theme = "dark""#;
    let config: Config = toml::from_str(toml).unwrap();
    assert!(config.theme_overrides.heading1.is_none());
    assert!(config.theme_overrides.heading2.is_none());
    assert!(config.theme_overrides.panel_bg.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff config
```

Expected: FAIL — `Config` has no `theme` field.

- [ ] **Step 3: Add `theme` and `theme_overrides` to `Config`**

In `src/config.rs`, update the `Config` struct (add two fields at the end):

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
    pub theme: String,
    pub theme_overrides: ThemeOverrides,
}
```

Update `impl Default for Config` (add the two new fields):

```rust
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
            theme: "light".to_string(),
            theme_overrides: ThemeOverrides::default(),
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p buff config
```

Expected: all config tests PASS.

- [ ] **Step 5: Verify full build passes**

```bash
cargo build -p buff
```

Expected: PASS (no compile errors).

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat: add theme and theme_overrides fields to Config"
```

---

## Task 3: Thread `&Theme` through the render pipeline

This task updates all render function signatures and call sites so the project compiles with `&Theme` flowing from `main.rs` through to every renderer. No color logic changes yet — uses `_theme` where not consumed to suppress warnings.

**Files:**
- Modify: `src/main.rs`
- Modify: `src/ui/layout.rs`
- Modify: `src/ui/document.rs`
- Modify: `src/ui/right_panel.rs`
- Modify: `src/ui/chat_panel.rs`

- [ ] **Step 1: Update `src/main.rs`** — build theme and pass to render:

```rust
// src/main.rs — replace the run() function body:
fn run() -> Result<()> {
    let Some(cli) = parse_cli_args()? else {
        return Ok(());
    };

    let (config, notes_dir) = buff::config::load(cli.notes_dir).context("Config error")?;
    let theme = buff::ui::theme::resolve_theme(&config.theme, &config.theme_overrides);
    let mut app =
        buff::app::state::AppState::open_day(notes_dir, config, chrono::Local::now().date_naive())
            .context("Failed to open day")?;

    let (llm_tx, llm_rx) = std::sync::mpsc::channel::<buff::app::llm::LlmEvent>();
    app.chat.event_tx = Some(llm_tx);

    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;

    loop {
        terminal.draw(|frame| {
            buff::ui::render(frame, &app, &theme);
        })?;

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

    Ok(())
}
```

- [ ] **Step 2: Update `src/ui/layout.rs`** — add `theme` parameter and pass it to sub-renderers:

Change the signature of `pub fn render`:

```rust
pub fn render(frame: &mut ratatui::Frame, app: &AppState, theme: &crate::ui::theme::Theme) {
```

Update the three sub-renderer calls inside `render` (search for `super::document::render`, `super::chat_panel::render`, `super::right_panel::render`):

```rust
super::document::render(frame, app, notes_inner, theme);
// ...
super::chat_panel::render(frame, chat_inner, app, theme);
// ...
super::right_panel::render(frame, panel_inner, app, theme);
```

Add a `test_theme()` helper inside the `#[cfg(test)]` block in `layout.rs` and update every `render(frame, &app)` test call to `render(frame, &app, &test_theme())`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // ... existing imports ...

    fn test_theme() -> crate::ui::theme::Theme {
        crate::ui::theme::light()
    }

    // Update ALL calls in this test module from:
    //   render(frame, &app)
    // to:
    //   render(frame, &app, &test_theme())
    // There are 10 such calls — update each one.
```

- [ ] **Step 3: Update `src/ui/document.rs`** — add `_theme` parameter (unused for now):

```rust
pub fn render(frame: &mut ratatui::Frame, app: &AppState, area: Rect, _theme: &crate::ui::theme::Theme) {
```

- [ ] **Step 4: Update `src/ui/right_panel.rs`** — add `theme` to public and private fns:

```rust
pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState, theme: &crate::ui::theme::Theme) {
```

Inside `render`, pass `theme` to the two private helpers:

```rust
render_calendar(frame, chunks[0], app, theme);
render_todo_list(frame, chunks[1], app, theme);
```

Update private fn signatures:

```rust
fn render_calendar(frame: &mut ratatui::Frame, area: Rect, app: &AppState, _theme: &crate::ui::theme::Theme) {
fn render_todo_list(frame: &mut ratatui::Frame, area: Rect, app: &AppState, _theme: &crate::ui::theme::Theme) {
```

Add a `test_theme()` helper inside the `#[cfg(test)]` block and update all `render(frame, frame.area(), &app)` calls to `render(frame, frame.area(), &app, &test_theme())`:

```rust
fn test_theme() -> crate::ui::theme::Theme {
    crate::ui::theme::light()
}
```

- [ ] **Step 5: Update `src/ui/chat_panel.rs`** — add `_theme` parameter:

```rust
pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState, _theme: &crate::ui::theme::Theme) {
```

Update the two test calls from `render(f, f.area(), &app)` to `render(f, f.area(), &app, &crate::ui::theme::light())`.

- [ ] **Step 6: Verify full build and all tests pass**

```bash
cargo test -p buff
```

Expected: all existing tests PASS (there will be unused-variable warnings for `_theme` — that's fine).

- [ ] **Step 7: Commit**

```bash
git add src/main.rs src/ui/layout.rs src/ui/document.rs src/ui/right_panel.rs src/ui/chat_panel.rs
git commit -m "refactor: thread &Theme through render pipeline (no color changes yet)"
```

---

## Task 4: Apply theme in `layout.rs` + add "Notes" bold title

**Files:**
- Modify: `src/ui/layout.rs`

- [ ] **Step 1: Write failing tests** (add to the `#[cfg(test)]` block in `layout.rs`):

```rust
#[test]
fn notes_pane_title_is_notes() {
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let app = test_app(doc, Focus::Capture, 0);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("Notes"), "Expected 'Notes' block title, got: {}", content);
}

#[test]
fn notes_pane_focused_border_uses_theme_color() {
    use ratatui::style::Color;
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let app = test_app(doc, Focus::Capture, 0);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();
    let buffer = terminal.backend().buffer();
    // light theme border_focused = Color::Rgb(2, 119, 189)
    let has_focused_border = buffer.content.iter().any(|cell| {
        cell.style().fg == Some(Color::Rgb(2, 119, 189))
            && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
    });
    assert!(has_focused_border, "expected theme border_focused color on notes pane border");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff layout::tests::notes_pane_title_is_notes
cargo test -p buff layout::tests::notes_pane_focused_border_uses_theme_color
```

Expected: FAIL — no "Notes" title, border still uses hardcoded `Color::Cyan`.

- [ ] **Step 3: Apply theme to borders and add "Notes" title in `layout.rs`**

At the top of `layout.rs`, remove the two const declarations:

```rust
// DELETE these two lines:
const FOCUSED_BORDER: Color = Color::Cyan;
const UNFOCUSED_BORDER: Color = Color::DarkGray;
```

Also remove `Color` from the style import (it's no longer needed directly in layout.rs). The import becomes:

```rust
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders};
```

Update the notes block (around line 73) to use theme colors and add a "Notes" title:

```rust
let notes_block = Block::default()
    .borders(Borders::ALL)
    .title(Span::styled(" Notes ", Style::default().add_modifier(Modifier::BOLD)))
    .border_style(Style::default().fg(if notes_focused {
        theme.border_focused
    } else {
        theme.border_unfocused
    }))
    .style(Style::default().bg(theme.notes_panel_bg));
```

Update the chat block (around line 88):

```rust
let chat_block = Block::default()
    .borders(Borders::ALL)
    .border_style(Style::default().fg(if chat_focused {
        theme.border_focused
    } else {
        theme.border_unfocused
    }));
```

Update the right panel block (around line 97):

```rust
let panel_block = Block::default()
    .borders(Borders::ALL)
    .border_style(Style::default().fg(if panel_focused {
        theme.border_focused
    } else {
        theme.border_unfocused
    }));
```

- [ ] **Step 4: Update the existing border color tests** to use light theme colors

Find and update `notes_pane_has_cyan_border_in_capture_mode` — the light theme uses `Color::Rgb(2, 119, 189)` for focused borders:

```rust
#[test]
fn notes_pane_has_cyan_border_in_capture_mode() {
    use ratatui::style::Color;
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let app = test_app(doc, Focus::Capture, 0);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();
    let buffer = terminal.backend().buffer();
    let has_focused_border = buffer.content.iter().any(|cell| {
        cell.style().fg == Some(Color::Rgb(2, 119, 189))
            && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
    });
    assert!(has_focused_border, "expected focused border color for notes pane");
}
```

Find and update `chat_pane_has_cyan_border_when_focused`:

```rust
#[test]
fn chat_pane_has_cyan_border_when_focused() {
    use ratatui::style::Color;
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::Chat, 0);
    app.chat.visible = true;
    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();
    let buffer = terminal.backend().buffer();
    let has_focused_border = buffer.content.iter().any(|cell| {
        cell.style().fg == Some(Color::Rgb(2, 119, 189))
            && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
    });
    assert!(has_focused_border, "expected focused border color for chat pane");
}
```

(`right_panel_has_dark_border_when_notes_focused` checks `Color::DarkGray` — same in light theme, no change needed.)

- [ ] **Step 5: Run all tests to verify**

```bash
cargo test -p buff
```

Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/ui/layout.rs
git commit -m "feat: use theme colors for borders; add bold Notes pane title"
```

---

## Task 5: Apply theme in `document.rs` + add h4–h6 rendering

**Files:**
- Modify: `src/ui/document.rs`

- [ ] **Step 1: Write failing tests** (add to `layout.rs` tests — we test via the full render pipeline since `document::render` is not public):

```rust
#[test]
fn render_h4_h5_h6_headings() {
    let doc = Document::from_text("#### Level 4\n##### Level 5\n###### Level 6\n");
    let app = test_app(doc, Focus::Capture, 0);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();
    let buffer = terminal.backend().buffer();
    // h4 color = Color::Rgb(106, 27, 154) in light theme
    let has_h4_color = buffer.content.iter().any(|cell| {
        cell.style().fg == Some(ratatui::style::Color::Rgb(106, 27, 154))
    });
    assert!(has_h4_color, "expected h4 heading color applied");
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("Level 4"), "h4 text missing: {}", content);
    assert!(content.contains("Level 5"), "h5 text missing: {}", content);
    assert!(content.contains("Level 6"), "h6 text missing: {}", content);
}

#[test]
fn render_h1_uses_theme_color() {
    let doc = Document::from_text("# My Heading\n");
    let app = test_app(doc, Focus::Capture, 0);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app, &test_theme())).unwrap();
    let buffer = terminal.backend().buffer();
    // light theme h1 = Color::Black
    let has_h1_color = buffer.content.iter().any(|cell| {
        cell.style().fg == Some(ratatui::style::Color::Black)
            && cell.style().add_modifier.contains(ratatui::style::Modifier::BOLD)
    });
    assert!(has_h1_color, "expected h1 theme color with BOLD");
}
```

- [ ] **Step 2: Run the new tests to verify they fail**

```bash
cargo test -p buff layout::tests::render_h4_h5_h6_headings
cargo test -p buff layout::tests::render_h1_uses_theme_color
```

Expected: FAIL — `render_h4_h5_h6_headings` fails because no h4 color; `render_h1_uses_theme_color` fails because h1 is still `Color::White` (dark default) not `Color::Black` (light default).

- [ ] **Step 3: Rewrite `src/ui/document.rs` to use theme colors and add h4–h6**

Replace the entire file:

```rust
use crate::app::state::{AppState, Focus};
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

pub fn render(frame: &mut ratatui::Frame, app: &AppState, area: Rect, theme: &Theme) {
    let selected_range: Option<std::ops::Range<usize>> = if app.focus == Focus::Navigate {
        app.selectables.get(app.selected).map(|s| s.lines.clone())
    } else {
        None
    };

    let mut in_code = false;
    let text_lines: Vec<Line> = app
        .doc
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let is_selected = selected_range.as_ref().is_some_and(|r| r.contains(&i));
            let highlight = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            let fence = line.trim_start().starts_with("```");
            if in_code || fence {
                if fence {
                    in_code = !in_code;
                }
                return Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(theme.code),
                ))
                .style(highlight);
            }

            if let Some(rest) = line.strip_prefix("###### ") {
                Line::from(vec![Span::styled(
                    format!("###### {}", rest),
                    Style::default().fg(theme.heading6).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("##### ") {
                Line::from(vec![Span::styled(
                    format!("##### {}", rest),
                    Style::default().fg(theme.heading5).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("#### ") {
                Line::from(vec![Span::styled(
                    format!("#### {}", rest),
                    Style::default().fg(theme.heading4).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("### ") {
                Line::from(vec![Span::styled(
                    format!("### {}", rest),
                    Style::default().fg(theme.heading3).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("## ") {
                Line::from(vec![Span::styled(
                    format!("## {}", rest),
                    Style::default().fg(theme.heading2).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("# ") {
                Line::from(vec![Span::styled(
                    format!("# {}", rest),
                    Style::default().fg(theme.heading1).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("- [ ] ") {
                Line::from(vec![Span::raw("☐ "), Span::raw(rest)]).style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("- [x] ")
                .or_else(|| line.strip_prefix("- [X] "))
            {
                Line::from(vec![
                    Span::styled("☑ ", Style::default().fg(theme.todo_done)),
                    Span::styled(
                        rest,
                        Style::default()
                            .fg(theme.todo_done)
                            .add_modifier(Modifier::CROSSED_OUT),
                    ),
                ])
                .style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("> ")
                .or_else(|| if line == ">" { Some("") } else { None })
            {
                Line::from(vec![
                    Span::styled(
                        "│ ",
                        Style::default()
                            .fg(theme.quote_marker)
                            .add_modifier(Modifier::ITALIC),
                    ),
                    Span::styled(rest, Style::default().add_modifier(Modifier::ITALIC)),
                ])
                .style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .or_else(|| line.strip_prefix("+ "))
            {
                Line::from(vec![Span::raw("• "), Span::raw(rest)]).style(highlight)
            } else if crate::model::parser::is_ordered(line) {
                Line::from(Span::raw(line.as_str())).style(highlight)
            } else {
                Line::from(line.as_str()).style(highlight)
            }
        })
        .collect();

    let scroll_offset = if let Some(r) = selected_range {
        let visible_height = area.height as usize;
        r.end.saturating_sub(visible_height)
    } else {
        0
    };

    let paragraph = Paragraph::new(Text::from(text_lines)).scroll((scroll_offset as u16, 0));
    frame.render_widget(paragraph, area);
}
```

**Important:** The heading match order is longest-prefix-first (`######` before `#####` before `####` before `###` before `##` before `#`) to avoid shorter prefixes matching longer headings.

- [ ] **Step 4: Run all tests**

```bash
cargo test -p buff
```

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ui/document.rs src/ui/layout.rs
git commit -m "feat: apply theme colors to document renderer; add h4-h6 heading support"
```

---

## Task 6: Apply theme in `right_panel.rs` + bold to-do items

**Files:**
- Modify: `src/ui/right_panel.rs`

- [ ] **Step 1: Write failing tests** (add to `right_panel.rs` `#[cfg(test)]` block):

```rust
#[test]
fn todo_items_are_bold() {
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
        focus: Focus::Capture, // not RightPanel so no REVERSED
        selected: 0,
        status: String::new(),
        input: String::new(),
        cursor_pos: 0,
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
        chat: crate::app::state::ChatState::default(),
    };

    let backend = TestBackend::new(30, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render(frame, frame.area(), &app, &test_theme()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let has_bold = buffer
        .content
        .iter()
        .any(|cell| cell.style().add_modifier.contains(Modifier::BOLD));
    assert!(has_bold, "expected BOLD modifier on todo item text");
}

#[test]
fn panel_uses_theme_panel_bg() {
    use crate::app::state::{AppState, Context, Focus, Overlay};
    use crate::config::Config;
    use crate::model::day::Document;
    use chrono::NaiveDate;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
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
        cursor_pos: 0,
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
        chat: crate::app::state::ChatState::default(),
    };

    let backend = TestBackend::new(30, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render(frame, frame.area(), &app, &test_theme()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    // light theme panel_bg = Color::Rgb(221, 232, 245)
    let has_panel_bg = buffer
        .content
        .iter()
        .any(|cell| cell.style().bg == Some(Color::Rgb(221, 232, 245)));
    assert!(has_panel_bg, "expected light theme panel_bg color in right panel");
}
```

- [ ] **Step 2: Run new tests to verify they fail**

```bash
cargo test -p buff right_panel::tests::todo_items_are_bold
cargo test -p buff right_panel::tests::panel_uses_theme_panel_bg
```

Expected: FAIL — no BOLD modifier; panel uses old `Rgb(220, 220, 220)`.

- [ ] **Step 3: Apply changes in `src/ui/right_panel.rs`**

Remove the `const PANEL_BG` declaration:

```rust
// DELETE:
const PANEL_BG: Color = Color::Rgb(220, 220, 220);
```

Remove `Color` from the use statement (it's now sourced from theme). The import line becomes:

```rust
use ratatui::style::{Modifier, Style};
```

Update `pub fn render` — replace `PANEL_BG` with `theme.panel_bg`:

```rust
pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState, theme: &crate::ui::theme::Theme) {
    let bg_block = Block::default()
        .style(Style::default().bg(theme.panel_bg))
        .padding(Padding::new(2, 2, 2, 2));
    let inner = bg_block.inner(area);
    frame.render_widget(bg_block, area);

    let calendar_height = 9u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(calendar_height), Constraint::Min(0)])
        .split(inner);

    render_calendar(frame, chunks[0], app, theme);
    render_todo_list(frame, chunks[1], app, theme);
}
```

Update `render_calendar` — replace `PANEL_BG` with `theme.panel_bg`:

```rust
fn render_calendar(frame: &mut ratatui::Frame, area: Rect, app: &AppState, theme: &crate::ui::theme::Theme) {
    // ... (no other changes needed except replacing PANEL_BG)
    // Line 110: .style(Style::default().bg(theme.panel_bg));
    // Line 119: .style(Style::default().bg(theme.panel_bg));
    // Line 148: .style(Style::default().bg(theme.panel_bg));
```

Update `render_todo_list` — replace `PANEL_BG` with `theme.panel_bg` AND add `Modifier::BOLD` to the todo item style:

```rust
fn render_todo_list(frame: &mut ratatui::Frame, area: Rect, app: &AppState, theme: &crate::ui::theme::Theme) {
    let mut virtual_lines: Vec<Line> = Vec::new();

    virtual_lines.push(Line::from("To-dos"));

    let mut current_date = None;
    for (flat_idx, todo) in app.panel_todos.iter().enumerate() {
        if Some(todo.date) != current_date {
            current_date = Some(todo.date);
            let header = todo.date.format("%a %b %d").to_string();
            virtual_lines.push(Line::from(format!("─ {} ", header)));
        }
        let is_selected =
            app.focus == Focus::RightPanel && flat_idx == app.right_panel_selected;
        let item_text = format!("☐ {}", todo.text);
        let style = if is_selected {
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        virtual_lines.push(Line::styled(item_text, style));
    }

    let scroll = app
        .right_panel_scroll
        .min(virtual_lines.len().saturating_sub(1));
    let visible: Vec<Line> = virtual_lines
        .into_iter()
        .skip(scroll)
        .take(area.height as usize)
        .collect();

    let widget = Paragraph::new(visible).style(Style::default().bg(theme.panel_bg));
    frame.render_widget(widget, area);
}
```

- [ ] **Step 4: Run all tests**

```bash
cargo test -p buff
```

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ui/right_panel.rs
git commit -m "feat: apply theme panel_bg to right panel; bold todo items"
```

---

## Task 7: Apply theme in `chat_panel.rs`

**Files:**
- Modify: `src/ui/chat_panel.rs`

- [ ] **Step 1: Write failing test** (add to `chat_panel.rs` `#[cfg(test)]` block):

```rust
#[test]
fn panel_uses_theme_chat_panel_bg() {
    use ratatui::style::Color;

    let app = app_with_messages(vec![]);
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    // light theme chat_panel_bg = Color::Rgb(230, 230, 240)
    let has_bg = buffer
        .content
        .iter()
        .any(|cell| cell.style().bg == Some(Color::Rgb(230, 230, 240)));
    assert!(has_bg, "expected light theme chat_panel_bg color");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p buff chat_panel::tests::panel_uses_theme_chat_panel_bg
```

Expected: FAIL — color check passes but `_theme` not used yet (actually this test may pass already since `chat_panel_bg = Rgb(230,230,240)` matches the current hardcoded value). If it passes, adjust the assertion to also verify it works with `dark()` theme where `chat_panel_bg` is the same — swap to using a custom theme with a distinct color:

```rust
// Alternative test if the above accidentally passes:
#[test]
fn panel_uses_theme_chat_panel_bg() {
    use ratatui::style::Color;

    // Build a theme with a distinctive chat_panel_bg to prove the theme is used
    let mut overrides = crate::config::ThemeOverrides::default();
    overrides.chat_panel_bg = Some("#010203".to_string());
    let custom_theme = crate::ui::theme::resolve_theme("light", &overrides);

    let app = app_with_messages(vec![]);
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render(f, f.area(), &app, &custom_theme))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let has_bg = buffer
        .content
        .iter()
        .any(|cell| cell.style().bg == Some(Color::Rgb(1, 2, 3)));
    assert!(has_bg, "expected custom chat_panel_bg color from theme");
}
```

- [ ] **Step 3: Run to verify this version fails**

```bash
cargo test -p buff chat_panel::tests::panel_uses_theme_chat_panel_bg
```

Expected: FAIL — `_theme` param not used; bg is still the hardcoded constant `Rgb(230, 230, 240)`, not `Rgb(1, 2, 3)`.

- [ ] **Step 4: Apply theme in `src/ui/chat_panel.rs`**

Remove the `const PANEL_BG` declaration:

```rust
// DELETE:
const PANEL_BG: Color = Color::Rgb(230, 230, 240);
```

Change the signature (remove leading `_` since theme is now used):

```rust
pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState, theme: &crate::ui::theme::Theme) {
```

Replace every use of `PANEL_BG` with `theme.chat_panel_bg`. There are 6 occurrences — all in the `render` fn body. Find each `PANEL_BG` and replace with `theme.chat_panel_bg`:

```rust
// Line 38:
let bg = Block::default()
    .style(Style::default().bg(theme.chat_panel_bg))
    .padding(Padding::new(1, 1, 1, 1));

// Line 52-56:
let header = Paragraph::new("Chat").style(
    Style::default()
        .bg(theme.chat_panel_bg)
        .add_modifier(Modifier::BOLD),
);

// Line 70:
lines.push(Line::styled(wl, style.bg(theme.chat_panel_bg)));

// Line 80:
lines.push(Line::styled("…", Style::default().bg(theme.chat_panel_bg)));

// Line 91:
frame.render_widget(
    Paragraph::new(visible).style(Style::default().bg(theme.chat_panel_bg)),
    body_area,
);

// Line 97 (error status):
let status_widget = Paragraph::new(status.clone())
    .style(Style::default().bg(theme.chat_panel_bg).fg(Color::Red));
```

Note: `Color` is still needed for `Color::Red` on the error status line — keep `use ratatui::style::{Color, Modifier, Style};` unchanged.

Also update the two existing test render calls (already done in Task 3 — verify they still use `&crate::ui::theme::light()`).

- [ ] **Step 5: Run all tests**

```bash
cargo test -p buff
```

Expected: all tests PASS.

- [ ] **Step 6: Final build check**

```bash
cargo build -p buff
```

Expected: PASS with no errors. Warnings about unused imports (`Color` in files that no longer use it directly) should be absent; fix any that appear.

- [ ] **Step 7: Commit**

```bash
git add src/ui/chat_panel.rs
git commit -m "feat: apply theme chat_panel_bg to chat panel"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Task |
|---|---|
| `Theme` struct with all fields including `todo_overdue` stub | Task 1 |
| `light()` built-in theme (default) | Task 1 |
| `dark()` built-in theme | Task 1 |
| `parse_color()` — named + hex | Task 1 |
| `resolve_theme()` — base + overrides | Task 1 |
| `theme: String` in `Config` defaulting to `"light"` | Task 2 |
| `ThemeOverrides` struct in `config.rs` | Task 1 + 2 |
| `[theme_overrides]` TOML section deserialization | Task 2 |
| `&Theme` threaded through render pipeline | Task 3 |
| Theme built in `main.rs` from config | Task 3 |
| Border colors from theme (`layout.rs`) | Task 4 |
| `notes_panel_bg` from theme (`layout.rs`) | Task 4 |
| "Notes" bold title on notes pane | Task 4 |
| h1–h3 colors from theme (`document.rs`) | Task 5 |
| h4–h6 colors from theme (`document.rs`) | Task 5 |
| `quote_marker`, `code`, `todo_done` from theme | Task 5 |
| `panel_bg` from theme (`right_panel.rs`) | Task 6 |
| Bold to-do items in right panel | Task 6 |
| `chat_panel_bg` from theme (`chat_panel.rs`) | Task 7 |

All spec requirements covered. No gaps.
