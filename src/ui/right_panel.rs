use crate::app::state::{AppState, Focus};
use crate::config::{Config, WeekStart};
use crate::model::day::{Document, SelectableKind};
use crate::storage;
use crate::ui::calendar;
use chrono::{Datelike, NaiveDate};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Padding, Paragraph, Row, Table};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelTodo {
    pub date: NaiveDate,
    pub text: String,
    pub todo_index: usize, // index into that day's Document::selectables()
}

/// Strip "- [ ] " or "- [x] " prefix and any trailing " _(Tag)_" meeting tag.
/// Returns the first line only (multi-line todos are shown truncated).
fn display_text(raw: &str) -> String {
    let stripped = raw
        .strip_prefix("- [ ] ")
        .or_else(|| raw.strip_prefix("- [x] "))
        .or_else(|| raw.strip_prefix("- [X] "))
        .unwrap_or(raw);
    let first_line = stripped.lines().next().unwrap_or(stripped);
    // Strip trailing " _(Tag)_"
    if let Some(tag_start) = first_line.rfind(" _(") {
        if first_line.ends_with(")_") {
            return first_line[..tag_start].to_string();
        }
    }
    first_line.to_string()
}

/// Collect all incomplete todos from the last `config.todo_lookback_days` days
/// (including `date` itself), most-recent-day first.
pub fn collect_panel_todos(notes_dir: &Path, date: NaiveDate, config: &Config) -> Vec<PanelTodo> {
    let mut todos = Vec::new();
    for offset in 0..config.todo_lookback_days {
        let day = date - chrono::Duration::days(offset as i64);
        let path = storage::path_for(notes_dir, day, &config.date_format);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let doc = Document::from_text(&text);
        for (sel_index, sel) in doc.selectables().iter().enumerate() {
            if matches!(sel.kind, SelectableKind::Todo { done: false }) {
                todos.push(PanelTodo {
                    date: day,
                    text: display_text(&sel.text),
                    todo_index: sel_index,
                });
            }
        }
    }
    todos
}

const PANEL_BG: Color = Color::Rgb(220, 220, 220);

pub fn render(frame: &mut ratatui::Frame, area: Rect, app: &AppState) {
    // Fill panel background with padding inset
    let bg_block = Block::default()
        .style(Style::default().bg(PANEL_BG))
        .padding(Padding::new(2, 2, 2, 2));
    let inner = bg_block.inner(area);
    frame.render_widget(bg_block, area);

    // Split the inner area: calendar top (fixed 9 lines) + todo list (rest)
    let calendar_height = 9u16; // header(1) + day-names(1) + weeks grid(7)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(calendar_height), Constraint::Min(0)])
        .split(inner);

    render_calendar(frame, chunks[0], app);
    render_todo_list(frame, chunks[1], app);
}

fn render_calendar(frame: &mut ratatui::Frame, area: Rect, app: &AppState) {
    let visible_month = (app.date.year(), app.date.month());
    let weeks_grid = calendar::weeks(visible_month, app.config.week_starts_on);
    let dates_with_notes = &app.dates_with_notes;

    let (year, month) = visible_month;
    let month_name = NaiveDate::from_ymd_opt(year, month, 1)
        .map(|d| d.format("%B %Y").to_string())
        .unwrap_or_default();

    // Split calendar area: header (1) + day-names (1) + weeks (rest)
    let cal_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    let header_sub = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(21), Constraint::Min(0)])
        .split(cal_chunks[0]);
    let header_widget = Paragraph::new(month_name)
        .alignment(Alignment::Center)
        .style(Style::default().bg(PANEL_BG));
    frame.render_widget(header_widget, header_sub[0]);

    let day_names: Vec<&str> = match app.config.week_starts_on {
        WeekStart::Sunday => vec!["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
        WeekStart::Monday => vec!["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    };
    let names_row = Row::new(day_names);
    let names_table = Table::new(vec![names_row], [Constraint::Length(3); 7])
        .style(Style::default().bg(PANEL_BG));
    frame.render_widget(names_table, cal_chunks[1]);

    let mut rows = Vec::new();
    for week in &weeks_grid {
        let mut cells = Vec::new();
        for day_opt in week {
            match day_opt {
                Some(date) => {
                    let is_today = *date == app.date;
                    let has_note = calendar::marked(*date, dates_with_notes);
                    let marker = if has_note { "·" } else { " " };
                    let text = format!("{:>2}{}", date.day(), marker);
                    let style = if is_today {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    cells.push(Cell::from(text).style(style));
                }
                None => {
                    cells.push(Cell::from("   "));
                }
            }
        }
        rows.push(Row::new(cells));
    }

    let table = Table::new(rows, [Constraint::Length(3); 7])
        .style(Style::default().bg(PANEL_BG));
    frame.render_widget(table, cal_chunks[2]);
}

fn render_todo_list(frame: &mut ratatui::Frame, area: Rect, app: &AppState) {
    let mut virtual_lines: Vec<Line> = Vec::new();

    virtual_lines.push(Line::from("To-dos"));

    let mut current_date = None;
    for (flat_idx, todo) in app.panel_todos.iter().enumerate() {
        if Some(todo.date) != current_date {
            current_date = Some(todo.date);
            let header = todo.date.format("%a %b %d").to_string();
            virtual_lines.push(Line::from(format!("─ {} ", header)));
        }
        let is_selected =
            app.focus == Focus::RightPanel && flat_idx == app.right_panel_selected;
        let item_text = format!("☐ {}", todo.text);
        let style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        virtual_lines.push(Line::styled(item_text, style));
    }

    let scroll = app
        .right_panel_scroll
        .min(virtual_lines.len().saturating_sub(1));
    let visible: Vec<Line> = virtual_lines
        .into_iter()
        .skip(scroll)
        .take(area.height as usize)
        .collect();

    let widget = Paragraph::new(visible).style(Style::default().bg(PANEL_BG));
    frame.render_widget(widget, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use chrono::NaiveDate;

    #[test]
    fn render_shows_current_month_header() {
        use crate::app::state::{AppState, Context, Focus, Overlay};
        use crate::config::Config;
        use crate::model::day::Document;
        use chrono::NaiveDate;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use std::path::PathBuf;

        let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        let doc = Document::new_for_date(date);
        let selectables = doc.selectables();
        let app = AppState {
            doc,
            date,
            notes_dir: PathBuf::from("/tmp"),
            config: Config::default(),
            context: Context::Notes,
            focus: Focus::Capture,
            selected: 0,
            status: String::new(),
            input: String::new(),
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
        };

        let backend = TestBackend::new(30, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("June 2026"), "expected 'June 2026', got: {}", content);
    }

    #[test]
    fn render_shows_todo_text() {
        use crate::app::state::{AppState, Context, Focus, Overlay};
        use crate::config::Config;
        use crate::model::day::Document;
        use chrono::NaiveDate;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use std::path::PathBuf;

        let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        let doc = Document::new_for_date(date);
        let selectables = doc.selectables();
        let panel_todos = vec![PanelTodo {
            date,
            text: "buy milk".to_string(),
            todo_index: 0,
        }];
        let app = AppState {
            doc,
            date,
            notes_dir: PathBuf::from("/tmp"),
            config: Config::default(),
            context: Context::Notes,
            focus: Focus::Capture,
            selected: 0,
            status: String::new(),
            input: String::new(),
            overlay: Overlay::None,
            editing: None,
            should_quit: false,
            selectables,
            context_display: "context: Notes".to_string(),
            pending_delete: false,
            dates_with_notes: std::collections::BTreeSet::new(),
            right_panel_selected: 0,
            right_panel_scroll: 0,
            panel_todos,
        };

        let backend = TestBackend::new(30, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("buy milk"), "expected 'buy milk', got: {}", content);
    }

    #[test]
    fn render_selected_item_has_reversed_modifier() {
        use crate::app::state::{AppState, Context, Focus, Overlay};
        use crate::config::Config;
        use crate::model::day::Document;
        use chrono::NaiveDate;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use ratatui::style::Modifier;
        use std::path::PathBuf;

        let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        let doc = Document::new_for_date(date);
        let selectables = doc.selectables();
        let panel_todos = vec![PanelTodo {
            date,
            text: "buy milk".to_string(),
            todo_index: 0,
        }];
        let app = AppState {
            doc,
            date,
            notes_dir: PathBuf::from("/tmp"),
            config: Config::default(),
            context: Context::Notes,
            focus: Focus::RightPanel,
            selected: 0,
            status: String::new(),
            input: String::new(),
            overlay: Overlay::None,
            editing: None,
            should_quit: false,
            selectables,
            context_display: "context: Notes".to_string(),
            pending_delete: false,
            dates_with_notes: std::collections::BTreeSet::new(),
            right_panel_selected: 0,
            right_panel_scroll: 0,
            panel_todos,
        };

        let backend = TestBackend::new(30, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_reversed = buffer
            .content
            .iter()
            .any(|cell| cell.style().add_modifier.contains(Modifier::REVERSED));
        assert!(has_reversed, "expected REVERSED modifier for selected todo");
    }

    fn jun5() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()
    }

    fn write_day(dir: &std::path::Path, date: NaiveDate, content: &str) {
        let config = Config::default();
        let path = storage::path_for(dir, date, &config.date_format);
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn empty_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert!(todos.is_empty());
    }

    #[test]
    fn done_todos_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        let content = "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [x] done thing\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert!(todos.is_empty());
    }

    #[test]
    fn incomplete_todo_is_included() {
        let tmp = tempfile::tempdir().unwrap();
        let content = "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] buy milk\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].text, "buy milk");
        assert_eq!(todos[0].date, jun5());
    }

    #[test]
    fn mixed_todos_only_incomplete_returned() {
        let tmp = tempfile::tempdir().unwrap();
        let content =
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] buy milk\n- [x] done\n- [ ] call bank\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].text, "buy milk");
        assert_eq!(todos[1].text, "call bank");
    }

    #[test]
    fn multiple_days_most_recent_first() {
        let tmp = tempfile::tempdir().unwrap();
        let jun4 = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        write_day(
            tmp.path(),
            jun5(),
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] today task\n",
        );
        write_day(
            tmp.path(),
            jun4,
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] yesterday task\n",
        );
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].date, jun5());
        assert_eq!(todos[1].date, jun4);
    }

    #[test]
    fn days_beyond_lookback_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        // 8 days ago — outside default window of 7
        let old = jun5() - chrono::Duration::days(8);
        write_day(
            tmp.path(),
            old,
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] old task\n",
        );
        let config = Config::default(); // todo_lookback_days = 7
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert!(todos.is_empty());
    }

    #[test]
    fn lookback_boundary_is_inclusive() {
        let tmp = tempfile::tempdir().unwrap();
        // 6 days ago — inside default window of 7 (offsets 0..7 = 0,1,2,3,4,5,6)
        let border = jun5() - chrono::Duration::days(6);
        write_day(
            tmp.path(),
            border,
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] border task\n",
        );
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].text, "border task");
    }

    #[test]
    fn meeting_tag_stripped_from_display() {
        let tmp = tempfile::tempdir().unwrap();
        let content =
            "# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] follow up _(Standup)_\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].text, "follow up");
    }

    #[test]
    fn uppercase_x_done_prefix_stripped() {
        let result = display_text("- [X] some task");
        assert_eq!(result, "some task");
    }

    #[test]
    fn todo_index_matches_selectable_position() {
        let tmp = tempfile::tempdir().unwrap();
        // A doc with a bullet then a todo — todo should be at selectable index 1
        let content =
            "# Day\n\n## Meetings\n\n## Notes\n- some note\n\n## To-dos\n- [ ] the task\n";
        write_day(tmp.path(), jun5(), content);
        let config = Config::default();
        let todos = collect_panel_todos(tmp.path(), jun5(), &config);
        assert_eq!(todos.len(), 1);

        // Verify the stored todo_index actually points to the right selectable
        let doc = Document::from_text(
            "# Day\n\n## Meetings\n\n## Notes\n- some note\n\n## To-dos\n- [ ] the task\n",
        );
        let selectables = doc.selectables();
        assert!(
            matches!(selectables[todos[0].todo_index].kind, SelectableKind::Todo { done: false }),
            "expected todo at todo_index {}, got {:?}",
            todos[0].todo_index,
            selectables[todos[0].todo_index].kind
        );
    }
}
