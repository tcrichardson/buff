use crate::app::state::AppState;
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Enter => {
            if state.editing.is_some() {
                Some(UiAction::CommitEdit)
            } else {
                Some(UiAction::SubmitInput)
            }
        }
        KeyCode::Backspace => Some(UiAction::DeleteChar),
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::TypeNewline)
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
        {
            Some(UiAction::TypeChar(c))
        }
        KeyCode::Left => Some(UiAction::MoveCursorLeft),
        KeyCode::Right => Some(UiAction::MoveCursorRight),
        KeyCode::Home => Some(UiAction::MoveCursorLineStart),
        KeyCode::End => Some(UiAction::MoveCursorLineEnd),
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::MoveCursorLineStart)
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::MoveCursorLineEnd)
        }
        KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::PrependIndent)
        }
        KeyCode::Up | KeyCode::Down => None,
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::TypeChar(c) => {
            state.input.insert(state.cursor_pos, c);
            state.cursor_pos += c.len_utf8();
        }
        UiAction::DeleteChar => {
            if state.cursor_pos > 0 {
                let prev = super::prev_char_boundary(&state.input, state.cursor_pos);
                state.input.remove(prev);
                state.cursor_pos = prev;
            }
        }
        UiAction::TypeNewline => {
            state.input.insert(state.cursor_pos, '\n');
            state.cursor_pos += 1;
        }
        UiAction::TypeIndent => {
            state.input.insert_str(state.cursor_pos, "->");
            state.cursor_pos += 2;
        }
        UiAction::PrependIndent => {
            let line_start = match state.input[..state.cursor_pos].rfind('\n') {
                Some(nl) => nl + 1,
                None => 0,
            };
            state.input.insert_str(line_start, "->");
            state.cursor_pos += 2;
        }
        UiAction::RemoveIndent => {
            let line_start = match state.input[..state.cursor_pos].rfind('\n') {
                Some(nl) => nl + 1,
                None => 0,
            };
            if state.input[line_start..].starts_with("->") {
                state.input.drain(line_start..line_start + 2);
                if state.cursor_pos > line_start {
                    state.cursor_pos = state.cursor_pos.saturating_sub(2).max(line_start);
                }
            }
        }
        UiAction::SubmitInput => {
            let cmd = crate::app::command::parse(&state.input);
            crate::app::actions::dispatch(state, cmd)?;
            state.input.clear();
            state.cursor_pos = 0;
            crate::app::actions::vim_jump_to_new_content(state);
        }
        UiAction::CommitEdit => {
            crate::app::actions::commit_edit(state)?;
        }
        UiAction::MoveCursorLeft => {
            state.cursor_pos = super::prev_char_boundary(&state.input, state.cursor_pos);
        }
        UiAction::MoveCursorRight => {
            state.cursor_pos = super::next_char_boundary(&state.input, state.cursor_pos);
        }
        UiAction::MoveCursorLineStart => {
            let before = &state.input[..state.cursor_pos];
            state.cursor_pos = match before.rfind('\n') {
                Some(nl_pos) => nl_pos + 1,
                None => 0,
            };
        }
        UiAction::MoveCursorLineEnd => {
            let after = &state.input[state.cursor_pos..];
            state.cursor_pos = match after.find('\n') {
                Some(nl_offset) => state.cursor_pos + nl_offset,
                None => state.input.len(),
            };
        }
        UiAction::SelectNext => crate::app::actions::select_next(state),
        UiAction::SelectPrev => crate::app::actions::select_prev(state),
        UiAction::SelectFirst => crate::app::actions::select_first(state),
        UiAction::SelectLast => crate::app::actions::select_last(state),
        UiAction::ToggleSelected => crate::app::actions::toggle_selected(state),
        UiAction::BeginEdit => crate::app::actions::begin_edit_selected(state),
        UiAction::ResumeHeading => crate::app::actions::resume_selected_heading(state),
        _ => unreachable!("capture::execute_action called with non-capture action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
