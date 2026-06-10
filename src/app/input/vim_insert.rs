use crate::app::state::{AppState, Focus};
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Esc       => Some(UiAction::VimExitInsert),
        KeyCode::Enter     => Some(UiAction::VimInsertNewline),
        KeyCode::Backspace => Some(UiAction::VimInsertBackspace),
        KeyCode::Tab       => Some(UiAction::VimInsertTab),
        KeyCode::Left      => Some(UiAction::VimMoveLeft),
        KeyCode::Right     => Some(UiAction::VimMoveRight),
        KeyCode::Up        => Some(UiAction::VimMoveUp),
        KeyCode::Down      => Some(UiAction::VimMoveDown),
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::VimInsertDeleteWordBefore)
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
        {
            Some(UiAction::VimInsertChar(c))
        }
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::VimExitInsert             => exit_insert(state),
        UiAction::VimInsertChar(c)          => insert_char(state, c),
        UiAction::VimInsertNewline          => insert_newline(state),
        UiAction::VimInsertBackspace        => insert_backspace(state),
        UiAction::VimInsertDeleteWordBefore => delete_word_before(state),
        UiAction::VimInsertTab             => insert_tab(state),
        _ => unreachable!("vim_insert::execute_action called with non-vim-insert action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}

// ── Handlers ───────────────────────────────────────────────────────────────────

fn exit_insert(state: &mut AppState) {
    let col = state.vim.cursor_col;
    let line = &state.doc.lines[state.vim.cursor_line];
    if col > 0 {
        state.vim.cursor_col = super::prev_char_boundary(line, col);
    }
    state.vim.cursor_col = super::vim_clamp_col(
        &state.doc.lines[state.vim.cursor_line],
        state.vim.cursor_col,
    );
    state.vim.pending_op = None;
    state.focus = Focus::VimNormal;
    let _ = crate::app::actions::after_vim_edit(state);
}

fn insert_char(state: &mut AppState, c: char) {
    let line = &mut state.doc.lines[state.vim.cursor_line];
    line.insert(state.vim.cursor_col, c);
    state.vim.cursor_col += c.len_utf8();
}

fn insert_newline(state: &mut AppState) {
    let tail = state.doc.lines[state.vim.cursor_line][state.vim.cursor_col..].to_string();
    state.doc.lines[state.vim.cursor_line].truncate(state.vim.cursor_col);
    state.vim.cursor_line += 1;
    state.doc.lines.insert(state.vim.cursor_line, tail);
    state.vim.cursor_col = 0;
}

fn insert_backspace(state: &mut AppState) {
    let col = state.vim.cursor_col;
    if col > 0 {
        let prev = super::prev_char_boundary(&state.doc.lines[state.vim.cursor_line], col);
        state.doc.lines[state.vim.cursor_line].remove(prev);
        state.vim.cursor_col = prev;
    } else if state.vim.cursor_line > 0 {
        let current = state.doc.lines.remove(state.vim.cursor_line);
        state.vim.cursor_line -= 1;
        let prev_len = state.doc.lines[state.vim.cursor_line].len();
        state.doc.lines[state.vim.cursor_line].push_str(&current);
        state.vim.cursor_col = prev_len;
    }
}

fn delete_word_before(state: &mut AppState) {
    let col = state.vim.cursor_col;
    let new_col = super::prev_word_start(&state.doc.lines[state.vim.cursor_line], col);
    let line = &mut state.doc.lines[state.vim.cursor_line];
    line.drain(new_col..col);
    state.vim.cursor_col = new_col;
}

fn insert_tab(state: &mut AppState) {
    let line = &mut state.doc.lines[state.vim.cursor_line];
    line.insert_str(state.vim.cursor_col, "  ");
    state.vim.cursor_col += 2;
}
