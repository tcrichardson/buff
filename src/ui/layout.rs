use crate::app::state::{AppState, Overlay};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Block;

pub fn render(frame: &mut ratatui::Frame, app: &AppState, theme: &crate::ui::theme::Theme) {
    // Paint the terminal canvas with the theme's base background and foreground.
    // All panel widgets render on top of this layer.
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.terminal_bg).fg(theme.terminal_fg)),
        frame.area(),
    );

    // Full-width outer vertical split: header | middle (panels) | status | input
    let input_line_count = app.input.split('\n').count().max(1) as u16;
    let input_height = (input_line_count + 2).clamp(app.config.capture_height, 12);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),            // header (5 rows + 1 padding line at bottom)
            Constraint::Min(0),               // middle row (notes + chat + right panel)
            Constraint::Length(1),            // status bar
            Constraint::Length(input_height), // capture input (footer)
        ])
        .split(frame.area());
    let header_area = outer[0];
    let middle_row = outer[1];
    let status_area = outer[2];
    let input_area = outer[3];

    // middle_row horizontal split: notes+chat area | right panel
    let panel_constraint = pane_size_to_constraint(&app.config.panel_width);
    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1), panel_constraint])
        .split(middle_row);
    let content_row = middle_chunks[0];
    let panel_area = middle_chunks[2];

    // Header: buff ASCII art (left) + date/context (right), spans full main_area width
    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)])
        .split(header_area);
    let ascii_art = r#" _             __  __
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

    // content_row horizontal split: notes | gap | chat (optional, equal halves)
    let (notes_area, chat_area_opt) = if app.chat.visible {
        let row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(content_row);
        (row[0], Some(row[2]))
    } else {
        (content_row, None)
    };

    // Notes pane (no border)
    let notes_block = Block::default().style(Style::default().bg(theme.notes_panel_bg));
    frame.render_widget(notes_block, notes_area);

    let notes_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(0),
            ratatui::layout::Constraint::Length(1),
        ])
        .split(notes_area);
    let notes_label_area = notes_layout[0];
    let notes_content_area = notes_layout[1];
    let notes_mode_area = notes_layout[2];

    let notes_label = ratatui::widgets::Paragraph::new(" Notes ").style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .bg(theme.notes_panel_bg),
    );
    frame.render_widget(notes_label, notes_label_area);

    super::document::render(frame, app, notes_content_area, theme);
    super::document::render_mode_line(frame, app, notes_mode_area, theme);

    // Status bar (footer chrome, no border)
    super::capture::render_status(frame, app, status_area, theme);

    // Input box (footer, spans full main_area width)
    super::capture::render_input(frame, app, input_area, theme);

    // Chat pane (no border)
    if let Some(chat_area) = chat_area_opt {
        super::chat_panel::render(frame, chat_area, app, theme);
    }

    // Right panel (no border)
    super::right_panel::render(frame, panel_area, app, theme);

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
            dates_with_notes: std::collections::BTreeSet::new(),
            right_panel_selected: 0,
            right_panel_scroll: 0,
            doc_anchor_line: 0,
            panel_todos: Vec::new(),
            panel_agenda: Vec::new(),
            chat: crate::app::state::ChatState::default(),
            vim: crate::app::state::VimState::default(),
        }
    }

    fn test_theme() -> crate::ui::theme::Theme {
        crate::ui::theme::light()
    }

    #[test]
    fn render_empty_day() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let app = test_app(doc, Focus::Capture, 0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app, &test_theme());
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
                render(frame, &app, &test_theme());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        // Time is HH:MM — look for the colon surrounded by digits
        let has_time = content.chars().collect::<Vec<_>>().windows(5).any(|w| {
            w[0].is_ascii_digit()
                && w[1].is_ascii_digit()
                && w[2] == ':'
                && w[3].is_ascii_digit()
                && w[4].is_ascii_digit()
        });
        assert!(
            has_time,
            "Expected HH:MM time in header, buffer: {}",
            content
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
                render(frame, &app, &test_theme());
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

        let app = test_app(doc, Focus::VimNormal, 1);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app, &test_theme());
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
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();

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
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();

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
        let mut app = test_app(doc, Focus::VimNormal, 0);
        app.overlay = Overlay::Help;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, &app, &test_theme());
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
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();
    }

    #[test]
    fn render_quote_and_code_and_numbered() {
        let doc = Document::from_text(
            "# Day\n\n## Notes\n\n> a quote\n1. first item\n```\ncode line\n```\n\n## To-dos\n",
        );
        let app = test_app(doc, Focus::Capture, 0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();

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
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();
        let content: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            content.contains("paneltext"),
            "chat text missing: {}",
            content
        );
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
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();
        let content: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            !content.contains("paneltext"),
            "chat should be hidden: {}",
            content
        );
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
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("June 2026"),
            "calendar header should be present with chat visible"
        );
    }

    #[test]
    fn notes_pane_title_is_notes() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let app = test_app(doc, Focus::Capture, 0);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Notes"),
            "Expected 'Notes' block title, got: {}",
            content
        );
    }

    #[test]
    fn render_h4_h5_h6_headings() {
        let doc = Document::from_text("#### Level 4\n##### Level 5\n###### Level 6\n");
        let app = test_app(doc, Focus::Capture, 0);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        // h4 color = Color::Rgb(49, 130, 206) in light theme
        let has_h4_color = buffer
            .content
            .iter()
            .any(|cell| cell.style().fg == Some(ratatui::style::Color::Rgb(49, 130, 206)));
        assert!(has_h4_color, "expected h4 heading color applied");
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Level 4"), "h4 text missing: {}", content);
        assert!(content.contains("Level 5"), "h5 text missing: {}", content);
        assert!(content.contains("Level 6"), "h6 text missing: {}", content);
    }

    #[test]
    fn render_h1_uses_theme_color() {
        let doc = Document::from_text("# My Heading\n");
        let app = test_app(doc, Focus::Capture, 0);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        // light theme h1 = Color::Rgb(26, 54, 93)
        let has_h1_color = buffer.content.iter().any(|cell| {
            cell.style().fg == Some(ratatui::style::Color::Rgb(26, 54, 93))
                && cell
                    .style()
                    .add_modifier
                    .contains(ratatui::style::Modifier::BOLD)
        });
        assert!(has_h1_color, "expected h1 theme color with BOLD");
    }

    #[test]
    fn render_vim_normal_shows_mode_line() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::VimNormal, 0);
        app.vim.cursor_line = 0;

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("NORMAL"),
            "Expected NORMAL in mode line, got: {}",
            content
        );
    }

    #[test]
    fn render_vim_insert_shows_insert_mode_line() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::VimInsert, 0);
        app.vim.cursor_line = 0;

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("INSERT"),
            "Expected INSERT in mode line, got: {}",
            content
        );
    }

    #[test]
    fn render_vim_normal_cursor_line_uses_vim_cursor_line_bg() {
        use ratatui::style::Color;
        let doc = Document::from_text("cursor line\nother line\n");
        let mut app = test_app(doc, Focus::VimNormal, 0);
        app.vim.cursor_line = 0;

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app, &test_theme()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        // light theme vim_cursor_line = Color::Rgb(224, 231, 255)
        let has_highlight = buffer
            .content
            .iter()
            .any(|cell| cell.style().bg == Some(Color::Rgb(224, 231, 255)));
        assert!(
            has_highlight,
            "expected vim_cursor_line background on the cursor line, got none"
        );
    }

    #[test]
    fn render_terminal_bg_paints_canvas() {
        use ratatui::style::Color;
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let app = test_app(doc, Focus::Capture, 0);

        // Build a custom theme with a distinctive terminal_bg and terminal_fg.
        let mut theme = crate::ui::theme::light();
        theme.terminal_bg = Color::Rgb(99, 0, 99);
        theme.terminal_fg = Color::Rgb(200, 200, 0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app, &theme)).unwrap();

        let buffer = terminal.backend().buffer();
        // At least one cell should carry the terminal_bg as its background.
        let has_canvas_bg = buffer
            .content
            .iter()
            .any(|cell| cell.style().bg == Some(Color::Rgb(99, 0, 99)));
        assert!(
            has_canvas_bg,
            "expected terminal_bg to appear in at least one canvas cell"
        );
    }
}
