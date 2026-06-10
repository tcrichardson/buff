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
    /// Plain text — shown verbatim.
    Plain(&'a str),
}

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
    // Vim mode: anchor follows cursor; keeps cursor near bottom of viewport.
    // Capture mode: anchor is context heading or last inserted line; shown near top.
    let doc_anchor = app.doc_anchor_line;
    let visible_height = area.height as usize;
    let scroll_offset: usize = if vim_active {
        doc_anchor.saturating_sub(visible_height.saturating_sub(1))
    } else {
        doc_anchor.saturating_sub(3)
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
}
