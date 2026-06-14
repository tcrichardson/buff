use crate::app::state::AppState;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

pub fn render_status(
    frame: &mut ratatui::Frame,
    app: &AppState,
    area: Rect,
    theme: &crate::ui::theme::Theme,
) {
    let text = if !app.status.is_empty() {
        Line::from(app.status.as_str())
    } else {
        let context_str = app.context_display.clone();
        let help = "[? help]";
        let total_len = context_str.len() + help.len();
        let spaces = (area.width as usize).saturating_sub(total_len);
        Line::from(vec![
            Span::raw(context_str),
            Span::raw(" ".repeat(spaces)),
            Span::raw(help),
        ])
    };
    let paragraph = Paragraph::new(text).style(Style::default().bg(theme.capture_bg));
    frame.render_widget(paragraph, area);
}

pub fn render_input(
    frame: &mut ratatui::Frame,
    app: &AppState,
    area: Rect,
    theme: &crate::ui::theme::Theme,
) {
    use crate::app::state::Focus;
    use ratatui::text::Text;

    let prefix = if app.editing.is_some() {
        "Edit: › "
    } else {
        "› "
    };
    let block = Block::default().style(Style::default().bg(theme.capture_bg));

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

    let inner_height = area.height;
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

        frame.set_cursor_position(ratatui::layout::Position::new(
            area.x + col as u16,
            area.y + (cursor_row.saturating_sub(overflow)) as u16,
        ));
    }
}
