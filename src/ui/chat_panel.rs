use crate::app::state::{AppState, ChatRole, Focus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph};

/// Word-wrap a single logical line of `text` to fit in `width` columns,
/// splitting on spaces. Callers must split multi-line input on `'\n'` first.
/// Returns at least one element even for empty input.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
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
    out
}

fn wrap_prefixed(
    prefix: &str,
    cont: &str,
    rest: &str,
    prefix_width: usize,
    width: usize,
    prefix_style: Style,
    content_style: Style,
) -> Vec<Line<'static>> {
    use crate::ui::markdown::parse_inline_formatting;

    let avail = width.saturating_sub(prefix_width).max(1);
    wrap_text(rest, avail)
        .into_iter()
        .enumerate()
        .map(|(i, seg)| {
            let p = if i == 0 {
                prefix.to_owned()
            } else {
                cont.to_owned()
            };
            let mut spans: Vec<Span<'static>> = vec![Span::styled(p, prefix_style)];
            spans.extend(parse_inline_formatting(&seg, content_style));
            Line::from(spans)
        })
        .collect()
}

/// Render a single raw chat message line as zero or more styled `Line<'static>`,
/// applying markdown classification and word-wrap to fit `width` columns.
fn render_markdown_wrapped(
    raw: &str,
    in_code: &mut bool,
    width: usize,
    theme: &crate::ui::theme::Theme,
    base_style: Style,
) -> Vec<Line<'static>> {
    use crate::ui::markdown::{LineKind, classify_line, parse_inline_formatting};
    use ratatui::style::Modifier;

    if width == 0 {
        return vec![];
    }

    let kind = classify_line(raw, in_code);
    match kind {
        LineKind::Code(text) => {
            // Code: no word-wrap — preserve as-is.
            vec![Line::from(Span::styled(
                text.to_owned(),
                base_style.fg(theme.code),
            ))]
        }

        LineKind::Heading(level, text) => {
            let color = crate::ui::markdown::heading_color(level, theme);
            let style = base_style.fg(color).add_modifier(Modifier::BOLD);
            wrap_text(text, width)
                .into_iter()
                .map(|seg| Line::from(Span::styled(seg, style)))
                .collect()
        }

        LineKind::Bullet(indent, rest) => {
            let prefix = format!("{}• ", indent);
            let cont = format!("{}  ", indent);
            let prefix_width = indent.chars().count() + 2; // "• " = 2 visible chars
            wrap_prefixed(
                &prefix,
                &cont,
                rest,
                prefix_width,
                width,
                base_style,
                base_style,
            )
        }

        LineKind::TodoUnchecked(indent, rest) => {
            let prefix = format!("{}☐ ", indent);
            let cont = format!("{}  ", indent);
            let prefix_width = indent.chars().count() + 2;
            wrap_prefixed(
                &prefix,
                &cont,
                rest,
                prefix_width,
                width,
                base_style,
                base_style,
            )
        }

        LineKind::TodoDone(indent, rest) => {
            let prefix = format!("{}☑ ", indent);
            let cont = format!("{}  ", indent);
            let prefix_width = indent.chars().count() + 2;
            let content_style = base_style
                .fg(theme.todo_done)
                .add_modifier(Modifier::CROSSED_OUT);
            wrap_prefixed(
                &prefix,
                &cont,
                rest,
                prefix_width,
                width,
                base_style.fg(theme.todo_done),
                content_style,
            )
        }

        LineKind::Quote(rest) => {
            let avail = width.saturating_sub(2).max(1); // "│ " = 2 visible chars
            let content_style = base_style.add_modifier(Modifier::ITALIC);
            let marker_style = base_style
                .fg(theme.quote_marker)
                .add_modifier(Modifier::ITALIC);
            wrap_text(rest, avail)
                .into_iter()
                .map(|seg| {
                    let mut spans: Vec<Span<'static>> = vec![Span::styled("│ ", marker_style)];
                    spans.extend(parse_inline_formatting(&seg, content_style));
                    Line::from(spans)
                })
                .collect()
        }

        LineKind::Ordered(text) | LineKind::Plain(text) => wrap_text(text, width)
            .into_iter()
            .map(|seg| Line::from(parse_inline_formatting(&seg, base_style)))
            .collect(),
    }
}

pub fn render(
    frame: &mut ratatui::Frame,
    area: Rect,
    app: &AppState,
    theme: &crate::ui::theme::Theme,
) {
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

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut msg_iter = app.chat.messages.iter().peekable();
    while let Some(msg) = msg_iter.next() {
        // Speaker label on its own line.
        let (label, label_style, body_style) = match msg.role {
            ChatRole::User => (
                "You",
                Style::default().add_modifier(Modifier::DIM),
                Style::default().add_modifier(Modifier::DIM),
            ),
            ChatRole::Assistant => ("AI", Style::default(), Style::default()),
        };
        lines.push(Line::from(Span::styled(label, label_style)));

        // Render message body with markdown and word-wrap.
        if !msg.content.is_empty() {
            let mut in_code = false;
            for raw in msg.content.split('\n') {
                lines.extend(render_markdown_wrapped(
                    raw,
                    &mut in_code,
                    width,
                    theme,
                    body_style,
                ));
            }
        }

        // Blank line between messages (not after the last one).
        if msg_iter.peek().is_some() {
            lines.push(Line::raw(""));
        }
    }
    // Thinking indicator while waiting for the first token.
    if app.chat.pending
        && matches!(
            app.chat.messages.last(),
            Some(m) if m.role == ChatRole::Assistant && m.content.is_empty()
        )
    {
        lines.push(Line::raw("…"));
    }

    let height = body_area.height as usize;
    let total = lines.len();
    let max_top = total.saturating_sub(height);
    let scroll = if app.focus == Focus::Chat {
        app.chat.scroll
    } else {
        0
    };
    let top = max_top.saturating_sub(scroll);
    let end = (top + height).min(total);
    let visible: Vec<Line> = if top < end {
        lines[top..end].to_vec()
    } else {
        Vec::new()
    };
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
        let lines = wrap_text("one two three", 7);
        assert_eq!(lines, vec!["one two".to_string(), "three".to_string()]);
    }

    #[test]
    fn wrap_text_handles_single_word() {
        let lines = wrap_text("hello", 10);
        assert_eq!(lines, vec!["hello".to_string()]);
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
    fn render_speaker_label_on_own_line() {
        let app = app_with_messages(vec![
            ChatMessage {
                role: ChatRole::User,
                content: "hello".into(),
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: "world".into(),
            },
        ]);
        let width = 40;
        let backend = TestBackend::new(width, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = buffer
            .content
            .chunks(width as usize)
            .map(|row| {
                row.iter()
                    .map(|c| c.symbol())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect();
        // Label immediately precedes its content; messages separated by a blank row.
        let nonempty_rows: Vec<&String> = rows
            .iter()
            .filter(|r| !r.is_empty() && *r != "Chat")
            .collect();
        assert_eq!(
            nonempty_rows,
            vec!["You", "hello", "AI", "world"],
            "expected ordered labels and content, got: {:?}",
            nonempty_rows
        );
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            !content.contains("You: "),
            "old prefixed format should be gone: {}",
            content
        );
        assert!(
            !content.contains("AI: "),
            "old prefixed format should be gone: {}",
            content
        );
    }

    #[test]
    fn render_user_content_is_dim() {
        fn cell_has_dim_at(buffer: &ratatui::buffer::Buffer, symbol: char) -> bool {
            buffer.content.iter().any(|c| {
                c.symbol() == symbol.to_string() && c.style().add_modifier.contains(Modifier::DIM)
            })
        }

        fn cell_not_dim_at(buffer: &ratatui::buffer::Buffer, symbol: char) -> bool {
            buffer.content.iter().any(|c| {
                c.symbol() == symbol.to_string() && !c.style().add_modifier.contains(Modifier::DIM)
            })
        }

        // Plain user content is dim; plain assistant content is not.
        let app = app_with_messages(vec![
            ChatMessage {
                role: ChatRole::User,
                content: "hello".into(),
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: "world".into(),
            },
        ]);
        let backend = TestBackend::new(40, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(
            cell_has_dim_at(buffer, 'h'),
            "plain user message content should be rendered with DIM modifier"
        );
        assert!(
            cell_not_dim_at(buffer, 'w'),
            "plain assistant message content should not be rendered with DIM modifier"
        );

        // User headings and code blocks are also dim.
        let app = app_with_messages(vec![
            ChatMessage {
                role: ChatRole::User,
                content: "# Heading".into(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: "`code`".into(),
            },
        ]);
        let backend = TestBackend::new(40, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(
            cell_has_dim_at(buffer, 'H'),
            "user heading content should be rendered with DIM modifier"
        );
        assert!(
            cell_has_dim_at(buffer, 'c'),
            "user code content should be rendered with DIM modifier"
        );
    }

    #[test]
    fn render_markdown_bullet_in_chat() {
        let app = app_with_messages(vec![ChatMessage {
            role: ChatRole::Assistant,
            content: "- first\n- second".into(),
        }]);
        let width = 40;
        let backend = TestBackend::new(width, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = buffer
            .content
            .chunks(width as usize)
            .map(|row| {
                row.iter()
                    .map(|c| c.symbol())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .filter(|r| !r.is_empty())
            .collect();
        let first_idx = rows
            .iter()
            .position(|r| r == "• first")
            .expect("first bullet missing");
        let second_idx = rows
            .iter()
            .position(|r| r == "• second")
            .expect("second bullet missing");
        assert!(
            first_idx < second_idx,
            "bullets should appear in order, got: {:?}",
            rows
        );
    }

    #[test]
    fn render_markdown_heading_in_chat() {
        let app = app_with_messages(vec![ChatMessage {
            role: ChatRole::Assistant,
            content: "# Section".into(),
        }]);
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
        // The heading text "Section" should appear; the "# " prefix should be stripped
        assert!(
            content.contains("Section"),
            "heading text missing: {}",
            content
        );
        assert!(
            !content.contains("# Section"),
            "heading markers should be stripped: {}",
            content
        );
    }

    #[test]
    fn render_shows_message_text() {
        let app = app_with_messages(vec![
            ChatMessage {
                role: ChatRole::User,
                content: "ping".into(),
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: "pong".into(),
            },
        ]);
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
        assert!(content.contains("ping"), "got: {}", content);
        assert!(content.contains("pong"), "got: {}", content);
        assert!(content.contains("Chat"), "header missing: {}", content);
    }

    #[test]
    fn render_shows_thinking_indicator() {
        let mut app = app_with_messages(vec![
            ChatMessage {
                role: ChatRole::User,
                content: "q".into(),
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: String::new(),
            },
        ]);
        app.chat.pending = true;
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
            content.contains('…'),
            "thinking indicator missing: {}",
            content
        );
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
        assert!(
            content.contains("Chat"),
            "header should show 'Chat': {}",
            content
        );
        assert!(
            !content.contains("Chat ["),
            "should not show meeting bracket outside meeting mode: {}",
            content
        );
    }
}
