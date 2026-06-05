use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::app::state::{AppState, Context};

pub fn render_status(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    let text = if !app.status.is_empty() {
        Line::from(app.status.as_str())
    } else {
        let context_str = match app.context {
            Context::Notes => "context: Notes".to_string(),
            Context::Meeting(ord) => {
                let meetings = app.doc.meetings();
                match meetings.get(ord) {
                    Some(m) => format!("context: {}", m.name),
                    None => "context: Notes".to_string(),
                }
            }
        };
        let help = "[? help]";
        let total_len = context_str.len() + help.len();
        let spaces = (area.width as usize).saturating_sub(total_len);
        Line::from(vec![
            ratatui::text::Span::raw(context_str),
            ratatui::text::Span::raw(" ".repeat(spaces)),
            ratatui::text::Span::raw(help),
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
