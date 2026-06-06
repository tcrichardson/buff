use crate::config::Config;
use crate::model::day::{Document, SelectableKind};
use crate::storage;
use chrono::NaiveDate;
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

pub fn render(
    _frame: &mut ratatui::Frame,
    _area: ratatui::layout::Rect,
    _app: &crate::app::state::AppState,
) {
    // stub — implemented in Task 7 and 8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use chrono::NaiveDate;

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
