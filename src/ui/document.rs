use crate::app::state::AppState;
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

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
        let line_text = app
            .doc
            .lines
            .get(cursor_line)
            .map(|l| l.as_str())
            .unwrap_or("");
        let display_col = line_text[..app.vim.cursor_col.min(line_text.len())]
            .chars()
            .count() as u16;
        let display_row = (cursor_line.saturating_sub(scroll_offset)) as u16;
        if display_row < area.height {
            frame.set_cursor_position((area.x + display_col, area.y + display_row));
        }
    }
}

pub fn render_mode_line(frame: &mut ratatui::Frame, app: &AppState, area: Rect, theme: &Theme) {
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
    let line = Line::from(vec![left, Span::raw(" ".repeat(gap as usize)), right]);
    let widget = ratatui::widgets::Paragraph::new(line);
    frame.render_widget(widget, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Line;

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
                Style::default()
                    .fg(th().heading1)
                    .add_modifier(Modifier::BOLD),
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
                Style::default()
                    .fg(th().heading2)
                    .add_modifier(Modifier::BOLD),
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
                Style::default()
                    .fg(th().heading3)
                    .add_modifier(Modifier::BOLD),
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
                Style::default()
                    .fg(th().heading4)
                    .add_modifier(Modifier::BOLD),
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
                Style::default()
                    .fg(th().heading5)
                    .add_modifier(Modifier::BOLD),
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
                Style::default()
                    .fg(th().heading6)
                    .add_modifier(Modifier::BOLD),
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
        assert_eq!(result, Line::from(vec![Span::raw("• "), Span::raw("item")]));
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

    // --- vim cursor ---

    #[test]
    fn vim_cursor_resets_in_code() {
        let mut in_code = true;
        style_line("anything", &mut in_code, true, &th());
        assert!(!in_code, "vim_cursor should reset in_code to false");
    }

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

    // --- meta fields ---

    #[test]
    fn meta_field_strips_prefix() {
        let mut in_code = false;
        let t = th();
        let line = style_line("meta:Purpose: kick off Q3", &mut in_code, false, &t);
        assert_eq!(
            line,
            Line::from(ratatui::text::Span::styled(
                "Purpose: kick off Q3",
                Style::default()
                    .fg(t.metadata)
                    .add_modifier(Modifier::ITALIC),
            ))
        );
    }

    #[test]
    fn meta_field_scheduled() {
        let mut in_code = false;
        let t = th();
        let line = style_line("meta:Scheduled: 09:00", &mut in_code, false, &t);
        assert_eq!(
            line,
            Line::from(ratatui::text::Span::styled(
                "Scheduled: 09:00",
                Style::default()
                    .fg(t.metadata)
                    .add_modifier(Modifier::ITALIC),
            ))
        );
    }

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

    #[test]
    fn meta_field_italic_metadata_color() {
        let t = th();
        let line = style_line("meta:Purpose: kick off Q3", &mut false, false, &t);
        assert_eq!(
            line,
            Line::from(ratatui::text::Span::styled(
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
}
