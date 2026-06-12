use crate::app::state::AppState;
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

/// Classifies a single document line into a rendering variant.
/// Each variant carries exactly the string slices needed for rendering,
/// so `render_line_kind` needs no other context.
#[derive(Debug, PartialEq)]
enum LineKind<'a> {
    /// Vim cursor sits on this line — show raw text with cursor background.
    VimCursor(&'a str),
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
    /// Metadata line stored with `meta:` prefix. Contains the text after stripping `meta:`.
    /// Example: `meta:Purpose: kick off Q3` → `MetaField("Purpose: kick off Q3")`.
    MetaField(&'a str),
    /// Plain text — shown verbatim.
    Plain(&'a str),
}

/// Classifies `line` into a `LineKind`, mutating `in_code` when a code fence
/// is encountered.  Called once per line during rendering.
fn classify_line<'a>(line: &'a str, in_code: &mut bool, vim_cursor: bool) -> LineKind<'a> {
    if vim_cursor {
        *in_code = false;
        return LineKind::VimCursor(line);
    }

    let fence = line.trim_start().starts_with("```");
    if *in_code || fence {
        if fence {
            *in_code = !*in_code;
        }
        return LineKind::Code(line);
    }

    // Try headings longest-prefix-first to avoid `#` matching `##`.
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

    // Blockquote matches the full line (not trimmed — they're not indented here).
    if let Some(rest) = line.strip_prefix("> ") {
        return LineKind::Quote(rest);
    }
    if line == ">" {
        return LineKind::Quote("");
    }

    // For bullets and todos: extract leading whitespace so indented variants
    // render identically to unindented ones.
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

    if let Some(rest) = line.strip_prefix("meta:") {
        return LineKind::MetaField(rest);
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
/// formatted spans (`Modifier::BOLD` or `Modifier::ITALIC`).
fn parse_inline_formatting<'a>(text: &'a str, base_style: Style) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut rest = text;

    while !rest.is_empty() {
        let star_pos = rest.find("**");
        let under_pos = rest.find("__");
        // Ignore a single `*` or `_` that is the first character of a double marker.
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
                spans.push(Span::styled(&rest[..start], base_style));
            }
            let styled_text = &after_open[..end];
            spans.push(Span::styled(styled_text, base_style.add_modifier(modifier)));
            rest = &after_open[end + marker.len()..];
        } else {
            // Unmatched opening marker — treat remaining text as plain.
            break;
        }
    }

    if !rest.is_empty() {
        spans.push(Span::styled(rest, base_style));
    }

    spans
}

/// Converts a classified `LineKind` into a styled ratatui `Line`.
/// No branching logic lives here — only span construction.
fn render_line_kind<'a>(kind: LineKind<'a>, theme: &Theme) -> Line<'a> {
    match kind {
        LineKind::VimCursor(line) => {
            Line::from(Span::styled(line, Style::default().bg(theme.vim_cursor_line)))
        }
        LineKind::Code(line) => {
            Line::from(Span::styled(line, Style::default().fg(theme.code)))
        }
        LineKind::Heading(level, text) => Line::from(Span::styled(
            text,
            Style::default()
                .fg(heading_color(level, theme))
                .add_modifier(Modifier::BOLD),
        )),
        LineKind::TodoUnchecked(indent, rest) => {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::raw("☐ "));
            spans.extend(parse_inline_formatting(rest, Style::default()));
            Line::from(spans)
        }
        LineKind::TodoDone(indent, rest) => {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::styled("☑ ", Style::default().fg(theme.todo_done)));
            let base_style = Style::default()
                .fg(theme.todo_done)
                .add_modifier(Modifier::CROSSED_OUT);
            spans.extend(parse_inline_formatting(rest, base_style));
            Line::from(spans)
        }
        LineKind::Quote(rest) => {
            let mut spans: Vec<Span> = vec![Span::styled(
                "│ ",
                Style::default()
                    .fg(theme.quote_marker)
                    .add_modifier(Modifier::ITALIC),
            )];
            spans.extend(parse_inline_formatting(rest, Style::default().add_modifier(Modifier::ITALIC)));
            Line::from(spans)
        }
        LineKind::Bullet(indent, rest) => {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::raw("• "));
            spans.extend(parse_inline_formatting(rest, Style::default()));
            Line::from(spans)
        }
        LineKind::Ordered(line) => Line::from(parse_inline_formatting(line, Style::default())),
        LineKind::MetaField(rest) => Line::from(parse_inline_formatting(
            rest,
            Style::default()
                .fg(theme.metadata)
                .add_modifier(Modifier::ITALIC),
        )),
        LineKind::Plain(line) => Line::from(parse_inline_formatting(line, Style::default())),
    }
}

/// Converts one document line to a styled ratatui `Line` for display.
///
/// `in_code` tracks whether a code-fence block is active; it is mutated
/// when a fence marker is encountered.  `vim_cursor` is true when the vim
/// cursor sits on this line — in that case the raw text is returned with a
/// background highlight and `in_code` is reset.
fn style_line<'a>(line: &'a str, in_code: &mut bool, vim_cursor: bool, theme: &Theme) -> Line<'a> {
    render_line_kind(classify_line(line, in_code, vim_cursor), theme)
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

    // Scroll: use doc_anchor_line for both vim and capture modes.
    // Both modes: anchor is shown ~3 lines from the top of the viewport so that
    // toggling between Capture and VimNormal keeps the visible note position stable.
    let doc_anchor = app.doc_anchor_line;
    let scroll_offset: usize = doc_anchor.saturating_sub(3);

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

    // --- classify_line tests ---

    #[test]
    fn classify_vim_cursor_returns_vim_cursor_variant() {
        let mut in_code = false;
        let result = classify_line("# My Note", &mut in_code, true);
        assert_eq!(result, LineKind::VimCursor("# My Note"));
        assert!(!in_code, "vim_cursor should reset in_code to false");
    }

    #[test]
    fn classify_vim_cursor_resets_in_code() {
        let mut in_code = true;
        let result = classify_line("anything", &mut in_code, true);
        assert_eq!(result, LineKind::VimCursor("anything"));
        assert!(!in_code);
    }

    #[test]
    fn classify_code_fence_opens_block() {
        let mut in_code = false;
        let result = classify_line("```", &mut in_code, false);
        assert_eq!(result, LineKind::Code("```"));
        assert!(in_code, "fence should open the code block");
    }

    #[test]
    fn classify_code_fence_closes_block() {
        let mut in_code = true;
        let result = classify_line("```", &mut in_code, false);
        assert_eq!(result, LineKind::Code("```"));
        assert!(!in_code, "second fence should close the code block");
    }

    #[test]
    fn classify_line_inside_code_block() {
        let mut in_code = true;
        let result = classify_line("let x = 1;", &mut in_code, false);
        assert_eq!(result, LineKind::Code("let x = 1;"));
        assert!(in_code, "in_code should remain true for non-fence lines");
    }

    #[test]
    fn classify_heading1() {
        let mut in_code = false;
        assert_eq!(classify_line("# Title", &mut in_code, false), LineKind::Heading(1, "Title"));
    }

    #[test]
    fn classify_heading2() {
        let mut in_code = false;
        assert_eq!(classify_line("## Notes", &mut in_code, false), LineKind::Heading(2, "Notes"));
    }

    #[test]
    fn classify_heading3() {
        let mut in_code = false;
        assert_eq!(classify_line("### Sub", &mut in_code, false), LineKind::Heading(3, "Sub"));
    }

    #[test]
    fn classify_heading4() {
        let mut in_code = false;
        assert_eq!(classify_line("#### Deep", &mut in_code, false), LineKind::Heading(4, "Deep"));
    }

    #[test]
    fn classify_heading5() {
        let mut in_code = false;
        assert_eq!(classify_line("##### Five", &mut in_code, false), LineKind::Heading(5, "Five"));
    }

    #[test]
    fn classify_heading6() {
        let mut in_code = false;
        assert_eq!(classify_line("###### Six", &mut in_code, false), LineKind::Heading(6, "Six"));
    }

    #[test]
    fn classify_todo_unchecked() {
        let mut in_code = false;
        assert_eq!(classify_line("- [ ] task", &mut in_code, false), LineKind::TodoUnchecked("", "task"));
    }

    #[test]
    fn classify_todo_unchecked_indented() {
        let mut in_code = false;
        assert_eq!(classify_line("  - [ ] task", &mut in_code, false), LineKind::TodoUnchecked("  ", "task"));
    }

    #[test]
    fn classify_todo_done_lowercase_x() {
        let mut in_code = false;
        assert_eq!(classify_line("- [x] done", &mut in_code, false), LineKind::TodoDone("", "done"));
    }

    #[test]
    fn classify_todo_done_uppercase_x() {
        let mut in_code = false;
        assert_eq!(classify_line("  - [X] done", &mut in_code, false), LineKind::TodoDone("  ", "done"));
    }

    #[test]
    fn classify_quote_with_text() {
        let mut in_code = false;
        assert_eq!(classify_line("> hello", &mut in_code, false), LineKind::Quote("hello"));
    }

    #[test]
    fn classify_quote_bare() {
        let mut in_code = false;
        assert_eq!(classify_line(">", &mut in_code, false), LineKind::Quote(""));
    }

    #[test]
    fn classify_bullet_dash() {
        let mut in_code = false;
        assert_eq!(classify_line("- item", &mut in_code, false), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_star() {
        let mut in_code = false;
        assert_eq!(classify_line("* item", &mut in_code, false), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_plus() {
        let mut in_code = false;
        assert_eq!(classify_line("+ item", &mut in_code, false), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_indented() {
        let mut in_code = false;
        assert_eq!(classify_line("  - sub", &mut in_code, false), LineKind::Bullet("  ", "sub"));
    }

    #[test]
    fn classify_ordered() {
        let mut in_code = false;
        assert_eq!(classify_line("1. first", &mut in_code, false), LineKind::Ordered("1. first"));
    }

    #[test]
    fn classify_plain() {
        let mut in_code = false;
        assert_eq!(classify_line("just text", &mut in_code, false), LineKind::Plain("just text"));
    }

    // --- render_line_kind tests ---

    #[test]
    fn render_vim_cursor_uses_cursor_bg() {
        let t = th();
        let line = render_line_kind(LineKind::VimCursor("# raw"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled("# raw", Style::default().bg(t.vim_cursor_line)))
        );
    }

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

    #[test]
    fn classify_meta_field_strips_prefix() {
        let mut in_code = false;
        let result = classify_line("meta:Purpose: kick off Q3", &mut in_code, false);
        assert_eq!(result, LineKind::MetaField("Purpose: kick off Q3"));
    }

    #[test]
    fn classify_meta_field_scheduled() {
        let mut in_code = false;
        let result = classify_line("meta:Scheduled: 09:00", &mut in_code, false);
        assert_eq!(result, LineKind::MetaField("Scheduled: 09:00"));
    }

    #[test]
    fn classify_meta_field_on_cursor_line_shows_raw() {
        // vim cursor always shows raw text, even for meta: lines
        let mut in_code = false;
        let result = classify_line("meta:Purpose: kick off Q3", &mut in_code, true);
        assert_eq!(result, LineKind::VimCursor("meta:Purpose: kick off Q3"));
    }

    #[test]
    fn render_meta_field_italic_metadata_color() {
        let t = th();
        let line = render_line_kind(LineKind::MetaField("Purpose: kick off Q3"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled(
                "Purpose: kick off Q3",
                Style::default()
                    .fg(t.metadata)
                    .add_modifier(Modifier::ITALIC),
            ))
        );
    }

    #[test]
    fn style_line_meta_field_round_trip() {
        // style_line is the public-facing function — verify end-to-end
        let t = th();
        let mut in_code = false;
        let result = style_line("meta:Started: 09:05", &mut in_code, false, &t);
        assert_eq!(
            result,
            Line::from(Span::styled(
                "Started: 09:05",
                Style::default()
                    .fg(t.metadata)
                    .add_modifier(Modifier::ITALIC),
            ))
        );
    }

    // --- parse_inline_formatting tests ---

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
        let base = Style::default().fg(ratatui::style::Color::Red).add_modifier(Modifier::ITALIC);
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
    fn render_plain_with_inline_bold() {
        let t = th();
        let line = render_line_kind(LineKind::Plain("hello **world**"), &t);
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

    #[test]
    fn render_meta_field_with_inline_bold() {
        let t = th();
        let line = render_line_kind(LineKind::MetaField("hello **world**"), &t);
        let base = Style::default().fg(t.metadata).add_modifier(Modifier::ITALIC);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled("hello ", base),
                Span::styled("world", base.add_modifier(Modifier::BOLD)),
            ])
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
}
