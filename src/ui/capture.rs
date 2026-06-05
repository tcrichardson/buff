use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::app::state::AppState;

pub fn render_status(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
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
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, area);
}

pub fn render_input(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    let (title, prefix) = if app.editing.is_some() {
        ("Edit", "Edit: › ")
    } else {
        ("Capture", "› ")
    };
    let block = Block::default().title(title).borders(Borders::ALL);
    let text = format!("{}{}", prefix, app.input);
    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);

    let inner_x = area.x + 1;
    let inner_y = area.y + 1;
    let cursor_x = inner_x + prefix.chars().count() as u16 + app.input.chars().count() as u16;
    let cursor_y = inner_y;
    frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, cursor_y));
}
