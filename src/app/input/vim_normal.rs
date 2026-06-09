use crate::app::state::{AppState, Focus, UndoEntry};
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    if let Some(op) = state.vim.pending_op {
        return match (op, &key.code) {
            ('d', KeyCode::Char('d')) => Some(UiAction::VimDeleteLine),
            ('y', KeyCode::Char('y')) => Some(UiAction::VimYankLine),
            ('g', KeyCode::Char('g')) => Some(UiAction::VimMoveFileStart),
            _ => Some(UiAction::VimClearPendingOp),
        };
    }
    match key.code {
        KeyCode::Char('h') | KeyCode::Left  => Some(UiAction::VimMoveLeft),
        KeyCode::Char('l') | KeyCode::Right => Some(UiAction::VimMoveRight),
        KeyCode::Char('j') | KeyCode::Down  => Some(UiAction::VimMoveDown),
        KeyCode::Char('k') | KeyCode::Up    => Some(UiAction::VimMoveUp),
        KeyCode::Char('w') => Some(UiAction::VimMoveWordForward),
        KeyCode::Char('b') => Some(UiAction::VimMoveWordBackward),
        KeyCode::Char('e') => Some(UiAction::VimMoveWordEnd),
        KeyCode::Char('0') => Some(UiAction::VimMoveLineStart),
        KeyCode::Char('$') => Some(UiAction::VimMoveLineEnd),
        KeyCode::Char('G') => Some(UiAction::VimMoveFileEnd),
        KeyCode::Char('i') => Some(UiAction::VimEnterInsert),
        KeyCode::Char('a') => Some(UiAction::VimEnterInsertAfter),
        KeyCode::Char('A') => Some(UiAction::VimEnterInsertEOL),
        KeyCode::Char('o') => Some(UiAction::VimInsertLineBelow),
        KeyCode::Char('O') => Some(UiAction::VimInsertLineAbove),
        KeyCode::Char('x') => Some(UiAction::VimDeleteChar),
        KeyCode::Char('d') => Some(UiAction::VimSetPendingOp('d')),
        KeyCode::Char('y') => Some(UiAction::VimSetPendingOp('y')),
        KeyCode::Char('g') => Some(UiAction::VimSetPendingOp('g')),
        KeyCode::Char('p') => Some(UiAction::VimPasteBelow),
        KeyCode::Char('P') => Some(UiAction::VimPasteAbove),
        KeyCode::Char('u') => Some(UiAction::VimUndo),
        KeyCode::Char('t') => Some(UiAction::VimToggleTodo),
        KeyCode::Char('?') => Some(UiAction::OpenHelp),
        KeyCode::Tab       => Some(UiAction::SwitchToCapture),
        KeyCode::Esc       => None,
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::VimMoveLeft         => move_left(state),
        UiAction::VimMoveRight        => move_right(state),
        UiAction::VimMoveDown         => move_down(state),
        UiAction::VimMoveUp           => move_up(state),
        UiAction::VimMoveLineStart    => move_line_start(state),
        UiAction::VimMoveLineEnd      => move_line_end(state),
        UiAction::VimMoveFileStart    => move_file_start(state),
        UiAction::VimMoveFileEnd      => move_file_end(state),
        UiAction::VimMoveWordForward  => move_word_forward(state),
        UiAction::VimMoveWordBackward => move_word_backward(state),
        UiAction::VimMoveWordEnd      => move_word_end(state),
        UiAction::VimSetPendingOp(op) => { state.vim.pending_op = Some(op); }
        UiAction::VimClearPendingOp   => { state.vim.pending_op = None; }
        UiAction::VimEnterInsert      => enter_insert(state),
        UiAction::VimEnterInsertAfter => enter_insert_after(state),
        UiAction::VimEnterInsertEOL   => enter_insert_eol(state),
        UiAction::VimInsertLineBelow  => insert_line_below(state),
        UiAction::VimInsertLineAbove  => insert_line_above(state),
        UiAction::VimDeleteChar       => delete_char(state),
        UiAction::VimDeleteLine       => delete_line(state),
        UiAction::VimYankLine         => yank_line(state),
        UiAction::VimPasteBelow       => paste_below(state),
        UiAction::VimPasteAbove       => paste_above(state),
        UiAction::VimUndo             => undo(state),
        UiAction::VimToggleTodo       => toggle_todo(state),
        _ => unreachable!("vim_normal::execute_action called with non-vim-normal action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}

// ── Shared helpers ─────────────────────────────────────────────────────────────

fn push_undo_snapshot(state: &mut AppState) {
    state.vim.undo_stack.push(UndoEntry {
        lines: state.doc.lines.clone(),
        cursor_line: state.vim.cursor_line,
        cursor_col: state.vim.cursor_col,
    });
}

/// Move to a new line, clamping the column and updating context.
fn move_vertical(state: &mut AppState, new_line: usize) {
    state.vim.cursor_line = new_line;
    state.vim.cursor_col = super::vim_clamp_col(
        &state.doc.lines[new_line],
        state.vim.cursor_col,
    );
    crate::app::actions::vim_update_context(state);
}

// ── Motion handlers ────────────────────────────────────────────────────────────

fn move_left(state: &mut AppState) {
    let col = state.vim.cursor_col;
    if col > 0 {
        state.vim.cursor_col =
            super::prev_char_boundary(&state.doc.lines[state.vim.cursor_line], col);
    }
    crate::app::actions::vim_update_context(state);
}

fn move_right(state: &mut AppState) {
    let line = &state.doc.lines[state.vim.cursor_line];
    let col = state.vim.cursor_col;
    let next = super::next_char_boundary(line, col);
    if next < line.len() {
        state.vim.cursor_col = next;
    }
    crate::app::actions::vim_update_context(state);
}

fn move_down(state: &mut AppState) {
    let n = state.doc.lines.len();
    if state.vim.cursor_line + 1 < n {
        move_vertical(state, state.vim.cursor_line + 1);
    }
}

fn move_up(state: &mut AppState) {
    if state.vim.cursor_line > 0 {
        move_vertical(state, state.vim.cursor_line - 1);
    }
}

fn move_line_start(state: &mut AppState) {
    state.vim.cursor_col = 0;
    crate::app::actions::vim_update_context(state);
}

fn move_line_end(state: &mut AppState) {
    let line = &state.doc.lines[state.vim.cursor_line];
    state.vim.cursor_col = if line.is_empty() {
        0
    } else {
        super::prev_char_boundary(line, line.len())
    };
    crate::app::actions::vim_update_context(state);
}

fn move_file_start(state: &mut AppState) {
    state.vim.pending_op = None;
    state.vim.cursor_line = 0;
    state.vim.cursor_col = 0;
    crate::app::actions::vim_update_context(state);
}

fn move_file_end(state: &mut AppState) {
    let n = state.doc.lines.len();
    state.vim.pending_op = None;
    state.vim.cursor_line = n.saturating_sub(1);
    state.vim.cursor_col = super::vim_clamp_col(
        &state.doc.lines[state.vim.cursor_line],
        state.vim.cursor_col,
    );
    crate::app::actions::vim_update_context(state);
}

fn move_word_forward(state: &mut AppState) {
    let line = &state.doc.lines[state.vim.cursor_line];
    let new_col = super::next_word_start(line, state.vim.cursor_col);
    state.vim.cursor_col = super::vim_clamp_col(line, new_col);
}

fn move_word_backward(state: &mut AppState) {
    let line = &state.doc.lines[state.vim.cursor_line];
    state.vim.cursor_col = super::prev_word_start(line, state.vim.cursor_col);
}

fn move_word_end(state: &mut AppState) {
    let line = &state.doc.lines[state.vim.cursor_line];
    state.vim.cursor_col = super::word_end(line, state.vim.cursor_col);
}

// ── Mode transition handlers ───────────────────────────────────────────────────

fn enter_insert(state: &mut AppState) {
    push_undo_snapshot(state);
    state.vim.pending_op = None;
    state.focus = Focus::VimInsert;
}

fn enter_insert_after(state: &mut AppState) {
    push_undo_snapshot(state);
    state.vim.pending_op = None;
    let line = &state.doc.lines[state.vim.cursor_line];
    let next = super::next_char_boundary(line, state.vim.cursor_col);
    state.vim.cursor_col = next.min(line.len());
    state.focus = Focus::VimInsert;
}

fn enter_insert_eol(state: &mut AppState) {
    push_undo_snapshot(state);
    state.vim.pending_op = None;
    state.vim.cursor_col = state.doc.lines[state.vim.cursor_line].len();
    state.focus = Focus::VimInsert;
}

fn insert_line_below(state: &mut AppState) {
    push_undo_snapshot(state);
    state.vim.pending_op = None;
    let insert_at = state.vim.cursor_line + 1;
    state.doc.lines.insert(insert_at, String::new());
    state.vim.cursor_line = insert_at;
    state.vim.cursor_col = 0;
    state.focus = Focus::VimInsert;
}

fn insert_line_above(state: &mut AppState) {
    push_undo_snapshot(state);
    state.vim.pending_op = None;
    state.doc.lines.insert(state.vim.cursor_line, String::new());
    state.vim.cursor_col = 0;
    state.focus = Focus::VimInsert;
}

// ── Edit handlers ──────────────────────────────────────────────────────────────

fn delete_char(state: &mut AppState) {
    let line = &state.doc.lines[state.vim.cursor_line];
    if !line.is_empty() {
        let col = state.vim.cursor_col;
        let end = super::next_char_boundary(line, col);
        let line = &mut state.doc.lines[state.vim.cursor_line];
        line.drain(col..end);
        let line_len = state.doc.lines[state.vim.cursor_line].len();
        if col > 0 && col >= line_len {
            state.vim.cursor_col = super::prev_char_boundary(
                &state.doc.lines[state.vim.cursor_line],
                line_len,
            );
        }
        let _ = crate::app::actions::after_vim_edit(state);
    }
}

fn delete_line(state: &mut AppState) {
    state.vim.pending_op = None;
    let n = state.doc.lines.len();
    if n > 0 {
        let removed = state.doc.lines.remove(state.vim.cursor_line);
        state.vim.yank_buffer = vec![removed];
        let new_n = state.doc.lines.len();
        if state.vim.cursor_line >= new_n && new_n > 0 {
            state.vim.cursor_line = new_n - 1;
        }
        if !state.doc.lines.is_empty() {
            state.vim.cursor_col = super::vim_clamp_col(
                &state.doc.lines[state.vim.cursor_line],
                state.vim.cursor_col,
            );
        }
        let _ = crate::app::actions::after_vim_edit(state);
    }
}

fn yank_line(state: &mut AppState) {
    state.vim.pending_op = None;
    let line = state.doc.lines[state.vim.cursor_line].clone();
    state.vim.yank_buffer = vec![line];
}

fn paste_below(state: &mut AppState) {
    if !state.vim.yank_buffer.is_empty() {
        let insert_at = state.vim.cursor_line + 1;
        for (i, line) in state.vim.yank_buffer.clone().into_iter().enumerate() {
            state.doc.lines.insert(insert_at + i, line);
        }
        state.vim.cursor_line = insert_at;
        state.vim.cursor_col = 0;
        let _ = crate::app::actions::after_vim_edit(state);
    }
}

fn paste_above(state: &mut AppState) {
    if !state.vim.yank_buffer.is_empty() {
        let insert_at = state.vim.cursor_line;
        for (i, line) in state.vim.yank_buffer.clone().into_iter().enumerate() {
            state.doc.lines.insert(insert_at + i, line);
        }
        state.vim.cursor_col = 0;
        let _ = crate::app::actions::after_vim_edit(state);
    }
}

fn undo(state: &mut AppState) {
    if let Some(entry) = state.vim.undo_stack.pop() {
        state.doc.lines = entry.lines;
        state.vim.cursor_line = entry.cursor_line;
        state.vim.cursor_col = entry.cursor_col;
        let _ = crate::app::actions::after_vim_edit(state);
    }
}

fn toggle_todo(state: &mut AppState) {
    let line_idx = state.vim.cursor_line;
    if state.doc.toggle_todo_at_line(line_idx).is_ok() {
        let _ = crate::app::actions::after_vim_edit(state);
    }
}
