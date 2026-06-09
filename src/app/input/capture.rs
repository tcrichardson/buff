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
        UiAction::TypeChar(c)         => type_char(state, c),
        UiAction::DeleteChar          => delete_char(state),
        UiAction::TypeNewline         => type_newline(state),
        UiAction::TypeIndent          => type_indent(state),
        UiAction::PrependIndent       => prepend_indent(state),
        UiAction::RemoveIndent        => remove_indent(state),
        UiAction::SubmitInput         => submit_input(state)?,
        UiAction::CommitEdit          => crate::app::actions::commit_edit(state)?,
        UiAction::MoveCursorLeft      => move_cursor_left(state),
        UiAction::MoveCursorRight     => move_cursor_right(state),
        UiAction::MoveCursorLineStart => move_cursor_line_start(state),
        UiAction::MoveCursorLineEnd   => move_cursor_line_end(state),
        UiAction::SelectNext          => crate::app::actions::select_next(state),
        UiAction::SelectPrev          => crate::app::actions::select_prev(state),
        UiAction::SelectFirst         => crate::app::actions::select_first(state),
        UiAction::SelectLast          => crate::app::actions::select_last(state),
        UiAction::ToggleSelected      => crate::app::actions::toggle_selected(state),
        UiAction::BeginEdit           => crate::app::actions::begin_edit_selected(state),
        UiAction::ResumeHeading       => crate::app::actions::resume_selected_heading(state),
        _ => unreachable!("capture::execute_action called with non-capture action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}

// ── Shared helper ──────────────────────────────────────────────────────────────

/// Byte offset of the start of the line containing `cursor_pos`.
fn current_line_start(input: &str, cursor_pos: usize) -> usize {
    match input[..cursor_pos].rfind('\n') {
        Some(nl) => nl + 1,
        None => 0,
    }
}

// ── Input handlers ─────────────────────────────────────────────────────────────

fn type_char(state: &mut AppState, c: char) {
    state.input.insert(state.cursor_pos, c);
    state.cursor_pos += c.len_utf8();
}

fn delete_char(state: &mut AppState) {
    if state.cursor_pos > 0 {
        let prev = super::prev_char_boundary(&state.input, state.cursor_pos);
        state.input.remove(prev);
        state.cursor_pos = prev;
    }
}

fn type_newline(state: &mut AppState) {
    state.input.insert(state.cursor_pos, '\n');
    state.cursor_pos += 1;
}

fn type_indent(state: &mut AppState) {
    state.input.insert_str(state.cursor_pos, "->");
    state.cursor_pos += 2;
}

fn prepend_indent(state: &mut AppState) {
    let line_start = current_line_start(&state.input, state.cursor_pos);
    state.input.insert_str(line_start, "->");
    state.cursor_pos += 2;
}

fn remove_indent(state: &mut AppState) {
    let line_start = current_line_start(&state.input, state.cursor_pos);
    if state.input[line_start..].starts_with("->") {
        state.input.drain(line_start..line_start + 2);
        if state.cursor_pos > line_start {
            state.cursor_pos = state.cursor_pos.saturating_sub(2).max(line_start);
        }
    }
}

fn submit_input(state: &mut AppState) -> Result<()> {
    let cmd = crate::app::command::parse(&state.input);
    crate::app::actions::dispatch(state, cmd)?;
    state.input.clear();
    state.cursor_pos = 0;
    crate::app::actions::vim_jump_to_new_content(state);
    Ok(())
}

// ── Cursor movement handlers ───────────────────────────────────────────────────

fn move_cursor_left(state: &mut AppState) {
    state.cursor_pos = super::prev_char_boundary(&state.input, state.cursor_pos);
}

fn move_cursor_right(state: &mut AppState) {
    state.cursor_pos = super::next_char_boundary(&state.input, state.cursor_pos);
}

fn move_cursor_line_start(state: &mut AppState) {
    state.cursor_pos = current_line_start(&state.input, state.cursor_pos);
}

fn move_cursor_line_end(state: &mut AppState) {
    let after = &state.input[state.cursor_pos..];
    state.cursor_pos = match after.find('\n') {
        Some(nl_offset) => state.cursor_pos + nl_offset,
        None => state.input.len(),
    };
}
