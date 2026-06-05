use crate::app::command::Command;
use crate::app::state::{AppState, Context};
use crate::model::day::EntryTarget;

pub fn go_to_date(state: &mut AppState, date: chrono::NaiveDate) -> anyhow::Result<()> {
    state.save()?;
    let notes_dir = state.notes_dir.clone();
    let config = state.config.clone();
    *state = AppState::open_day(notes_dir, config, date)?;
    state.save()?;
    state.dates_with_notes =
        crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
    Ok(())
}

pub fn go_today(state: &mut AppState) -> anyhow::Result<()> {
    go_to_date(state, chrono::Local::now().date_naive())
}

pub fn go_prev_day(state: &mut AppState) -> anyhow::Result<()> {
    go_to_date(state, state.date - chrono::Duration::days(1))
}

pub fn go_next_day(state: &mut AppState) -> anyhow::Result<()> {
    go_to_date(state, state.date + chrono::Duration::days(1))
}

pub fn dispatch(state: &mut AppState, cmd: Command) -> anyhow::Result<()> {
    match cmd {
        Command::Entry(text) => {
            let text = text.trim();
            if text.is_empty() {
                return Ok(());
            }
            let time_str = state.current_time_hhmm();
            let time = if state.config.timestamp_entries {
                Some(time_str.as_str())
            } else {
                None
            };
            match &state.context {
                Context::Notes => {
                    state.doc.add_entry(&EntryTarget::Notes, text, time);
                }
                Context::Meeting(ord) => {
                    state.doc.add_entry(&EntryTarget::Meeting(*ord), text, time);
                }
            }
            state.selectables = state.doc.selectables();
            state.save()?;
            state.dates_with_notes =
                crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
            state.status.clear();
        }
        Command::Meeting(name) => {
            let ord = state.doc.add_meeting(&state.current_time_hhmm(), &name);
            state.context = Context::Meeting(ord);
            state.selectables = state.doc.selectables();
            state.update_context_display();
            state.save()?;
            state.dates_with_notes =
                crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
            state.status.clear();
        }
        Command::Note => {
            state.context = Context::Notes;
            state.update_context_display();
            state.status.clear();
        }
        Command::Todo(text) => {
            let meeting_name = match &state.context {
                Context::Meeting(ord) => state.doc.meetings().get(*ord).map(|m| m.name.clone()),
                _ => None,
            };
            state.doc.add_todo(&text, meeting_name.as_deref());
            state.selectables = state.doc.selectables();
            state.save()?;
            state.dates_with_notes =
                crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
            state.status.clear();
        }
        Command::Leave => {
            state.context = Context::Notes;
            state.update_context_display();
            state.status.clear();
        }
        Command::Help => {
            state.status = "Press ? for help, /quit to exit".to_string();
        }
        Command::Quit => {
            state.should_quit = true;
        }
        Command::Summarize => {
            state.status = "summarize is not implemented yet".to_string();
        }
        Command::Unknown(word) => {
            state.status = format!("Unknown command: /{}", word);
        }
        Command::InvalidArgs(msg) => {
            state.status = msg;
        }
        Command::Today => {
            go_today(state)?;
            state.status.clear();
        }
        Command::Goto(Some(date)) => {
            go_to_date(state, date)?;
            state.status.clear();
        }
        Command::Goto(None) => {
            state.calendar = Some(crate::ui::calendar::CalendarState::new(state.date));
            state.overlay = crate::app::state::Overlay::Calendar;
        }
    }
    Ok(())
}

pub fn select_next(state: &mut AppState) {
    let count = state.selectables.len();
    if count > 0 {
        state.selected = (state.selected + 1).min(count - 1);
    }
}

pub fn select_prev(state: &mut AppState) {
    if state.selected > 0 {
        state.selected -= 1;
    }
}

pub fn select_first(state: &mut AppState) {
    if state.selectables.is_empty() {
        return;
    }
    state.selected = 0;
}

pub fn select_last(state: &mut AppState) {
    let count = state.selectables.len();
    if count > 0 {
        state.selected = count - 1;
    }
}

pub fn toggle_selected(state: &mut AppState) {
    match state.doc.toggle_todo(state.selected) {
        Ok(()) => {
            state.selectables = state.doc.selectables();
            let _ = state.save();
            state.dates_with_notes =
                crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
        }
        Err(e) => {
            state.status = e.to_string();
        }
    }
}

pub fn delete_selected(state: &mut AppState) -> anyhow::Result<()> {
    state.doc.delete_selectable(state.selected)?;
    state.selectables = state.doc.selectables();
    let count = state.selectables.len();
    if count > 0 {
        state.selected = state.selected.min(count - 1);
    } else {
        state.selected = 0;
    }
    state.save()?;
    state.dates_with_notes =
        crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
    Ok(())
}

pub fn begin_edit_selected(state: &mut AppState) {
    if let Some(sel) = state.selectables.get(state.selected) {
        state.editing = Some(state.selected);
        state.input = sel.text.clone();
        state.focus = crate::app::state::Focus::Capture;
    } else {
        state.status = "nothing selected".to_string();
    }
}

pub fn commit_edit(state: &mut AppState) -> anyhow::Result<()> {
    if let Some(idx) = state.editing {
        let new_lines = crate::model::writer::format_entry(&state.input, None);
        state.doc.replace_selectable(idx, &new_lines)?;
        state.selectables = state.doc.selectables();
        state.editing = None;
        state.input.clear();
        state.focus = crate::app::state::Focus::Navigate;
        state.save()?;
        state.dates_with_notes =
            crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
        state.status.clear();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::Config;
    use chrono::NaiveDate;

    fn test_state(tmp: &tempfile::TempDir) -> AppState {
        let config = Config::default();
        AppState::open_day(
            tmp.path().to_path_buf(),
            config,
            NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn two_plain_lines_append_notes() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first".to_string())).unwrap();
        dispatch(&mut state, Command::Entry("second".to_string())).unwrap();
        let text = state.doc.to_text();
        assert!(text.contains("- first\n"), "got: {}", text);
        assert!(text.contains("- second\n"), "got: {}", text);
    }

    #[test]
    fn meeting_then_entry_nests_bullet() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
        dispatch(&mut state, Command::Entry("point".to_string())).unwrap();
        let text = state.doc.to_text();
        let meeting_pos = text.find("### ").unwrap();
        let entry_pos = text.find("- point").unwrap();
        assert!(
            entry_pos > meeting_pos,
            "entry should be after meeting heading"
        );
        assert_eq!(state.context, Context::Meeting(0));
    }

    #[test]
    fn todo_in_meeting_gets_tag() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
        dispatch(&mut state, Command::Todo("follow up".to_string())).unwrap();
        let text = state.doc.to_text();
        assert!(
            text.contains("- [ ] follow up _(Standup)_"),
            "got: {}",
            text
        );
        assert_eq!(state.context, Context::Meeting(0));
    }

    #[test]
    fn leave_resets_context() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
        dispatch(&mut state, Command::Leave).unwrap();
        assert_eq!(state.context, Context::Notes);
    }

    #[test]
    fn note_resets_context() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
        dispatch(&mut state, Command::Note).unwrap();
        assert_eq!(state.context, Context::Notes);
    }

    #[test]
    fn empty_entry_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        let before = state.doc.to_text();
        dispatch(&mut state, Command::Entry("".to_string())).unwrap();
        assert_eq!(state.doc.to_text(), before);
    }

    #[test]
    fn summarize_sets_status() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        let before = state.doc.to_text();
        dispatch(&mut state, Command::Summarize).unwrap();
        assert_eq!(state.status, "summarize is not implemented yet");
        assert_eq!(state.doc.to_text(), before);
    }

    #[test]
    fn unknown_command_sets_status() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        let before = state.doc.to_text();
        dispatch(&mut state, Command::Unknown("bogus".to_string())).unwrap();
        assert_eq!(state.status, "Unknown command: /bogus");
        assert_eq!(state.doc.to_text(), before);
    }

    #[test]
    fn open_day_creates_template_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let state = test_state(&tmp);
        let text = state.doc.to_text();
        assert!(text.contains("# 2026-06-04 (Thu)"), "got: {}", text);
        assert!(text.contains("## Meetings"), "got: {}", text);
    }

    #[test]
    fn open_day_loads_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("2026-06-04-Thu.md");
        std::fs::write(&path, "# Custom\n\n## Meetings\n\n## Notes\n\n## To-dos\n").unwrap();
        let state = test_state(&tmp);
        assert_eq!(
            state.doc.to_text(),
            "# Custom\n\n## Meetings\n\n## Notes\n\n## To-dos\n"
        );
    }

    #[test]
    fn save_persists_to_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("hello".to_string())).unwrap();
        let path = tmp.path().join("2026-06-04-Thu.md");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("- hello\n"), "saved: {}", saved);
    }

    #[test]
    fn select_next_moves_down() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first".to_string())).unwrap();
        dispatch(&mut state, Command::Entry("second".to_string())).unwrap();
        state.selected = 0;
        select_next(&mut state);
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn select_next_clamps_at_end() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first".to_string())).unwrap();
        state.selected = 0;
        select_next(&mut state);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_prev_moves_up() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first".to_string())).unwrap();
        dispatch(&mut state, Command::Entry("second".to_string())).unwrap();
        state.selected = 1;
        select_prev(&mut state);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_prev_stops_at_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first".to_string())).unwrap();
        state.selected = 0;
        select_prev(&mut state);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_first_goes_to_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first".to_string())).unwrap();
        dispatch(&mut state, Command::Entry("second".to_string())).unwrap();
        state.selected = 1;
        select_first(&mut state);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_last_goes_to_end() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first".to_string())).unwrap();
        dispatch(&mut state, Command::Entry("second".to_string())).unwrap();
        state.selected = 0;
        select_last(&mut state);
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn toggle_selected_todo() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Todo("buy milk".to_string())).unwrap();
        state.selected = 0;
        toggle_selected(&mut state);
        let text = state.doc.to_text();
        assert!(
            text.contains("- [x] buy milk"),
            "todo should be checked: {}",
            text
        );
        let path = tmp.path().join("2026-06-04-Thu.md");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("- [x] buy milk"), "saved: {}", saved);
    }

    #[test]
    fn toggle_selected_entry_sets_status() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("idea".to_string())).unwrap();
        state.selected = 0;
        let before = state.doc.to_text();
        toggle_selected(&mut state);
        assert_eq!(state.status, "not a to-do");
        assert_eq!(state.doc.to_text(), before);
    }

    #[test]
    fn select_next_empty_doc_no_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.selected = 0;
        select_next(&mut state);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_prev_empty_doc_no_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.selected = 0;
        select_prev(&mut state);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn delete_selected_empty_doc_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        assert!(delete_selected(&mut state).is_err());
    }

    #[test]
    fn commit_edit_none_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = crate::app::state::Focus::Capture;
        assert!(commit_edit(&mut state).is_ok());
        assert_eq!(state.editing, None);
        assert_eq!(state.focus, crate::app::state::Focus::Capture);
    }

    #[test]
    fn delete_selected_removes_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first".to_string())).unwrap();
        dispatch(&mut state, Command::Entry("second".to_string())).unwrap();
        dispatch(&mut state, Command::Entry("third".to_string())).unwrap();
        state.selected = 1;
        delete_selected(&mut state).unwrap();
        let text = state.doc.to_text();
        assert!(text.contains("- first\n"), "first should remain");
        assert!(!text.contains("- second\n"), "second should be removed");
        assert!(text.contains("- third\n"), "third should remain");
        assert_eq!(
            state.selected, 1,
            "selection should be clamped to last index"
        );
    }

    #[test]
    fn edit_flow() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("idea".to_string())).unwrap();
        state.selected = 0;
        begin_edit_selected(&mut state);
        assert_eq!(state.editing, Some(0));
        assert_eq!(state.input, "- idea");
        assert_eq!(state.focus, crate::app::state::Focus::Capture);
        state.input = "new idea".to_string();
        commit_edit(&mut state).unwrap();
        let text = state.doc.to_text();
        assert!(text.contains("- new idea\n"), "got: {}", text);
        assert!(!text.contains("- idea\n"), "old text should be gone");
        assert_eq!(state.editing, None);
        assert_eq!(state.focus, crate::app::state::Focus::Navigate);
        assert!(state.input.is_empty());
        let path = tmp.path().join("2026-06-04-Thu.md");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("- new idea\n"), "saved: {}", saved);
    }

    #[test]
    fn go_prev_day_switches_date() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        go_prev_day(&mut state).unwrap();
        assert_eq!(state.date, NaiveDate::from_ymd_opt(2026, 6, 3).unwrap());
        let path = tmp.path().join("2026-06-03-Wed.md");
        assert!(path.exists(), "previous day file should be created");
    }

    #[test]
    fn go_next_day_switches_date() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        go_next_day(&mut state).unwrap();
        assert_eq!(state.date, NaiveDate::from_ymd_opt(2026, 6, 5).unwrap());
        let path = tmp.path().join("2026-06-05-Fri.md");
        assert!(path.exists(), "next day file should be created");
    }

    #[test]
    fn go_to_date_persists_current() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("hello".to_string())).unwrap();
        go_to_date(&mut state, NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()).unwrap();
        let path = tmp.path().join("2026-06-04-Thu.md");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(
            saved.contains("- hello\n"),
            "original day should be persisted: {}",
            saved
        );
    }

    #[test]
    fn go_to_date_loads_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("2026-06-05-Fri.md");
        std::fs::write(
            &path,
            "# Custom Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n",
        )
        .unwrap();
        let mut state = test_state(&tmp);
        go_to_date(&mut state, NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()).unwrap();
        assert_eq!(
            state.doc.to_text(),
            "# Custom Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n"
        );
    }

    #[test]
    fn status_cleared_after_successful_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Summarize).unwrap();
        assert_eq!(state.status, "summarize is not implemented yet");
        dispatch(&mut state, Command::Entry("hello".to_string())).unwrap();
        assert!(state.status.is_empty());
    }

    #[test]
    fn status_cleared_after_successful_meeting() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Unknown("bogus".to_string())).unwrap();
        assert_eq!(state.status, "Unknown command: /bogus");
        dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
        assert!(state.status.is_empty());
    }

    #[test]
    fn status_preserved_for_info_commands() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Summarize).unwrap();
        assert_eq!(state.status, "summarize is not implemented yet");
        dispatch(&mut state, Command::Help).unwrap();
        assert_eq!(state.status, "Press ? for help, /quit to exit");
    }

    #[test]
    fn invalid_args_sets_status() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        let before = state.doc.to_text();
        dispatch(&mut state, Command::InvalidArgs("bad args".to_string())).unwrap();
        assert_eq!(state.status, "bad args");
        assert_eq!(state.doc.to_text(), before);
    }

    #[test]
    fn status_cleared_after_successful_todo() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Unknown("bogus".to_string())).unwrap();
        assert_eq!(state.status, "Unknown command: /bogus");
        dispatch(&mut state, Command::Todo("buy milk".to_string())).unwrap();
        assert!(state.status.is_empty());
    }
}
