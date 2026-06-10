use crate::app::state::{AppState, ChatRole, Focus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Padding, Paragraph};

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

pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState, theme: &crate::ui::theme::Theme) {
    let bg = Block::default()
        .style(Style::default().bg(theme.chat_panel_bg))
        .padding(Padding::new(1, 1, 0, 1));
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

    let header_text = if let Some(ord) = app.chat.meeting_ordinal {
        let meetings = app.doc.meetings();
        if let Some(m) = meetings.get(ord) {
            format!("Chat [{}]", m.name)
        } else {
            "Chat".to_string()
        }
    } else {
        "Chat".to_string()
    };
    let header = Paragraph::new(header_text).style(
        Style::default()
            .bg(theme.chat_panel_bg)
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
            lines.push(Line::styled(wl, style.bg(theme.chat_panel_bg)));
        }
    }
    // Thinking indicator while waiting for the first token.
    if app.chat.pending
        && matches!(
            app.chat.messages.last(),
            Some(m) if m.role == ChatRole::Assistant && m.content.is_empty()
        )
    {
        lines.push(Line::styled("…", Style::default().bg(theme.chat_panel_bg)));
    }

    let height = body_area.height as usize;
    let total = lines.len();
    let max_top = total.saturating_sub(height);
    let scroll = if app.focus == Focus::Chat { app.chat.scroll } else { 0 };
    let top = max_top.saturating_sub(scroll);
    let end = (top + height).min(total);
    let visible: Vec<Line> = if top < end { lines[top..end].to_vec() } else { Vec::new() };
    frame.render_widget(
        Paragraph::new(visible).style(Style::default().bg(theme.chat_panel_bg)),
        body_area,
    );

    if let Some(status) = &app.chat.status {
        let status_widget = Paragraph::new(status.clone())
            .style(Style::default().bg(theme.chat_panel_bg).fg(Color::Red));
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

    #[test]
    fn panel_uses_theme_chat_panel_bg() {
        use ratatui::style::Color;

        // Build a theme with a distinctive chat_panel_bg to prove the theme is used
        let mut overrides = crate::config::ThemeOverrides::default();
        overrides.chat_panel_bg = Some("#010203".to_string());
        let custom_theme = crate::ui::theme::resolve_theme("light", &overrides);

        let app = app_with_messages(vec![]);
        let backend = TestBackend::new(40, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &app, &custom_theme))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_bg = buffer
            .content
            .iter()
            .any(|cell| cell.style().bg == Some(Color::Rgb(1, 2, 3)));
        assert!(has_bg, "expected custom chat_panel_bg color from theme");
    }

    #[test]
    fn render_shows_meeting_name_in_header_when_in_meeting_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let config = crate::config::Config::default();
        let date = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let mut app = AppState::open_day(tmp.path().to_path_buf(), config, date).unwrap();

        // Set up a meeting in the document
        app.doc = crate::model::day::Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n",
        );
        app.chat.visible = true;
        app.chat.meeting_ordinal = Some(0);

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
        assert!(
            content.contains("Chat [Standup]"),
            "header should show meeting name, got: {}",
            content
        );
    }

    #[test]
    fn render_shows_plain_chat_header_outside_meeting_mode() {
        let app = app_with_messages(vec![]);
        // meeting_ordinal is None by default
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
        assert!(content.contains("Chat"), "header should show 'Chat': {}", content);
        assert!(
            !content.contains("Chat ["),
            "should not show meeting bracket outside meeting mode: {}",
            content
        );
    }
}
