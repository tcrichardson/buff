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
