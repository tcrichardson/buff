use crate::app::state::AppState;
use crate::app::input::{CaptureAction, EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Enter => {
            if state.editing.is_some() {
                Some(UiAction::Capture(CaptureAction::CommitEdit))
            } else {
                Some(UiAction::Capture(CaptureAction::SubmitInput))
            }
        }
        KeyCode::Backspace => Some(UiAction::Capture(CaptureAction::DeleteChar)),
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::Capture(CaptureAction::TypeNewline))
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
        {
            Some(UiAction::Capture(CaptureAction::TypeChar(c)))
        }
        KeyCode::Left  => Some(UiAction::Capture(CaptureAction::MoveCursorLeft)),
        KeyCode::Right => Some(UiAction::Capture(CaptureAction::MoveCursorRight)),
        KeyCode::Home  => Some(UiAction::Capture(CaptureAction::MoveCursorLineStart)),
        KeyCode::End   => Some(UiAction::Capture(CaptureAction::MoveCursorLineEnd)),
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::Capture(CaptureAction::MoveCursorLineStart))
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::Capture(CaptureAction::MoveCursorLineEnd))
        }
        KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::Capture(CaptureAction::PrependIndent))
        }
        KeyCode::Up | KeyCode::Down => None,
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: CaptureAction) -> Result<EventOutcome> {
    match action {
        CaptureAction::TypeChar(c)         => type_char(state, c),
        CaptureAction::DeleteChar          => delete_char(state),
        CaptureAction::TypeNewline         => type_newline(state),
        CaptureAction::TypeIndent          => type_indent(state),
        CaptureAction::PrependIndent       => prepend_indent(state),
        CaptureAction::RemoveIndent        => remove_indent(state),
        CaptureAction::SubmitInput         => submit_input(state)?,
        CaptureAction::CommitEdit          => crate::app::actions::commit_edit(state)?,
        CaptureAction::MoveCursorLeft      => move_cursor_left(state),
        CaptureAction::MoveCursorRight     => move_cursor_right(state),
        CaptureAction::MoveCursorLineStart => move_cursor_line_start(state),
        CaptureAction::MoveCursorLineEnd   => move_cursor_line_end(state),
        CaptureAction::SelectNext          => crate::app::actions::select_next(state),
        CaptureAction::SelectPrev          => crate::app::actions::select_prev(state),
        CaptureAction::SelectFirst         => crate::app::actions::select_first(state),
        CaptureAction::SelectLast          => crate::app::actions::select_last(state),
        CaptureAction::ToggleSelected      => crate::app::actions::toggle_selected(state),
        CaptureAction::BeginEdit           => crate::app::actions::begin_edit_selected(state),
        CaptureAction::ResumeHeading       => crate::app::actions::resume_selected_heading(state),
        CaptureAction::CancelEdit          => {
            state.editing = None;
            state.input.clear();
            state.cursor_pos = 0;
        }
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
    state.doc_anchor_line =
        crate::app::context::context_heading_line(&state.doc.lines, &state.context);
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
