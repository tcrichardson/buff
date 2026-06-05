use crate::app::command::Command;
use crate::app::state::{AppState, Context};
use crate::model::day::EntryTarget;

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
            state.save()?;
        }
        Command::Meeting(name) => {
            let ord = state.doc.add_meeting(&state.current_time_hhmm(), &name);
            state.context = Context::Meeting(ord);
            state.save()?;
        }
        Command::Note => {
            state.context = Context::Notes;
        }
        Command::Todo(text) => {
            let meeting_name = match &state.context {
                Context::Meeting(ord) => {
                    state.doc.meetings().get(*ord).map(|m| m.name.clone())
                }
                _ => None,
            };
            state.doc.add_todo(&text, meeting_name.as_deref());
            state.save()?;
        }
        Command::Leave => {
            state.context = Context::Notes;
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
        Command::Today | Command::Goto(_) => {
            state.status = "navigation handled separately".to_string();
        }
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
        AppState::open_day(tmp.path().to_path_buf(), config, NaiveDate::from_ymd_opt(2026, 6, 4).unwrap()).unwrap()
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
        assert!(entry_pos > meeting_pos, "entry should be after meeting heading");
        assert_eq!(state.context, Context::Meeting(0));
    }

    #[test]
    fn todo_in_meeting_gets_tag() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
        dispatch(&mut state, Command::Todo("follow up".to_string())).unwrap();
        let text = state.doc.to_text();
        assert!(text.contains("- [ ] follow up _(Standup)_"), "got: {}", text);
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
        assert_eq!(state.doc.to_text(), "# Custom\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
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
}
