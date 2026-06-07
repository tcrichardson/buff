use crate::app::state::{AppState, Overlay};

pub fn render(frame: &mut ratatui::Frame, app: &AppState) {
    use ratatui::layout::{Constraint, Direction, Layout};

    // Outer horizontal split: left = doc+chrome, [chat], right = panel
    let panel_width = match app.config.panel_width {
        crate::config::PaneSize::Columns(n) => n,
        crate::config::PaneSize::Percent(p) => {
            let total = frame.area().width;
            (total as u16 * p / 100).max(10)
        }
    };
    let chat_width = 30; // temporary until Task 4 replaces layout
    let (left_area, chat_area, panel_area) = if app.chat.visible {
        let outer = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(chat_width),
                Constraint::Length(panel_width),
            ])
            .split(frame.area());
        (outer[0], Some(outer[1]), outer[2])
    } else {
        let outer = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(panel_width)])
            .split(frame.area());
        (outer[0], None, outer[1])
    };

    // Left column: existing vertical stack
    let input_line_count = app.input.split('\n').count().max(1) as u16;
    let input_height = (input_line_count + 2).clamp(app.config.capture_height, 12);
    let title_height = 5u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(title_height),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(input_height),
        ])
        .split(left_area);

    let title_area = chunks[0];
    let document_area = chunks[1];
    let status_area = chunks[2];
    let input_area = chunks[3];

    // Split title area into left (ASCII art) and right (date + context)
    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)])
        .split(title_area);

    let ascii_art = r#"_             __  __
| |__  _   _  / _|/ _|
| '_ \| | | || |_ | |_
| |_) | |_| ||  _||  _|
|_.__/ \__,_||_|  |_|"#;
    let art_widget = ratatui::widgets::Paragraph::new(ascii_art);
    frame.render_widget(art_widget, title_chunks[0]);

    let meta = format!(
        "{}\n{}",
        app.date.format("%Y-%m-%d (%a)"),
        app.context_display
    );
    let meta_widget = ratatui::widgets::Paragraph::new(meta);
    frame.render_widget(meta_widget, title_chunks[1]);

    super::document::render(frame, app, document_area);
    super::capture::render_status(frame, app, status_area);
    super::capture::render_input(frame, app, input_area);

    // Chat panel (middle column), when visible
    if let Some(chat_area) = chat_area {
        super::chat_panel::render(frame, chat_area, app);
    }

    // Right panel
    super::right_panel::render(frame, panel_area, app);

    // Overlays
    if app.overlay == Overlay::Help {
        super::help::render(frame, frame.area());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{AppState, Context, Focus, Overlay};
    use crate::config::Config;
    use crate::model::day::Document;
    use chrono::NaiveDate;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;
    use std::path::PathBuf;

    fn test_app(doc: Document, focus: Focus, selected: usize) -> AppState {
        let selectables = doc.selectables();
        AppState {
            doc,
            date: NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
            notes_dir: PathBuf::from("/tmp"),
            config: Config::default(),
            context: Context::Notes,
            focus,
            selected,
            status: String::new(),
            input: String::new(),
            cursor_pos: 0,
            overlay: Overlay::None,
            editing: None,
            should_quit: false,
            selectables,
            context_display: "context: Notes".to_string(),
            pending_delete: false,
            dates_with_notes: std::collections::BTreeSet::new(),
            right_panel_selected: 0,
            right_panel_scroll: 0,
            panel_todos: Vec::new(),
            chat: crate::app::state::ChatState::default(),
        }
    }

    #[test]
    fn render_empty_day() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let app = test_app(doc, Focus::Capture, 0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("| |__"), "Expected ASCII art in buffer");
        assert!(content.contains("2026-06-04"), "Expected date in buffer");
        assert!(
            content.contains("context: Notes"),
            "Expected context in buffer"
        );
    }

    #[test]
    fn render_populated_day() {
        let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        doc.add_todo("Test todo", None);
        doc.add_todo("Done todo", None);
        doc.toggle_todo(1).unwrap();

        let app = test_app(doc, Focus::Capture, 0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains('☐'), "Expected unchecked box in buffer");
        assert!(content.contains('☑'), "Expected checked box in buffer");
    }

    #[test]
    fn render_navigate_mode() {
        let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        doc.add_todo("First", None);
        doc.add_todo("Second", None);

        let app = test_app(doc, Focus::Navigate, 1);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_reversed = buffer
            .content
            .iter()
            .any(|cell| cell.style().add_modifier.contains(Modifier::REVERSED));
        assert!(
            has_reversed,
            "Expected at least one cell with REVERSED modifier in navigate mode"
        );
    }

    #[test]
    fn render_multiline_input() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::Capture, 0);
        app.input = "line one\nline two".to_string();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("line one"),
            "first line missing: {}",
            content
        );
        assert!(
            content.contains("line two"),
            "second line missing: {}",
            content
        );
    }

    #[test]
    fn render_multiline_input_scrolls_when_too_tall() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::Capture, 0);
        // 15 lines — more than the 10-line inner height
        app.input = (1..=15)
            .map(|n| format!("line {}", n))
            .collect::<Vec<_>>()
            .join("\n");

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        // The last visible line should be present (scrolled into view)
        assert!(
            content.contains("line 15"),
            "last line should be visible after scroll: {}",
            content
        );
    }

    #[test]
    fn render_help_overlay() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::Navigate, 0);
        app.overlay = Overlay::Help;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("/meeting"),
            "Expected '/meeting' in buffer, got: {}",
            content
        );
        assert!(
            content.contains("/ask"),
            "Expected '/ask' in help buffer, got: {}",
            content
        );
    }

    #[test]
    fn right_panel_column_present_in_layout() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let app = test_app(doc, Focus::Capture, 0);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
    }

    #[test]
    fn render_quote_and_code_and_numbered() {
        let doc = Document::from_text(
            "# Day\n\n## Notes\n\n> a quote\n1. first item\n```\ncode line\n```\n\n## To-dos\n",
        );
        let app = test_app(doc, Focus::Capture, 0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("a quote"),
            "quote text missing: {}",
            content
        );
        assert!(
            content.contains("first item"),
            "numbered text missing: {}",
            content
        );
        assert!(
            content.contains("code line"),
            "code text missing: {}",
            content
        );
    }

    #[test]
    fn chat_panel_renders_when_visible() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::Capture, 0);
        app.chat.visible = true;
        app.chat.messages = vec![crate::app::state::ChatMessage {
            role: crate::app::state::ChatRole::Assistant,
            content: "paneltext".to_string(),
        }];

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("paneltext"), "chat text missing: {}", content);
    }

    #[test]
    fn chat_panel_hidden_does_not_render_chat() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::Capture, 0);
        app.chat.visible = false;
        app.chat.messages = vec![crate::app::state::ChatMessage {
            role: crate::app::state::ChatRole::Assistant,
            content: "paneltext".to_string(),
        }];

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let content: String = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect();
        assert!(!content.contains("paneltext"), "chat should be hidden: {}", content);
    }
}
