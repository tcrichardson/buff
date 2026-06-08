use crate::app::state::AppState;
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

pub fn render(frame: &mut ratatui::Frame, app: &AppState, area: Rect, theme: &Theme) {
    use crate::app::state::Focus;

    let vim_active = matches!(app.focus, Focus::VimNormal | Focus::VimInsert);
    let cursor_line = app.vim.cursor_line;

    // For Navigate mode (legacy), keep old selected-range highlight
    // (Navigate is gone, but keep the None path for Capture/Chat focus)
    let mut in_code = false;
    let text_lines: Vec<Line> = app
        .doc
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            // Cursor line: render raw in vim modes
            if vim_active && i == cursor_line {
                in_code = false; // reset; raw line shown anyway
                let bg_style = Style::default()
                    .bg(theme.notes_panel_bg); // subtle tint via bg; cursor cell highlighted separately
                return Line::from(Span::styled(line.as_str(), bg_style));
            }

            let fence = line.trim_start().starts_with("```");
            if in_code || fence {
                if fence {
                    in_code = !in_code;
                }
                return Line::from(Span::styled(line.as_str(), Style::default().fg(theme.code)));
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
                Line::from(Span::raw(line.as_str()))
            } else {
                Line::from(line.as_str())
            }
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
        // cursor_col is a byte offset; we need the display column (char count)
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
        _ => return, // no mode line when not in vim mode
    };
    let left = Span::styled(mode_label, Style::default().fg(mode_color));
    let right_text = format!("ln {}/{}", current, total);
    let right = Span::styled(right_text, Style::default().fg(theme.heading6));
    // Pad the middle
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
