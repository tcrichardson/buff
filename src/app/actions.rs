use crate::app::command::Command;
use crate::app::state::{AppState, Context};
use crate::model::day::{EntryTarget, SelectableKind};

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

fn after_doc_mutation(state: &mut AppState) -> anyhow::Result<()> {
    state.selectables = state.doc.selectables();
    state.save()?;
    state.dates_with_notes =
        crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
    state.status.clear();
    Ok(())
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
            let block = crate::model::writer::format_entry(text, time);
            let target = match &state.context {
                Context::Notes => EntryTarget::Notes,
                Context::Meeting(ord) => EntryTarget::Meeting(*ord),
                Context::NoteBlock(ord) => EntryTarget::NoteBlock(*ord),
            };
            state.doc.add_block(&target, &block);
            after_doc_mutation(state)?;
        }
        Command::Meeting(name) => {
            let ord = state.doc.add_meeting(&state.current_time_hhmm(), &name);
            state.context = Context::Meeting(ord);
            state.update_context_display();
            after_doc_mutation(state)?;
        }
        Command::Note(name) => {
            if let Some(n) = name {
                let ord = state.doc.add_note_heading(&n);
                state.context = Context::NoteBlock(ord);
                state.update_context_display();
                after_doc_mutation(state)?;
            } else {
                state.context = Context::Notes;
                state.update_context_display();
                state.status.clear();
            }
        }
        Command::Todo(text) => {
            let meeting_name = match &state.context {
                Context::Meeting(ord) => state.doc.meetings().get(*ord).map(|m| m.name.clone()),
                Context::NoteBlock(ord) => {
                    state.doc.note_headings().get(*ord).map(|n| n.name.clone())
                }
                _ => None,
            };
            state.doc.add_todo(&text, meeting_name.as_deref());
            after_doc_mutation(state)?;
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
            state.status = "usage: /goto YYYY-MM-DD".to_string();
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

pub fn resume_selected_heading(state: &mut AppState) {
    if let Some(sel) = state.selectables.get(state.selected) {
        match sel.kind {
            SelectableKind::MeetingHeading { ordinal } => {
                state.context = Context::Meeting(ordinal);
                state.update_context_display();
                state.focus = crate::app::state::Focus::Capture;
                state.status.clear();
                return;
            }
            SelectableKind::NoteHeading { ordinal } => {
                state.context = Context::NoteBlock(ordinal);
                state.update_context_display();
                state.focus = crate::app::state::Focus::Capture;
                state.status.clear();
                return;
            }
            _ => {}
        }
    }
    state.status = "not a meeting or note".to_string();
}

pub fn toggle_panel_todo(state: &mut AppState) -> anyhow::Result<()> {
    let Some(todo) = state.panel_todos.get(state.right_panel_selected).cloned() else {
        return Ok(()); // empty panel — nothing to do
    };

    let path = crate::storage::path_for(&state.notes_dir, todo.date, &state.config.date_format);
    let text = std::fs::read_to_string(&path)?;
    let mut doc = crate::model::day::Document::from_text(&text);
    doc.toggle_todo(todo.todo_index)?;

    // Write back to disk atomically
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, doc.to_text())?;
    std::fs::rename(&tmp_path, &path)?;

    // If this is today's date, also refresh app.doc so the left view stays in sync
    if todo.date == state.date {
        state.doc = doc;
        state.selectables = state.doc.selectables();
    }

    // Rebuild panel_todos (the toggled item is now done, drops off the list)
    state.panel_todos =
        crate::ui::right_panel::collect_panel_todos(&state.notes_dir, state.date, &state.config);

    // Clamp selection to new list length
    let new_len = state.panel_todos.len();
    if new_len == 0 {
        state.right_panel_selected = 0;
    } else {
        state.right_panel_selected = state.right_panel_selected.min(new_len - 1);
    }

    Ok(())
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
        dispatch(&mut state, Command::Note(None)).unwrap();
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
    fn goto_none_sets_usage_status() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Goto(None)).unwrap();
        assert_eq!(state.status, "usage: /goto YYYY-MM-DD");
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

    #[test]
    fn entry_markdown_heading_stored_verbatim() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("## Section".to_string())).unwrap();
        let text = state.doc.to_text();
        assert!(text.contains("## Section\n"), "got: {}", text);
        assert!(
            !text.contains("- ## Section"),
            "should not be wrapped: {}",
            text
        );
    }

    #[test]
    fn entry_multiline_plain_becomes_indented_bullet() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("first\nsecond".to_string())).unwrap();
        assert!(
            state.doc.to_text().contains("- first\n  second\n"),
            "got: {}",
            state.doc.to_text()
        );
    }

    #[test]
    fn resume_meeting_sets_context_and_focus() {
        use crate::model::day::SelectableKind;
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
        dispatch(&mut state, Command::Note(None)).unwrap(); // leave the meeting context
        assert_eq!(state.context, Context::Notes);

        let idx = state
            .selectables
            .iter()
            .position(|s| matches!(s.kind, SelectableKind::MeetingHeading { .. }))
            .expect("meeting heading should be selectable");
        state.selected = idx;
        state.focus = crate::app::state::Focus::Navigate;

        resume_selected_heading(&mut state);
        assert_eq!(state.context, Context::Meeting(0));
        assert_eq!(state.focus, crate::app::state::Focus::Capture);

        dispatch(&mut state, Command::Entry("under meeting".to_string())).unwrap();
        let text = state.doc.to_text();
        let heading = text.find("### ").unwrap();
        let entry = text.find("- under meeting").unwrap();
        assert!(entry > heading, "entry should be under the meeting heading");
    }

    #[test]
    fn resume_on_non_meeting_sets_status() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Entry("idea".to_string())).unwrap();
        state.selected = 0;
        state.focus = crate::app::state::Focus::Navigate;
        resume_selected_heading(&mut state);
        assert_eq!(state.status, "not a meeting or note");
    }

    #[test]
    fn note_then_entry_nests_bullet() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Note(Some("Idea Bucket".to_string()))).unwrap();
        dispatch(&mut state, Command::Entry("point".to_string())).unwrap();
        let text = state.doc.to_text();
        let heading_pos = text.find("### Idea Bucket").unwrap();
        let entry_pos = text.find("- point").unwrap();
        assert!(
            entry_pos > heading_pos,
            "entry should be after note heading"
        );
        assert_eq!(state.context, Context::NoteBlock(0));
    }

    #[test]
    fn note_without_name_resets_context() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Note(Some("Idea Bucket".to_string()))).unwrap();
        dispatch(&mut state, Command::Note(None)).unwrap();
        assert_eq!(state.context, Context::Notes);
    }

    #[test]
    fn resume_note_sets_context_and_focus() {
        use crate::model::day::SelectableKind;
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Note(Some("Idea Bucket".to_string()))).unwrap();
        dispatch(&mut state, Command::Note(None)).unwrap(); // leave the note context
        assert_eq!(state.context, Context::Notes);

        let idx = state
            .selectables
            .iter()
            .position(|s| matches!(s.kind, SelectableKind::NoteHeading { .. }))
            .expect("note heading should be selectable");
        state.selected = idx;
        state.focus = crate::app::state::Focus::Navigate;

        resume_selected_heading(&mut state);
        assert_eq!(state.context, Context::NoteBlock(0));
        assert_eq!(state.focus, crate::app::state::Focus::Capture);

        dispatch(&mut state, Command::Entry("under note".to_string())).unwrap();
        let text = state.doc.to_text();
        let heading = text.find("### Idea Bucket").unwrap();
        let entry = text.find("- under note").unwrap();
        assert!(entry > heading, "entry should be under the note heading");
    }

    #[test]
    fn toggle_panel_todo_marks_past_todo_done() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        let past = today - chrono::Duration::days(1);

        let past_path = crate::storage::path_for(tmp.path(), past, &config.date_format);
        std::fs::write(
            &past_path,
            "# 2026-06-04 (Thu)\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] past task\n",
        )
        .unwrap();

        let mut state =
            AppState::open_day(tmp.path().to_path_buf(), config.clone(), today).unwrap();
        state.focus = crate::app::state::Focus::RightPanel;
        state.right_panel_selected = 0;

        assert_eq!(state.panel_todos.len(), 1, "should have 1 open todo");

        toggle_panel_todo(&mut state).unwrap();

        let saved = std::fs::read_to_string(&past_path).unwrap();
        assert!(
            saved.contains("- [x] past task"),
            "past task should be checked: {}",
            saved
        );
        assert!(
            state.panel_todos.is_empty(),
            "panel_todos should be empty after toggle"
        );
    }

    #[test]
    fn toggle_panel_todo_today_updates_app_doc() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();

        let mut state =
            AppState::open_day(tmp.path().to_path_buf(), config.clone(), today).unwrap();

        dispatch(&mut state, Command::Todo("today task".to_string())).unwrap();

        state.panel_todos =
            crate::ui::right_panel::collect_panel_todos(&state.notes_dir, state.date, &state.config);

        state.focus = crate::app::state::Focus::RightPanel;
        state.right_panel_selected = 0;
        assert_eq!(state.panel_todos.len(), 1);

        toggle_panel_todo(&mut state).unwrap();

        let text = state.doc.to_text();
        assert!(
            text.contains("- [x] today task"),
            "today doc should be updated: {}",
            text
        );
        assert!(state.panel_todos.is_empty());
    }

    #[test]
    fn toggle_panel_todo_noop_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = crate::app::state::Focus::RightPanel;
        state.right_panel_selected = 0;
        assert!(toggle_panel_todo(&mut state).is_ok());
    }

    #[test]
    fn go_to_date_refreshes_panel_todos() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        let yesterday = today - chrono::Duration::days(1);

        let yest_path = crate::storage::path_for(tmp.path(), yesterday, &config.date_format);
        std::fs::write(
            &yest_path,
            "# 2026-06-04 (Thu)\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] old task\n",
        )
        .unwrap();

        let mut state =
            AppState::open_day(tmp.path().to_path_buf(), config.clone(), today).unwrap();
        let initial_count = state.panel_todos.len();

        go_to_date(&mut state, yesterday).unwrap();

        assert_eq!(
            state.panel_todos.len(),
            1,
            "panel_todos should be refreshed after navigation, had {} before",
            initial_count
        );
    }

    #[test]
    fn open_day_populates_panel_todos_from_past_files() {
        let tmp = tempfile::tempdir().unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        let past = date - chrono::Duration::days(2);
        let config = Config::default();
        let past_path = crate::storage::path_for(tmp.path(), past, &config.date_format);
        std::fs::write(
            &past_path,
            "# 2026-06-03 (Wed)\n\n## Meetings\n\n## Notes\n\n## To-dos\n- [ ] past task\n",
        )
        .unwrap();

        let state = AppState::open_day(tmp.path().to_path_buf(), config, date).unwrap();
        assert_eq!(state.panel_todos.len(), 1);
        assert_eq!(state.panel_todos[0].text, "past task");
    }

    #[test]
    fn todo_in_note_gets_tag() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Note(Some("Idea Bucket".to_string()))).unwrap();
        dispatch(&mut state, Command::Todo("follow up".to_string())).unwrap();
        let text = state.doc.to_text();
        assert!(
            text.contains("- [ ] follow up _(Idea Bucket)_"),
            "got: {}",
            text
        );
        assert_eq!(state.context, Context::NoteBlock(0));
    }
}
