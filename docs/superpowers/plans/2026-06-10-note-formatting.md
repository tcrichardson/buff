# Note Formatting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hide `#` heading markers from display and render indented bullets/todos identically to unindented ones, with both behaviors reverting to raw text when the vim cursor is on the line.

**Architecture:** All rendering lives in `src/ui/document.rs`. We first extract the per-line styling logic into a testable `style_line` helper function (pure refactor), then make the two targeted changes. No other files are touched.

**Tech Stack:** Rust, ratatui (`Line`, `Span`, `Style`, `Modifier`)

---

## File Structure

| File | Change |
|------|--------|
| `src/ui/document.rs` | Extract `style_line` helper; hide `#` in headings; handle indented bullets/todos |

---

### Task 1: Extract `style_line` helper (pure refactor)

**Files:**
- Modify: `src/ui/document.rs`

This is a pure structural refactor — no behavior change. It makes the rendering logic testable without a `ratatui::Frame`.

- [ ] **Step 1: Add the `style_line` function just above the `render` function**

Replace the content of `src/ui/document.rs` lines 1–137 with:

```rust
use crate::app::state::AppState;
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

/// Converts one document line to a styled ratatui `Line` for display.
///
/// `in_code` tracks whether a code-fence block is active; it is mutated
/// when a fence marker is encountered.  `vim_cursor` is true when the vim
/// cursor sits on this line — in that case the raw text is returned with a
/// background highlight and `in_code` is reset.
fn style_line<'a>(line: &'a str, in_code: &mut bool, vim_cursor: bool, theme: &Theme) -> Line<'a> {
    if vim_cursor {
        *in_code = false;
        let bg_style = Style::default().bg(theme.vim_cursor_line);
        return Line::from(Span::styled(line, bg_style));
    }

    let fence = line.trim_start().starts_with("```");
    if *in_code || fence {
        if fence {
            *in_code = !*in_code;
        }
        return Line::from(Span::styled(line, Style::default().fg(theme.code)));
    }

    if let Some(rest) = line.strip_prefix("###### ") {
        Line::from(Span::styled(
            format!("###### {}", rest),
            Style::default().fg(theme.heading6).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("##### ") {
        Line::from(Span::styled(
            format!("##### {}", rest),
            Style::default().fg(theme.heading5).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("#### ") {
        Line::from(Span::styled(
            format!("#### {}", rest),
            Style::default().fg(theme.heading4).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("### ") {
        Line::from(Span::styled(
            format!("### {}", rest),
            Style::default().fg(theme.heading3).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("## ") {
        Line::from(Span::styled(
            format!("## {}", rest),
            Style::default().fg(theme.heading2).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("# ") {
        Line::from(Span::styled(
            format!("# {}", rest),
            Style::default().fg(theme.heading1).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("- [ ] ") {
        Line::from(vec![Span::raw("☐ "), Span::raw(rest)])
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
    } else if let Some(rest) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        Line::from(vec![Span::raw("• "), Span::raw(rest)])
    } else if crate::model::parser::is_ordered(line) {
        Line::from(Span::raw(line))
    } else {
        Line::from(line)
    }
}

pub fn render(frame: &mut ratatui::Frame, app: &AppState, area: Rect, theme: &Theme) {
    use crate::app::state::Focus;

    let vim_active = matches!(app.focus, Focus::VimNormal | Focus::VimInsert);
    let cursor_line = app.vim.cursor_line;

    let mut in_code = false;
    let text_lines: Vec<Line> = app
        .doc
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let vim_cursor = vim_active && i == cursor_line;
            style_line(line.as_str(), &mut in_code, vim_cursor, theme)
        })
        .collect();

    // Scroll: follow cursor in vim mode, else 0
    let scroll_offset: usize = if vim_active {
        let visible_height = area.height as usize;
        cursor_line.saturating_sub(visible_height.saturating_sub(1))
    } else {
        0
    };

    let paragraph = Paragraph::new(Text::from(text_lines)).scroll((scroll_offset as u16, 0));
    frame.render_widget(paragraph, area);

    // Place terminal cursor for vim modes
    if vim_active {
        let line_text = app.doc.lines.get(cursor_line).map(|l| l.as_str()).unwrap_or("");
        let display_col = line_text[..app.vim.cursor_col.min(line_text.len())]
            .chars()
            .count() as u16;
        let display_row = (cursor_line.saturating_sub(scroll_offset)) as u16;
        if display_row < area.height {
            frame.set_cursor_position((
                area.x + display_col,
                area.y + display_row,
            ));
        }
    }
}

pub fn render_mode_line(
    frame: &mut ratatui::Frame,
    app: &AppState,
    area: Rect,
    theme: &Theme,
) {
    use crate::app::state::Focus;
    let total = app.doc.lines.len();
    let current = app.vim.cursor_line + 1;
    let (mode_label, mode_color) = match app.focus {
        Focus::VimNormal => ("-- NORMAL --", theme.heading2),
        Focus::VimInsert => ("-- INSERT --", theme.heading3),
        _ => return,
    };
    let left = Span::styled(mode_label, Style::default().fg(mode_color));
    let right_text = format!("ln {}/{}", current, total);
    let right = Span::styled(right_text, Style::default().fg(theme.heading6));
    let left_len = mode_label.len() as u16;
    let right_len = format!("ln {}/{}", current, total).len() as u16;
    let gap = area.width.saturating_sub(left_len + right_len);
    let line = Line::from(vec![
        left,
        Span::raw(" ".repeat(gap as usize)),
        right,
    ]);
    let widget = ratatui::widgets::Paragraph::new(line);
    frame.render_widget(widget, area);
}
```

- [ ] **Step 2: Build to confirm no errors**

Run: `cargo build`
Expected: compiles without errors or warnings about unused variables.

- [ ] **Step 3: Commit the pure refactor**

```bash
git add src/ui/document.rs
git commit -m "refactor: extract style_line helper from document render loop"
```

---

### Task 2: Hide `#` in heading rendering

**Files:**
- Modify: `src/ui/document.rs`

- [ ] **Step 1: Add failing tests for heading display**

Append to `src/ui/document.rs` (after the closing `}` of `render_mode_line`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};

    fn th() -> theme::Theme {
        theme::light()
    }

    // --- Change 1: headings should not display the # prefix ---

    #[test]
    fn heading1_hides_hash() {
        let mut in_code = false;
        let result = style_line("# My Note", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(Span::styled(
                "My Note",
                Style::default().fg(th().heading1).add_modifier(Modifier::BOLD),
            ))
        );
    }

    #[test]
    fn heading2_hides_hashes() {
        let mut in_code = false;
        let result = style_line("## Notes", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(Span::styled(
                "Notes",
                Style::default().fg(th().heading2).add_modifier(Modifier::BOLD),
            ))
        );
    }

    #[test]
    fn heading3_hides_hashes() {
        let mut in_code = false;
        let result = style_line("### Sub", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(Span::styled(
                "Sub",
                Style::default().fg(th().heading3).add_modifier(Modifier::BOLD),
            ))
        );
    }

    #[test]
    fn heading4_hides_hashes() {
        let mut in_code = false;
        let result = style_line("#### Deep", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(Span::styled(
                "Deep",
                Style::default().fg(th().heading4).add_modifier(Modifier::BOLD),
            ))
        );
    }

    #[test]
    fn heading5_hides_hashes() {
        let mut in_code = false;
        let result = style_line("##### Five", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(Span::styled(
                "Five",
                Style::default().fg(th().heading5).add_modifier(Modifier::BOLD),
            ))
        );
    }

    #[test]
    fn heading6_hides_hashes() {
        let mut in_code = false;
        let result = style_line("###### Six", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(Span::styled(
                "Six",
                Style::default().fg(th().heading6).add_modifier(Modifier::BOLD),
            ))
        );
    }

    #[test]
    fn heading_on_cursor_line_shows_raw() {
        let mut in_code = false;
        let result = style_line("# My Note", &mut in_code, true, &th());
        assert_eq!(
            result,
            Line::from(Span::styled(
                "# My Note",
                Style::default().bg(th().vim_cursor_line),
            ))
        );
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p buff -- document::tests 2>&1 | head -40`
Expected: 6 heading tests FAIL (content still includes `#` prefix), `heading_on_cursor_line_shows_raw` PASS.

- [ ] **Step 3: Implement the heading change in `style_line`**

In `src/ui/document.rs`, find the six heading `if let` branches inside `style_line` and replace each `format!("…", rest)` with `rest` directly:

```rust
    if let Some(rest) = line.strip_prefix("###### ") {
        Line::from(Span::styled(
            rest,
            Style::default().fg(theme.heading6).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("##### ") {
        Line::from(Span::styled(
            rest,
            Style::default().fg(theme.heading5).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("#### ") {
        Line::from(Span::styled(
            rest,
            Style::default().fg(theme.heading4).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("### ") {
        Line::from(Span::styled(
            rest,
            Style::default().fg(theme.heading3).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("## ") {
        Line::from(Span::styled(
            rest,
            Style::default().fg(theme.heading2).add_modifier(Modifier::BOLD),
        ))
    } else if let Some(rest) = line.strip_prefix("# ") {
        Line::from(Span::styled(
            rest,
            Style::default().fg(theme.heading1).add_modifier(Modifier::BOLD),
        ))
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test -p buff -- document::tests 2>&1`
Expected: all 7 heading tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ui/document.rs
git commit -m "feat: hide # heading markers from display (show only on cursor line)"
```

---

### Task 3: Handle indented bullet and todo rendering

**Files:**
- Modify: `src/ui/document.rs`

- [ ] **Step 1: Add failing tests for indented bullets and todos**

Add the following tests inside the existing `mod tests` block in `src/ui/document.rs`, before the closing `}`:

```rust
    // --- Change 2: indented bullets/todos render same as unindented ---

    #[test]
    fn unindented_bullet_still_works() {
        let mut in_code = false;
        let result = style_line("- item", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(vec![Span::raw("• "), Span::raw("item")])
        );
    }

    #[test]
    fn indented_bullet_two_spaces() {
        let mut in_code = false;
        let result = style_line("  - sub", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(vec![Span::raw("  "), Span::raw("• "), Span::raw("sub")])
        );
    }

    #[test]
    fn indented_bullet_four_spaces() {
        let mut in_code = false;
        let result = style_line("    * deep", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(vec![Span::raw("    "), Span::raw("• "), Span::raw("deep")])
        );
    }

    #[test]
    fn indented_bullet_plus_marker() {
        let mut in_code = false;
        let result = style_line("  + item", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(vec![Span::raw("  "), Span::raw("• "), Span::raw("item")])
        );
    }

    #[test]
    fn indented_todo_unchecked() {
        let mut in_code = false;
        let result = style_line("  - [ ] task", &mut in_code, false, &th());
        assert_eq!(
            result,
            Line::from(vec![Span::raw("  "), Span::raw("☐ "), Span::raw("task")])
        );
    }

    #[test]
    fn indented_todo_checked_lowercase() {
        let t = th();
        let mut in_code = false;
        let result = style_line("  - [x] done", &mut in_code, false, &t);
        assert_eq!(
            result,
            Line::from(vec![
                Span::raw("  "),
                Span::styled("☑ ", Style::default().fg(t.todo_done)),
                Span::styled(
                    "done",
                    Style::default()
                        .fg(t.todo_done)
                        .add_modifier(Modifier::CROSSED_OUT),
                ),
            ])
        );
    }

    #[test]
    fn indented_todo_checked_uppercase() {
        let t = th();
        let mut in_code = false;
        let result = style_line("    - [X] done", &mut in_code, false, &t);
        assert_eq!(
            result,
            Line::from(vec![
                Span::raw("    "),
                Span::styled("☑ ", Style::default().fg(t.todo_done)),
                Span::styled(
                    "done",
                    Style::default()
                        .fg(t.todo_done)
                        .add_modifier(Modifier::CROSSED_OUT),
                ),
            ])
        );
    }
```

- [ ] **Step 2: Run tests to confirm the new ones fail**

Run: `cargo test -p buff -- document::tests 2>&1 | head -60`
Expected: `unindented_bullet_still_works` PASS; the six `indented_*` tests FAIL (indented lines fall through to plain-text rendering).

- [ ] **Step 3: Implement indented bullet and todo handling in `style_line`**

In `src/ui/document.rs`, inside `style_line`, replace the four branches for todo-unchecked, todo-checked, blockquote, and plain-bullet with the block below. The key change: todo and bullet branches extract `indent`/`trimmed` and match against `trimmed`; blockquote is unchanged (still uses `line`).

Replace from `} else if let Some(rest) = line.strip_prefix("- [ ] ")` through `Line::from(vec![Span::raw("• "), Span::raw(rest)])` with:

```rust
    } else {
        // Extract leading whitespace once; bullet/todo branches match against
        // the trimmed portion so indented variants render like unindented ones.
        // Blockquotes are not indented in this app and still match against `line`.
        let indent_len = line.len() - line.trim_start().len();
        let indent = &line[..indent_len];
        let trimmed = &line[indent_len..];

        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::raw("☐ "));
            spans.push(Span::raw(rest));
            Line::from(spans)
        } else if let Some(rest) = trimmed
            .strip_prefix("- [x] ")
            .or_else(|| trimmed.strip_prefix("- [X] "))
        {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::styled("☑ ", Style::default().fg(theme.todo_done)));
            spans.push(Span::styled(
                rest,
                Style::default()
                    .fg(theme.todo_done)
                    .add_modifier(Modifier::CROSSED_OUT),
            ));
            Line::from(spans)
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
        } else if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::raw("• "));
            spans.push(Span::raw(rest));
            Line::from(spans)
        } else if crate::model::parser::is_ordered(line) {
            Line::from(Span::raw(line))
        } else {
            Line::from(line)
        }
    }
```

Note: this replaces the old closing `}` of the heading `if-else` chain with a new `else { ... }` block that contains the todo/bullet/blockquote/plain branches. The heading branches at the top of `style_line` remain unchanged.

- [ ] **Step 4: Run all tests to confirm everything passes**

Run: `cargo test -p buff -- document::tests 2>&1`
Expected: all tests PASS (7 heading tests + 8 bullet/todo tests).

- [ ] **Step 5: Run the full test suite to confirm no regressions**

Run: `cargo test 2>&1`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/ui/document.rs
git commit -m "feat: render indented bullets and todos identically to unindented"
```
