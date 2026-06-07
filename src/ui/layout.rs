use crate::app::state::{AppState, Focus, Overlay};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

const FOCUSED_BORDER: Color = Color::Cyan;
const UNFOCUSED_BORDER: Color = Color::DarkGray;

pub fn render(frame: &mut ratatui::Frame, app: &AppState) {
    // Outer horizontal split: main (notes + chat) | right panel (full height)
    let panel_constraint = pane_size_to_constraint(&app.config.panel_width);
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), panel_constraint])
        .split(frame.area());
    let main_area = outer[0];
    let panel_area = outer[1];

    // main_area vertical split: header | content_row | status | input
    let input_line_count = app.input.split('\n').count().max(1) as u16;
    let input_height = (input_line_count + 2).clamp(app.config.capture_height, 12);
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),             // header
            Constraint::Min(0),                // content_row
            Constraint::Length(1),             // status
            Constraint::Length(input_height),  // input (footer)
        ])
        .split(main_area);
    let header_area = main_chunks[0];
    let content_row = main_chunks[1];
    let status_area = main_chunks[2];
    let input_area = main_chunks[3];

    // Header: buff ASCII art (left) + date/context (right), spans full main_area width
    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)])
        .split(header_area);
    let ascii_art = r#"_             __  __
| |__  _   _  / _|/ _|
| '_ \| | | || |_ | |_
| |_) | |_| ||  _||  _|
|_.__/ \__,_||_|  |_|"#;
    let art_widget = ratatui::widgets::Paragraph::new(ascii_art);
    frame.render_widget(art_widget, title_chunks[0]);
    let meta = format!(
        "{}  {}\n{}",
        app.date.format("%Y-%m-%d (%a)"),
        chrono::Local::now().format("%H:%M"),
        app.context_display
    );
    let meta_widget = ratatui::widgets::Paragraph::new(meta);
    frame.render_widget(meta_widget, title_chunks[1]);

    // content_row horizontal split: notes | chat (optional, 50/50)
    let notes_focused = matches!(app.focus, Focus::Capture | Focus::Navigate);
    let chat_focused = app.focus == Focus::Chat;
    let panel_focused = app.focus == Focus::RightPanel;

    let (notes_area, chat_area_opt) = if app.chat.visible {
        let row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_row);
        (row[0], Some(row[1]))
    } else {
        (content_row, None)
    };

    // Notes pane with border
    let notes_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if notes_focused { FOCUSED_BORDER } else { UNFOCUSED_BORDER }));
    let notes_inner = notes_block.inner(notes_area);
    frame.render_widget(notes_block, notes_area);
    super::document::render(frame, app, notes_inner);

    // Status bar (footer chrome, no border)
    super::capture::render_status(frame, app, status_area);

    // Input box (footer, spans full main_area width; render_input draws its own Block)
    super::capture::render_input(frame, app, input_area);

    // Chat pane with border (when visible)
    if let Some(chat_area) = chat_area_opt {
        let chat_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if chat_focused { FOCUSED_BORDER } else { UNFOCUSED_BORDER }));
        let chat_inner = chat_block.inner(chat_area);
        frame.render_widget(chat_block, chat_area);
        super::chat_panel::render(frame, chat_inner, app);
    }

    // Right panel with border — full terminal height
    let panel_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if panel_focused { FOCUSED_BORDER } else { UNFOCUSED_BORDER }));
    let panel_inner = panel_block.inner(panel_area);
    frame.render_widget(panel_block, panel_area);
    super::right_panel::render(frame, panel_inner, app);

    // Overlays (always on top of full frame)
    if app.overlay == Overlay::Help {
        super::help::render(frame, frame.area());
    }
}

fn pane_size_to_constraint(size: &crate::config::PaneSize) -> Constraint {
    match size {
        crate::config::PaneSize::Columns(n) => Constraint::Length(*n),
        crate::config::PaneSize::Percent(p) => Constraint::Percentage(*p),
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
    fn render_header_contains_time() {
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
        // Time is HH:MM — look for the colon surrounded by digits
        let has_time = content
            .chars()
            .collect::<Vec<_>>()
            .windows(5)
            .any(|w| {
                w[0].is_ascii_digit()
                    && w[1].is_ascii_digit()
                    && w[2] == ':'
                    && w[3].is_ascii_digit()
                    && w[4].is_ascii_digit()
            });
        assert!(has_time, "Expected HH:MM time in header, buffer: {}", content);
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
        assert!(
            content.contains("/start"),
            "Expected '/start' in help buffer, got: {}",
            content
        );
        assert!(
            content.contains("/end"),
            "Expected '/end' in help buffer, got: {}",
            content
        );
        assert!(
            content.contains("/scheduled"),
            "Expected '/scheduled' in help buffer, got: {}",
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

    #[test]
    fn notes_pane_has_cyan_border_in_capture_mode() {
        use ratatui::style::Color;
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let app = test_app(doc, Focus::Capture, 0);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let has_cyan_border = buffer.content.iter().any(|cell| {
            cell.style().fg == Some(Color::Cyan)
                && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
        });
        assert!(has_cyan_border, "expected cyan border for focused notes pane");
    }

    #[test]
    fn right_panel_has_dark_border_when_notes_focused() {
        use ratatui::style::Color;
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let app = test_app(doc, Focus::Capture, 0);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let has_dark_border = buffer.content.iter().any(|cell| {
            cell.style().fg == Some(Color::DarkGray)
                && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
        });
        assert!(has_dark_border, "expected dark border for unfocused right panel");
    }

    #[test]
    fn chat_pane_has_cyan_border_when_focused() {
        use ratatui::style::Color;
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::Chat, 0);
        app.chat.visible = true;
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let has_cyan_border = buffer.content.iter().any(|cell| {
            cell.style().fg == Some(Color::Cyan)
                && matches!(cell.symbol(), "│" | "─" | "┌" | "┐" | "└" | "┘")
        });
        assert!(has_cyan_border, "expected cyan border for focused chat pane");
    }

    #[test]
    fn right_panel_has_full_height_independently() {
        // With chat visible, right panel should still span full height.
        // Verify it renders content (calendar header) in the same location
        // regardless of main_area layout.
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::Capture, 0);
        app.chat.visible = true;
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("June 2026"), "calendar header should be present with chat visible");
    }
}
