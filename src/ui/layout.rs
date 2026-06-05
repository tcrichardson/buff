use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::Paragraph;
use crate::app::state::{AppState, Overlay};

pub fn render(frame: &mut ratatui::Frame, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let title_area = chunks[0];
    let document_area = chunks[1];
    let status_area = chunks[2];
    let input_area = chunks[3];

    let title = format!("Kua-Tin — {}", app.date.format("%Y-%m-%d (%a)"));
    let title_widget = Paragraph::new(title);
    frame.render_widget(title_widget, title_area);

    super::document::render(frame, app, document_area);
    super::capture::render_status(frame, app, status_area);
    super::capture::render_input(frame, app, input_area);

    match app.overlay {
        Overlay::Calendar => {
            super::calendar::render(frame, app, frame.area());
        }
        Overlay::Help => {
            super::help::render(frame, frame.area());
        }
        Overlay::None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;
    use ratatui::Terminal;
    use crate::app::state::{AppState, Context, Focus, Overlay};
    use crate::config::Config;
    use crate::model::day::Document;
    use chrono::NaiveDate;
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
            overlay: Overlay::None,
            editing: None,
            should_quit: false,
            selectables,
            context_display: "context: Notes".to_string(),
            pending_delete: false,
            calendar: None,
            dates_with_notes: std::collections::BTreeSet::new(),
        }
    }

    #[test]
    fn render_empty_day() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let app = test_app(doc, Focus::Capture, 0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| {
            render(frame, &app);
        }).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Kua-Tin"), "Expected 'Kua-Tin' in buffer");
        assert!(content.contains("2026-06-04"), "Expected date in buffer");
        assert!(content.contains("context: Notes"), "Expected context in buffer");
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
        terminal.draw(|frame| {
            render(frame, &app);
        }).unwrap();

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
        terminal.draw(|frame| {
            render(frame, &app);
        }).unwrap();

        let buffer = terminal.backend().buffer();
        let has_reversed = buffer.content.iter().any(|cell| {
            cell.style().add_modifier.contains(Modifier::REVERSED)
        });
        assert!(
            has_reversed,
            "Expected at least one cell with REVERSED modifier in navigate mode"
        );
    }

    #[test]
    fn render_calendar_overlay() {
        let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let mut app = test_app(doc, Focus::Navigate, 0);
        app.overlay = Overlay::Calendar;
        app.calendar = Some(crate::ui::calendar::CalendarState::new(app.date));

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| {
            render(frame, &app);
        }).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("June 2026"), "Expected 'June 2026' in buffer, got: {}", content);
    }
}
