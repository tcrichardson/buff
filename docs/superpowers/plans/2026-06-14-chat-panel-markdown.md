# Chat Panel Markdown Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render markdown in the chat panel with full parity to the notes panel by extracting a shared `src/ui/markdown.rs` module.

**Architecture:** Move `LineKind`, `classify_line`, `parse_inline_formatting`, and `render_line_kind` from `document.rs` into a new public `ui/markdown.rs`. Document-specific variants (`VimCursor`, `MetaField`) are handled locally in `document.rs`. `chat_panel.rs` uses the shared module to render markdown-aware, word-wrapped message lines with speaker labels on their own lines.

**Tech Stack:** Rust, ratatui 0.30, crossterm — no new dependencies.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/ui/markdown.rs` | **Create** | Shared markdown pipeline: `LineKind`, `classify_line`, `parse_inline_formatting`, `render_line_kind`, `render_markdown_line` |
| `src/ui/mod.rs` | **Modify** | Register `pub mod markdown` |
| `src/ui/document.rs` | **Modify** | Remove duplicated code; handle `VimCursor`/`MetaField` locally; delegate to `markdown::render_markdown_line` |
| `src/ui/chat_panel.rs` | **Modify** | Speaker-label-per-line, markdown-aware word-wrap via `render_markdown_wrapped` |

---

## Task 1: Create `src/ui/markdown.rs`

**Files:**
- Create: `src/ui/markdown.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1.1: Write `src/ui/markdown.rs`**

Create the file with the full contents below. Key differences from the current `document.rs` code:
- `LineKind` has no `VimCursor` or `MetaField` variants
- `classify_line` takes two parameters (no `vim_cursor`)
- `parse_inline_formatting` returns `Vec<Span<'static>>` (strings are cloned to owned, so callers have no lifetime constraints)
- `render_line_kind` returns `Line<'static>` for the same reason
- `heading_color` is private

```rust
use crate::ui::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Classifies a single markdown line into a rendering variant.
#[derive(Debug, PartialEq)]
pub enum LineKind<'a> {
    /// Inside (or entering/leaving) a code fence — show with code colour.
    Code(&'a str),
    /// Markdown heading. `u8` is the level (1–6); `&str` is the text after the hashes.
    Heading(u8, &'a str),
    /// Unchecked todo checkbox. `(indent, rest)`.
    TodoUnchecked(&'a str, &'a str),
    /// Checked todo checkbox (lowercase or uppercase x). `(indent, rest)`.
    TodoDone(&'a str, &'a str),
    /// Blockquote line. Contains the text after `"> "` (may be empty).
    Quote(&'a str),
    /// Unordered bullet (`-`, `*`, `+`). `(indent, rest)`.
    Bullet(&'a str, &'a str),
    /// Ordered list item — shown verbatim.
    Ordered(&'a str),
    /// Plain text — shown verbatim.
    Plain(&'a str),
}

/// Classifies `line` into a `LineKind`, mutating `in_code` when a code fence
/// is encountered. Called once per line during rendering.
pub fn classify_line<'a>(line: &'a str, in_code: &mut bool) -> LineKind<'a> {
    let fence = line.trim_start().starts_with("```");
    if *in_code || fence {
        if fence {
            *in_code = !*in_code;
        }
        return LineKind::Code(line);
    }

    for (prefix, level) in [
        ("###### ", 6u8),
        ("##### ",  5),
        ("#### ",   4),
        ("### ",    3),
        ("## ",     2),
        ("# ",      1),
    ] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return LineKind::Heading(level, rest);
        }
    }

    if let Some(rest) = line.strip_prefix("> ") {
        return LineKind::Quote(rest);
    }
    if line == ">" {
        return LineKind::Quote("");
    }

    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let trimmed = &line[indent_len..];

    if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        return LineKind::TodoUnchecked(indent, rest);
    }
    if let Some(rest) = trimmed
        .strip_prefix("- [x] ")
        .or_else(|| trimmed.strip_prefix("- [X] "))
    {
        return LineKind::TodoDone(indent, rest);
    }
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        return LineKind::Bullet(indent, rest);
    }

    if crate::model::parser::is_ordered(line) {
        return LineKind::Ordered(line);
    }

    LineKind::Plain(line)
}

/// Maps a heading level (1–6) to its theme colour.
fn heading_color(level: u8, theme: &Theme) -> ratatui::style::Color {
    match level {
        1 => theme.heading1,
        2 => theme.heading2,
        3 => theme.heading3,
        4 => theme.heading4,
        5 => theme.heading5,
        _ => theme.heading6,
    }
}

/// Parse inline formatting markers (`**text**`, `__text__`, `*text*`, `_text_`) in
/// `text`, applying `base_style` to plain spans and the relevant modifier to
/// formatted spans. Returns owned (static) spans so callers have no lifetime
/// constraints on the input string.
pub fn parse_inline_formatting(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = text;

    while !rest.is_empty() {
        let star_pos = rest.find("**");
        let under_pos = rest.find("__");
        let single_star_pos = rest.find('*').filter(|&p| star_pos != Some(p));
        let single_under_pos = rest.find('_').filter(|&p| under_pos != Some(p));

        let candidates = [
            (star_pos, "**", Modifier::BOLD),
            (under_pos, "__", Modifier::BOLD),
            (single_star_pos, "*", Modifier::ITALIC),
            (single_under_pos, "_", Modifier::ITALIC),
        ];
        let Some((start, marker, modifier)) = candidates
            .into_iter()
            .filter_map(|(pos, marker, modifier)| pos.map(|p| (p, marker, modifier)))
            .min_by_key(|(p, _, _)| *p)
        else {
            break;
        };

        let after_open = &rest[start + marker.len()..];
        if let Some(end) = after_open.find(marker) {
            if start > 0 {
                spans.push(Span::styled(rest[..start].to_owned(), base_style));
            }
            let styled_text = after_open[..end].to_owned();
            spans.push(Span::styled(styled_text, base_style.add_modifier(modifier)));
            rest = &after_open[end + marker.len()..];
        } else {
            // Unmatched opening marker — treat remaining text as plain.
            break;
        }
    }

    if !rest.is_empty() {
        spans.push(Span::styled(rest.to_owned(), base_style));
    }

    spans
}

/// Converts a classified `LineKind` into a styled ratatui `Line<'static>`.
/// All string data is cloned to owned, so callers have no lifetime constraints.
pub fn render_line_kind(kind: LineKind<'_>, theme: &Theme) -> Line<'static> {
    match kind {
        LineKind::Code(line) => {
            Line::from(Span::styled(line.to_owned(), Style::default().fg(theme.code)))
        }
        LineKind::Heading(level, text) => Line::from(Span::styled(
            text.to_owned(),
            Style::default()
                .fg(heading_color(level, theme))
                .add_modifier(Modifier::BOLD),
        )),
        LineKind::TodoUnchecked(indent, rest) => {
            let mut spans: Vec<Span<'static>> = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent.to_owned()));
            }
            spans.push(Span::raw("☐ "));
            spans.extend(parse_inline_formatting(rest, Style::default()));
            Line::from(spans)
        }
        LineKind::TodoDone(indent, rest) => {
            let mut spans: Vec<Span<'static>> = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent.to_owned()));
            }
            spans.push(Span::styled("☑ ", Style::default().fg(theme.todo_done)));
            let base_style = Style::default()
                .fg(theme.todo_done)
                .add_modifier(Modifier::CROSSED_OUT);
            spans.extend(parse_inline_formatting(rest, base_style));
            Line::from(spans)
        }
        LineKind::Quote(rest) => {
            let mut spans: Vec<Span<'static>> = vec![Span::styled(
                "│ ",
                Style::default()
                    .fg(theme.quote_marker)
                    .add_modifier(Modifier::ITALIC),
            )];
            spans.extend(parse_inline_formatting(
                rest,
                Style::default().add_modifier(Modifier::ITALIC),
            ));
            Line::from(spans)
        }
        LineKind::Bullet(indent, rest) => {
            let mut spans: Vec<Span<'static>> = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent.to_owned()));
            }
            spans.push(Span::raw("• "));
            spans.extend(parse_inline_formatting(rest, Style::default()));
            Line::from(spans)
        }
        LineKind::Ordered(line) => Line::from(parse_inline_formatting(line, Style::default())),
        LineKind::Plain(line) => Line::from(parse_inline_formatting(line, Style::default())),
    }
}

/// Classify and render a single line in one call. Convenience wrapper for
/// callers that do not need to inspect the `LineKind`.
pub fn render_markdown_line(line: &str, in_code: &mut bool, theme: &Theme) -> Line<'static> {
    render_line_kind(classify_line(line, in_code), theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};

    fn th() -> theme::Theme {
        theme::light()
    }

    // --- classify_line ---

    #[test]
    fn classify_code_fence_opens_block() {
        let mut in_code = false;
        let result = classify_line("```", &mut in_code);
        assert_eq!(result, LineKind::Code("```"));
        assert!(in_code, "fence should open the code block");
    }

    #[test]
    fn classify_code_fence_closes_block() {
        let mut in_code = true;
        let result = classify_line("```", &mut in_code);
        assert_eq!(result, LineKind::Code("```"));
        assert!(!in_code, "second fence should close the code block");
    }

    #[test]
    fn classify_line_inside_code_block() {
        let mut in_code = true;
        let result = classify_line("let x = 1;", &mut in_code);
        assert_eq!(result, LineKind::Code("let x = 1;"));
        assert!(in_code, "in_code should remain true for non-fence lines");
    }

    #[test]
    fn classify_heading1() {
        let mut in_code = false;
        assert_eq!(classify_line("# Title", &mut in_code), LineKind::Heading(1, "Title"));
    }

    #[test]
    fn classify_heading2() {
        let mut in_code = false;
        assert_eq!(classify_line("## Notes", &mut in_code), LineKind::Heading(2, "Notes"));
    }

    #[test]
    fn classify_heading3() {
        let mut in_code = false;
        assert_eq!(classify_line("### Sub", &mut in_code), LineKind::Heading(3, "Sub"));
    }

    #[test]
    fn classify_heading4() {
        let mut in_code = false;
        assert_eq!(classify_line("#### Deep", &mut in_code), LineKind::Heading(4, "Deep"));
    }

    #[test]
    fn classify_heading5() {
        let mut in_code = false;
        assert_eq!(classify_line("##### Five", &mut in_code), LineKind::Heading(5, "Five"));
    }

    #[test]
    fn classify_heading6() {
        let mut in_code = false;
        assert_eq!(classify_line("###### Six", &mut in_code), LineKind::Heading(6, "Six"));
    }

    #[test]
    fn classify_todo_unchecked() {
        let mut in_code = false;
        assert_eq!(classify_line("- [ ] task", &mut in_code), LineKind::TodoUnchecked("", "task"));
    }

    #[test]
    fn classify_todo_unchecked_indented() {
        let mut in_code = false;
        assert_eq!(
            classify_line("  - [ ] task", &mut in_code),
            LineKind::TodoUnchecked("  ", "task")
        );
    }

    #[test]
    fn classify_todo_done_lowercase_x() {
        let mut in_code = false;
        assert_eq!(classify_line("- [x] done", &mut in_code), LineKind::TodoDone("", "done"));
    }

    #[test]
    fn classify_todo_done_uppercase_x() {
        let mut in_code = false;
        assert_eq!(
            classify_line("  - [X] done", &mut in_code),
            LineKind::TodoDone("  ", "done")
        );
    }

    #[test]
    fn classify_quote_with_text() {
        let mut in_code = false;
        assert_eq!(classify_line("> hello", &mut in_code), LineKind::Quote("hello"));
    }

    #[test]
    fn classify_quote_bare() {
        let mut in_code = false;
        assert_eq!(classify_line(">", &mut in_code), LineKind::Quote(""));
    }

    #[test]
    fn classify_bullet_dash() {
        let mut in_code = false;
        assert_eq!(classify_line("- item", &mut in_code), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_star() {
        let mut in_code = false;
        assert_eq!(classify_line("* item", &mut in_code), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_plus() {
        let mut in_code = false;
        assert_eq!(classify_line("+ item", &mut in_code), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_indented() {
        let mut in_code = false;
        assert_eq!(
            classify_line("  - sub", &mut in_code),
            LineKind::Bullet("  ", "sub")
        );
    }

    #[test]
    fn classify_ordered() {
        let mut in_code = false;
        assert_eq!(classify_line("1. first", &mut in_code), LineKind::Ordered("1. first"));
    }

    #[test]
    fn classify_plain() {
        let mut in_code = false;
        assert_eq!(classify_line("just text", &mut in_code), LineKind::Plain("just text"));
    }

    // --- render_line_kind ---

    #[test]
    fn render_code_uses_code_fg() {
        let t = th();
        let line = render_line_kind(LineKind::Code("let x = 1;"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled("let x = 1;", Style::default().fg(t.code)))
        );
    }

    #[test]
    fn render_heading1_bold_heading1_color() {
        let t = th();
        let line = render_line_kind(LineKind::Heading(1, "Title"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled(
                "Title",
                Style::default().fg(t.heading1).add_modifier(Modifier::BOLD)
            ))
        );
    }

    #[test]
    fn render_heading6_bold_heading6_color() {
        let t = th();
        let line = render_line_kind(LineKind::Heading(6, "Six"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled(
                "Six",
                Style::default().fg(t.heading6).add_modifier(Modifier::BOLD)
            ))
        );
    }

    #[test]
    fn render_todo_unchecked_no_indent() {
        let line = render_line_kind(LineKind::TodoUnchecked("", "task"), &th());
        assert_eq!(line, Line::from(vec![Span::raw("☐ "), Span::raw("task")]));
    }

    #[test]
    fn render_todo_unchecked_with_indent() {
        let line = render_line_kind(LineKind::TodoUnchecked("  ", "task"), &th());
        assert_eq!(
            line,
            Line::from(vec![Span::raw("  "), Span::raw("☐ "), Span::raw("task")])
        );
    }

    #[test]
    fn render_todo_done_strikethrough() {
        let t = th();
        let line = render_line_kind(LineKind::TodoDone("", "done"), &t);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled("☑ ", Style::default().fg(t.todo_done)),
                Span::styled(
                    "done",
                    Style::default().fg(t.todo_done).add_modifier(Modifier::CROSSED_OUT)
                ),
            ])
        );
    }

    #[test]
    fn render_quote() {
        let t = th();
        let line = render_line_kind(LineKind::Quote("hello"), &t);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled(
                    "│ ",
                    Style::default().fg(t.quote_marker).add_modifier(Modifier::ITALIC)
                ),
                Span::styled("hello", Style::default().add_modifier(Modifier::ITALIC)),
            ])
        );
    }

    #[test]
    fn render_bullet_no_indent() {
        let line = render_line_kind(LineKind::Bullet("", "item"), &th());
        assert_eq!(line, Line::from(vec![Span::raw("• "), Span::raw("item")]));
    }

    #[test]
    fn render_bullet_with_indent() {
        let line = render_line_kind(LineKind::Bullet("  ", "sub"), &th());
        assert_eq!(
            line,
            Line::from(vec![Span::raw("  "), Span::raw("• "), Span::raw("sub")])
        );
    }

    #[test]
    fn render_ordered_verbatim() {
        let line = render_line_kind(LineKind::Ordered("1. first"), &th());
        assert_eq!(line, Line::from(Span::raw("1. first")));
    }

    #[test]
    fn render_plain_verbatim() {
        let line = render_line_kind(LineKind::Plain("just text"), &th());
        assert_eq!(line, Line::from("just text"));
    }

    // --- parse_inline_formatting ---

    #[test]
    fn parse_inline_formatting_no_markers_returns_plain() {
        let spans = parse_inline_formatting("just text", Style::default());
        assert_eq!(spans, vec![Span::raw("just text")]);
    }

    #[test]
    fn parse_inline_formatting_double_stars() {
        let spans = parse_inline_formatting("hello **world**", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::BOLD)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_double_underscores() {
        let spans = parse_inline_formatting("hello __world__", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::BOLD)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_multiple_markers() {
        let spans = parse_inline_formatting("**a** and **b**", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::styled("a", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" and "),
                Span::styled("b", Style::default().add_modifier(Modifier::BOLD)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_unmatched_marker_is_plain() {
        let spans = parse_inline_formatting("hello **world", Style::default());
        assert_eq!(spans, vec![Span::raw("hello **world")]);
    }

    #[test]
    fn parse_inline_formatting_preserves_base_style() {
        let base = Style::default()
            .fg(ratatui::style::Color::Red)
            .add_modifier(Modifier::ITALIC);
        let spans = parse_inline_formatting("plain **bold**", base);
        assert_eq!(
            spans,
            vec![
                Span::styled("plain ", base),
                Span::styled("bold", base.add_modifier(Modifier::BOLD)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_italic_stars() {
        let spans = parse_inline_formatting("hello *world*", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_italic_underscores() {
        let spans = parse_inline_formatting("hello _world_", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_mixed_bold_and_italic() {
        let spans = parse_inline_formatting("**bold** and *italic*", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::styled("bold", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" and "),
                Span::styled("italic", Style::default().add_modifier(Modifier::ITALIC)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_italic_with_trailing_text() {
        let spans = parse_inline_formatting("hello *world* today", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
                Span::raw(" today"),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_italic_preserves_base_style() {
        let base = Style::default()
            .fg(ratatui::style::Color::Red)
            .add_modifier(Modifier::BOLD);
        let spans = parse_inline_formatting("plain *italic*", base);
        assert_eq!(
            spans,
            vec![
                Span::styled("plain ", base),
                Span::styled("italic", base.add_modifier(Modifier::ITALIC)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_unmatched_italic_is_plain() {
        let spans = parse_inline_formatting("hello *world", Style::default());
        assert_eq!(spans, vec![Span::raw("hello *world")]);
    }

    // --- render_line_kind with inline formatting ---

    #[test]
    fn render_plain_with_inline_bold() {
        let line = render_line_kind(LineKind::Plain("hello **world**"), &th());
        assert_eq!(
            line,
            Line::from(vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::BOLD)),
            ])
        );
    }

    #[test]
    fn render_bullet_with_inline_bold() {
        let line = render_line_kind(LineKind::Bullet("", "hello **world**"), &th());
        assert_eq!(
            line,
            Line::from(vec![
                Span::raw("• "),
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::BOLD)),
            ])
        );
    }

    #[test]
    fn render_todo_unchecked_with_inline_bold() {
        let line = render_line_kind(LineKind::TodoUnchecked("", "hello **world**"), &th());
        assert_eq!(
            line,
            Line::from(vec![
                Span::raw("☐ "),
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::BOLD)),
            ])
        );
    }

    #[test]
    fn render_todo_done_with_inline_bold() {
        let t = th();
        let line = render_line_kind(LineKind::TodoDone("", "hello **world**"), &t);
        let base = Style::default().fg(t.todo_done).add_modifier(Modifier::CROSSED_OUT);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled("☑ ", Style::default().fg(t.todo_done)),
                Span::styled("hello ", base),
                Span::styled("world", base.add_modifier(Modifier::BOLD)),
            ])
        );
    }

    #[test]
    fn render_quote_with_inline_bold() {
        let t = th();
        let line = render_line_kind(LineKind::Quote("hello **world**"), &t);
        let base = Style::default().add_modifier(Modifier::ITALIC);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled(
                    "│ ",
                    Style::default().fg(t.quote_marker).add_modifier(Modifier::ITALIC),
                ),
                Span::styled("hello ", base),
                Span::styled("world", base.add_modifier(Modifier::BOLD)),
            ])
        );
    }

    #[test]
    fn render_ordered_with_inline_bold() {
        let line = render_line_kind(LineKind::Ordered("1. hello **world**"), &th());
        assert_eq!(
            line,
            Line::from(vec![
                Span::raw("1. hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::BOLD)),
            ])
        );
    }
}
```

- [ ] **Step 1.2: Register `markdown` in `src/ui/mod.rs`**

Replace the contents of `src/ui/mod.rs` with:

```rust
mod calendar;
mod capture;
mod chat_panel;
mod document;
pub mod help;
pub mod layout;
pub mod markdown;
pub mod right_panel;
pub mod theme;

pub use layout::render;
```

- [ ] **Step 1.3: Verify the new module compiles and its tests pass**

Run:
```
cargo test --lib ui::markdown
```

Expected output: all tests pass, no compile errors.

---

## Task 2: Refactor `src/ui/document.rs`

**Files:**
- Modify: `src/ui/document.rs`

- [ ] **Step 2.1: Replace the internals of `style_line` and remove duplicated code**

In `document.rs`, delete the following blocks entirely:
1. The `LineKind` enum (lines 12–34)
2. The `classify_line` function (lines 38–106)
3. The `heading_color` function (lines 109–118)
4. The `parse_inline_formatting` function (lines 123–167)
5. The `render_line_kind` function (lines 171–234)

Replace the `style_line` function (lines 242–244) with this new version that handles the two document-specific cases locally and delegates everything else to the shared module:

```rust
/// Converts one document line to a styled ratatui `Line` for display.
///
/// `in_code` tracks whether a code-fence block is active. `vim_cursor` is
/// true when the vim cursor sits on this line — the raw text is returned
/// with a background highlight and `in_code` is reset.
fn style_line(line: &str, in_code: &mut bool, vim_cursor: bool, theme: &Theme) -> Line<'static> {
    use ratatui::text::Span;

    if vim_cursor {
        *in_code = false;
        return Line::from(Span::styled(
            line.to_owned(),
            Style::default().bg(theme.vim_cursor_line),
        ));
    }

    if let Some(rest) = line.strip_prefix("meta:") {
        return Line::from(crate::ui::markdown::parse_inline_formatting(
            rest,
            Style::default()
                .fg(theme.metadata)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    crate::ui::markdown::render_markdown_line(line, in_code, theme)
}
```

Also update the imports at the top of `document.rs` — remove `Span` and `Text` from the `ratatui::text` import since they are no longer used at module level (only locally in the new `style_line`):

```rust
use crate::app::state::AppState;
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
```

The `render` function body (lines 246–316) is unchanged.

- [ ] **Step 2.2: Update the test module in `document.rs`**

The `#[cfg(test)]` block needs to be updated. The tests that tested the now-deleted internal functions (`classify_line`, `render_line_kind`, `parse_inline_formatting`) must be removed from document.rs — they live in `markdown.rs` now.

**Tests to DELETE from document.rs** (they are now in `markdown.rs`):
- `classify_code_fence_opens_block`
- `classify_code_fence_closes_block`
- `classify_line_inside_code_block`
- `classify_heading1` through `classify_heading6`
- `classify_todo_unchecked`, `classify_todo_unchecked_indented`
- `classify_todo_done_lowercase_x`, `classify_todo_done_uppercase_x`
- `classify_quote_with_text`, `classify_quote_bare`
- `classify_bullet_dash`, `classify_bullet_star`, `classify_bullet_plus`, `classify_bullet_indented`
- `classify_ordered`, `classify_plain`
- `render_code_uses_code_fg`
- `render_heading1_bold_heading1_color`, `render_heading6_bold_heading6_color`
- `render_todo_unchecked_no_indent`, `render_todo_unchecked_with_indent`
- `render_todo_done_strikethrough`
- `render_quote`
- `render_bullet_no_indent`, `render_bullet_with_indent`
- `render_ordered_verbatim`, `render_plain_verbatim`
- All `parse_inline_formatting_*` tests
- All `render_*_with_inline_*` tests

**Tests to DELETE** (VimCursor classification internals no longer exist):
- `classify_vim_cursor_returns_vim_cursor_variant`

**Tests to UPDATE** — these test behaviors now handled in `style_line`. Replace their bodies to call `style_line` instead of the deleted internal functions:

Replace `classify_vim_cursor_resets_in_code` with:
```rust
#[test]
fn vim_cursor_resets_in_code() {
    let mut in_code = true;
    style_line("anything", &mut in_code, true, &th());
    assert!(!in_code, "vim_cursor should reset in_code to false");
}
```

Replace `render_vim_cursor_uses_cursor_bg` with:
```rust
#[test]
fn vim_cursor_uses_cursor_bg() {
    let t = th();
    let line = style_line("# raw", &mut false, true, &t);
    assert_eq!(
        line,
        Line::from(ratatui::text::Span::styled(
            "# raw",
            Style::default().bg(t.vim_cursor_line),
        ))
    );
}
```

Replace `classify_meta_field_strips_prefix` with:
```rust
#[test]
fn meta_field_strips_prefix() {
    let mut in_code = false;
    let t = th();
    let line = style_line("meta:Purpose: kick off Q3", &mut in_code, false, &t);
    assert_eq!(
        line,
        Line::from(ratatui::text::Span::styled(
            "Purpose: kick off Q3",
            Style::default().fg(t.metadata).add_modifier(Modifier::ITALIC),
        ))
    );
}
```

Replace `classify_meta_field_scheduled` with:
```rust
#[test]
fn meta_field_scheduled() {
    let mut in_code = false;
    let t = th();
    let line = style_line("meta:Scheduled: 09:00", &mut in_code, false, &t);
    assert_eq!(
        line,
        Line::from(ratatui::text::Span::styled(
            "Scheduled: 09:00",
            Style::default().fg(t.metadata).add_modifier(Modifier::ITALIC),
        ))
    );
}
```

Replace `classify_meta_field_on_cursor_line_shows_raw` with:
```rust
#[test]
fn meta_field_on_cursor_line_shows_raw() {
    let mut in_code = false;
    let t = th();
    let line = style_line("meta:Purpose: kick off Q3", &mut in_code, true, &t);
    assert_eq!(
        line,
        Line::from(ratatui::text::Span::styled(
            "meta:Purpose: kick off Q3",
            Style::default().bg(t.vim_cursor_line),
        ))
    );
}
```

Replace `render_meta_field_italic_metadata_color` with:
```rust
#[test]
fn meta_field_italic_metadata_color() {
    let t = th();
    let line = style_line("meta:Purpose: kick off Q3", &mut false, false, &t);
    assert_eq!(
        line,
        Line::from(ratatui::text::Span::styled(
            "Purpose: kick off Q3",
            Style::default().fg(t.metadata).add_modifier(Modifier::ITALIC),
        ))
    );
}
```

`style_line_meta_field_round_trip` is already testing `style_line` — leave it unchanged.

The test imports at the top of the test module become:
```rust
use super::*;
use crate::ui::theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
```

- [ ] **Step 2.3: Verify the notes panel still works**

Run:
```
cargo test --lib ui::document
```

Expected: all remaining tests pass, no compile errors.

- [ ] **Step 2.4: Run the full test suite**

Run:
```
cargo test
```

Expected: same 567 tests (minus the deleted document.rs tests, plus the new markdown.rs tests) — net count will differ but everything that remains should pass. Zero failures.

- [ ] **Step 2.5: Commit**

```bash
git add src/ui/markdown.rs src/ui/mod.rs src/ui/document.rs
git commit -m "refactor: extract shared markdown.rs module from document.rs"
```

---

## Task 3: Update `src/ui/chat_panel.rs`

**Files:**
- Modify: `src/ui/chat_panel.rs`

- [ ] **Step 3.1: Write the new tests first (they will fail until Step 3.3)**

Add the following tests to the `#[cfg(test)] mod tests` block in `chat_panel.rs`:

```rust
#[test]
fn render_speaker_label_on_own_line() {
    let app = app_with_messages(vec![
        ChatMessage { role: ChatRole::User, content: "hello".into() },
        ChatMessage { role: ChatRole::Assistant, content: "world".into() },
    ]);
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, f.area(), &app, &crate::ui::theme::light())).unwrap();
    let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
    // labels are their own tokens — not prefixed onto message text
    assert!(content.contains("You"), "user label missing: {}", content);
    assert!(content.contains("AI"), "assistant label missing: {}", content);
    // message text appears separately
    assert!(content.contains("hello"), "user message missing: {}", content);
    assert!(content.contains("world"), "assistant message missing: {}", content);
}

#[test]
fn render_markdown_bullet_in_chat() {
    let app = app_with_messages(vec![
        ChatMessage {
            role: ChatRole::Assistant,
            content: "- first\n- second".into(),
        },
    ]);
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
        .unwrap();
    let content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content.contains('•'), "bullet should be rendered as •: {}", content);
}

#[test]
fn render_markdown_heading_in_chat() {
    let app = app_with_messages(vec![
        ChatMessage {
            role: ChatRole::Assistant,
            content: "# Section".into(),
        },
    ]);
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
        .unwrap();
    let content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    // The heading text "Section" should appear; the "# " prefix should be stripped
    assert!(content.contains("Section"), "heading text missing: {}", content);
    assert!(!content.contains("# Section"), "heading markers should be stripped: {}", content);
}
```

Run the tests to confirm they fail (the first one will fail on label format):

```
cargo test --lib ui::chat_panel
```

Expected: `render_speaker_label_on_own_line`, `render_markdown_bullet_in_chat`, and `render_markdown_heading_in_chat` FAIL. Other existing tests pass.

- [ ] **Step 3.2: Replace `wrap_line` with `wrap_text`**

The old `wrap_line` split on `'\n'` and word-wrapped. The new approach splits on `'\n'` in the message rendering loop, so `wrap_text` only needs to word-wrap a single chunk:

Delete the existing `wrap_line` function (lines 8–32) and replace it with:

```rust
/// Word-wrap `text` to fit in `width` columns, splitting on spaces.
/// Returns at least one element even for empty input.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        if current.is_empty() {
            current = word.to_string();
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            out.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }
    out.push(current);
    out
}
```

- [ ] **Step 3.3: Add `render_markdown_wrapped` helper**

First, update the `use ratatui::text::Line;` import at the top of `chat_panel.rs` to:

```rust
use ratatui::text::{Line, Span};
```

Then add this function after `wrap_text` (before the `pub fn render` function):

```rust
/// Render a single raw chat message line as zero or more styled `Line<'static>`,
/// applying markdown classification and word-wrap to fit `width` columns.
fn render_markdown_wrapped(
    raw: &str,
    in_code: &mut bool,
    width: usize,
    theme: &crate::ui::theme::Theme,
) -> Vec<Line<'static>> {
    use crate::ui::markdown::{classify_line, parse_inline_formatting, LineKind};
    use ratatui::style::{Modifier, Style};

    if width == 0 {
        return vec![];
    }

    let kind = classify_line(raw, in_code);
    match kind {
        LineKind::Code(text) => {
            // Code: no word-wrap — preserve as-is.
            vec![Line::from(Span::styled(
                text.to_owned(),
                Style::default().fg(theme.code),
            ))]
        }

        LineKind::Heading(level, text) => {
            let color = match level {
                1 => theme.heading1,
                2 => theme.heading2,
                3 => theme.heading3,
                4 => theme.heading4,
                5 => theme.heading5,
                _ => theme.heading6,
            };
            let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
            wrap_text(text, width)
                .into_iter()
                .map(|seg| Line::from(Span::styled(seg, style)))
                .collect()
        }

        LineKind::Bullet(indent, rest) => {
            let prefix = format!("{}• ", indent);
            let cont = format!("{}  ", indent);
            let prefix_width = indent.chars().count() + 2; // "• " = 2 visible chars
            let avail = width.saturating_sub(prefix_width).max(1);
            wrap_text(rest, avail)
                .into_iter()
                .enumerate()
                .map(|(i, seg)| {
                    let p = if i == 0 { prefix.clone() } else { cont.clone() };
                    let mut spans: Vec<Span<'static>> = vec![Span::raw(p)];
                    spans.extend(parse_inline_formatting(&seg, Style::default()));
                    Line::from(spans)
                })
                .collect()
        }

        LineKind::TodoUnchecked(indent, rest) => {
            let prefix = format!("{}☐ ", indent);
            let cont = format!("{}  ", indent);
            let prefix_width = indent.chars().count() + 2;
            let avail = width.saturating_sub(prefix_width).max(1);
            wrap_text(rest, avail)
                .into_iter()
                .enumerate()
                .map(|(i, seg)| {
                    let p = if i == 0 { prefix.clone() } else { cont.clone() };
                    let mut spans: Vec<Span<'static>> = vec![Span::raw(p)];
                    spans.extend(parse_inline_formatting(&seg, Style::default()));
                    Line::from(spans)
                })
                .collect()
        }

        LineKind::TodoDone(indent, rest) => {
            let prefix = format!("{}☑ ", indent);
            let cont = format!("{}  ", indent);
            let prefix_width = indent.chars().count() + 2;
            let avail = width.saturating_sub(prefix_width).max(1);
            let base_style = Style::default()
                .fg(theme.todo_done)
                .add_modifier(Modifier::CROSSED_OUT);
            wrap_text(rest, avail)
                .into_iter()
                .enumerate()
                .map(|(i, seg)| {
                    let p = if i == 0 { prefix.clone() } else { cont.clone() };
                    let mut spans: Vec<Span<'static>> =
                        vec![Span::styled(p, Style::default().fg(theme.todo_done))];
                    spans.extend(parse_inline_formatting(&seg, base_style));
                    Line::from(spans)
                })
                .collect()
        }

        LineKind::Quote(rest) => {
            let avail = width.saturating_sub(2).max(1); // "│ " = 2 visible chars
            let base = Style::default().add_modifier(Modifier::ITALIC);
            wrap_text(rest, avail)
                .into_iter()
                .map(|seg| {
                    let mut spans: Vec<Span<'static>> = vec![Span::styled(
                        "│ ",
                        Style::default()
                            .fg(theme.quote_marker)
                            .add_modifier(Modifier::ITALIC),
                    )];
                    spans.extend(parse_inline_formatting(&seg, base));
                    Line::from(spans)
                })
                .collect()
        }

        LineKind::Ordered(text) | LineKind::Plain(text) => wrap_text(text, width)
            .into_iter()
            .map(|seg| Line::from(parse_inline_formatting(&seg, Style::default())))
            .collect(),
    }
}
```

- [ ] **Step 3.4: Replace the message rendering loop in `pub fn render`**

In `pub fn render`, find the section that builds `lines` (starting at `let mut lines: Vec<Line> = Vec::new();`). Replace from that line through the `app.chat.pending` block (the block ending at line 89 in the original) with:

```rust
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut msg_iter = app.chat.messages.iter().peekable();
    while let Some(msg) = msg_iter.next() {
        // Speaker label on its own line.
        let label_style = match msg.role {
            ChatRole::User => Style::default().add_modifier(Modifier::DIM),
            ChatRole::Assistant => Style::default(),
        };
        let label = match msg.role {
            ChatRole::User => "You",
            ChatRole::Assistant => "AI",
        };
        lines.push(Line::from(Span::styled(label, label_style)));

        // Render message body with markdown and word-wrap.
        if !msg.content.is_empty() {
            let mut in_code = false;
            for raw in msg.content.split('\n') {
                lines.extend(render_markdown_wrapped(raw, &mut in_code, width, theme));
            }
        }

        // Blank line between messages (not after the last one).
        if msg_iter.peek().is_some() {
            lines.push(Line::raw(""));
        }
    }
    // Thinking indicator while waiting for the first token.
    if app.chat.pending
        && matches!(
            app.chat.messages.last(),
            Some(m) if m.role == ChatRole::Assistant && m.content.is_empty()
        )
    {
        lines.push(Line::raw("…"));
    }
```

- [ ] **Step 3.5: Update the existing `wrap_breaks_on_width` test**

The test currently calls `wrap_line` which is now named `wrap_text`. Update it:

```rust
#[test]
fn wrap_breaks_on_width() {
    let lines = wrap_text("one two three", 7);
    assert_eq!(lines, vec!["one two".to_string(), "three".to_string()]);
}
```

Also delete `wrap_preserves_newlines` — `wrap_text` no longer splits on `'\n'` (that is handled by the message loop). Replace it with a test that confirms wrap_text handles a single chunk:

```rust
#[test]
fn wrap_text_handles_single_word() {
    let lines = wrap_text("hello", 10);
    assert_eq!(lines, vec!["hello".to_string()]);
}
```

- [ ] **Step 3.6: Run all tests and verify they pass**

```
cargo test
```

Expected:
- `render_speaker_label_on_own_line` — PASS
- `render_markdown_bullet_in_chat` — PASS
- `render_markdown_heading_in_chat` — PASS
- `render_shows_message_text` — PASS (content still contains "ping" and "pong")
- `render_shows_thinking_indicator` — PASS
- All other existing tests — PASS
- Zero failures

- [ ] **Step 3.7: Commit**

```bash
git add src/ui/chat_panel.rs
git commit -m "feat: render markdown in chat panel with speaker labels and word-wrap"
```

---

## Self-Review Checklist

| Requirement from spec | Covered by |
|----------------------|-----------|
| Full parity with notes panel markdown | Task 1 creates shared `markdown.rs` with all 8 LineKinds |
| No code duplication | Task 2 removes all duplicated code from document.rs |
| Speaker label on own line | Task 3, Step 3.4 message loop |
| Blank line separator between messages | Task 3, Step 3.4 message loop |
| Word-wrap with bullet indentation | Task 3, Step 3.3 `render_markdown_wrapped` |
| Notes panel behavior unchanged | Task 2 Step 2.3, `cargo test --lib ui::document` |
| Inline bold/italic in chat | `parse_inline_formatting` called in `render_markdown_wrapped` for all applicable kinds |
