use crate::app::state::{AppState, ChatRole, Focus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Padding, Paragraph};

const PANEL_BG: Color = Color::Rgb(230, 230, 240);

/// Wrap `text` to `width` columns, splitting on spaces and honoring existing newlines.
fn wrap_line(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let mut current = String::new();
        for word in raw.split(' ') {
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
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState, _theme: &crate::ui::theme::Theme) {
    let bg = Block::default()
        .style(Style::default().bg(PANEL_BG))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = bg.inner(area);
    frame.render_widget(bg, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(0),    // body
            Constraint::Length(1), // status
        ])
        .split(inner);

    let header = Paragraph::new("Chat").style(
        Style::default()
            .bg(PANEL_BG)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(header, chunks[0]);

    let body_area = chunks[1];
    let width = body_area.width as usize;

    let mut lines: Vec<Line> = Vec::new();
    for msg in &app.chat.messages {
        let (prefix, style) = match msg.role {
            ChatRole::User => ("You: ", Style::default().add_modifier(Modifier::DIM)),
            ChatRole::Assistant => ("AI:  ", Style::default()),
        };
        let full = format!("{}{}", prefix, msg.content);
        for wl in wrap_line(&full, width) {
            lines.push(Line::styled(wl, style.bg(PANEL_BG)));
        }
    }
    // Thinking indicator while waiting for the first token.
    if app.chat.pending
        && matches!(
            app.chat.messages.last(),
            Some(m) if m.role == ChatRole::Assistant && m.content.is_empty()
        )
    {
        lines.push(Line::styled("…", Style::default().bg(PANEL_BG)));
    }

    let height = body_area.height as usize;
    let total = lines.len();
    let max_top = total.saturating_sub(height);
    let scroll = if app.focus == Focus::Chat { app.chat.scroll } else { 0 };
    let top = max_top.saturating_sub(scroll);
    let end = (top + height).min(total);
    let visible: Vec<Line> = if top < end { lines[top..end].to_vec() } else { Vec::new() };
    frame.render_widget(
        Paragraph::new(visible).style(Style::default().bg(PANEL_BG)),
        body_area,
    );

    if let Some(status) = &app.chat.status {
        let status_widget = Paragraph::new(status.clone())
            .style(Style::default().bg(PANEL_BG).fg(Color::Red));
        frame.render_widget(status_widget, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{AppState, ChatMessage, ChatRole};
    use crate::config::Config;
    use chrono::NaiveDate;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn wrap_breaks_on_width() {
        let lines = wrap_line("one two three", 7);
        assert_eq!(lines, vec!["one two".to_string(), "three".to_string()]);
    }

    #[test]
    fn wrap_preserves_newlines() {
        let lines = wrap_line("a\nb", 10);
        assert_eq!(lines, vec!["a".to_string(), "b".to_string()]);
    }

    fn app_with_messages(messages: Vec<ChatMessage>) -> AppState {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = AppState::open_day(
            tmp.path().to_path_buf(),
            Config::default(),
            NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
        )
        .unwrap();
        s.chat.visible = true;
        s.chat.messages = messages;
        s
    }

    #[test]
    fn render_shows_message_text() {
        let app = app_with_messages(vec![
            ChatMessage { role: ChatRole::User, content: "ping".into() },
            ChatMessage { role: ChatRole::Assistant, content: "pong".into() },
        ]);
        let backend = TestBackend::new(40, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, f.area(), &app, &crate::ui::theme::light())).unwrap();
        let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("ping"), "got: {}", content);
        assert!(content.contains("pong"), "got: {}", content);
        assert!(content.contains("Chat"), "header missing: {}", content);
    }

    #[test]
    fn render_shows_thinking_indicator() {
        let mut app = app_with_messages(vec![
            ChatMessage { role: ChatRole::User, content: "q".into() },
            ChatMessage { role: ChatRole::Assistant, content: String::new() },
        ]);
        app.chat.pending = true;
        let backend = TestBackend::new(40, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, f.area(), &app, &crate::ui::theme::light())).unwrap();
        let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains('…'), "thinking indicator missing: {}", content);
    }
}
