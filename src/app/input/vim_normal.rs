use crate::app::state::{AppState, Focus, UndoEntry};
use crate::app::input::{EventOutcome, FocusAction, OverlayAction, UiAction, VimNormalAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    if let Some(op) = state.vim.pending_op {
        return match (op, &key.code) {
            ('d', KeyCode::Char('d')) => Some(UiAction::VimNormal(VimNormalAction::DeleteLine)),
            ('y', KeyCode::Char('y')) => Some(UiAction::VimNormal(VimNormalAction::YankLine)),
            ('g', KeyCode::Char('g')) => Some(UiAction::VimNormal(VimNormalAction::MoveFileStart)),
            _ => Some(UiAction::VimNormal(VimNormalAction::ClearPendingOp)),
        };
    }
    match key.code {
        KeyCode::Char('h') | KeyCode::Left  => Some(UiAction::VimNormal(VimNormalAction::MoveLeft)),
        KeyCode::Char('l') | KeyCode::Right => Some(UiAction::VimNormal(VimNormalAction::MoveRight)),
        KeyCode::Char('j') | KeyCode::Down  => Some(UiAction::VimNormal(VimNormalAction::MoveDown)),
        KeyCode::Char('k') | KeyCode::Up    => Some(UiAction::VimNormal(VimNormalAction::MoveUp)),
        KeyCode::Char('w') => Some(UiAction::VimNormal(VimNormalAction::MoveWordForward)),
        KeyCode::Char('b') => Some(UiAction::VimNormal(VimNormalAction::MoveWordBackward)),
        KeyCode::Char('e') => Some(UiAction::VimNormal(VimNormalAction::MoveWordEnd)),
        KeyCode::Char('0') => Some(UiAction::VimNormal(VimNormalAction::MoveLineStart)),
        KeyCode::Char('$') => Some(UiAction::VimNormal(VimNormalAction::MoveLineEnd)),
        KeyCode::Char('G') => Some(UiAction::VimNormal(VimNormalAction::MoveFileEnd)),
        KeyCode::Char('i') => Some(UiAction::VimNormal(VimNormalAction::EnterInsert)),
        KeyCode::Char('a') => Some(UiAction::VimNormal(VimNormalAction::EnterInsertAfter)),
        KeyCode::Char('A') => Some(UiAction::VimNormal(VimNormalAction::EnterInsertEOL)),
        KeyCode::Char('o') => Some(UiAction::VimNormal(VimNormalAction::InsertLineBelow)),
        KeyCode::Char('O') => Some(UiAction::VimNormal(VimNormalAction::InsertLineAbove)),
        KeyCode::Char('x') => Some(UiAction::VimNormal(VimNormalAction::DeleteChar)),
        KeyCode::Char('d') => Some(UiAction::VimNormal(VimNormalAction::SetPendingOp('d'))),
        KeyCode::Char('y') => Some(UiAction::VimNormal(VimNormalAction::SetPendingOp('y'))),
        KeyCode::Char('g') => Some(UiAction::VimNormal(VimNormalAction::SetPendingOp('g'))),
        KeyCode::Char('p') => Some(UiAction::VimNormal(VimNormalAction::PasteBelow)),
        KeyCode::Char('P') => Some(UiAction::VimNormal(VimNormalAction::PasteAbove)),
        KeyCode::Char('u') => Some(UiAction::VimNormal(VimNormalAction::Undo)),
        KeyCode::Char('t') => Some(UiAction::VimNormal(VimNormalAction::ToggleTodo)),
        KeyCode::Char('?') => Some(UiAction::Overlay(OverlayAction::OpenHelp)),
        KeyCode::Tab       => Some(UiAction::Focus(FocusAction::SwitchToCapture)),
        KeyCode::Enter     => Some(UiAction::VimNormal(VimNormalAction::BeginEditLine)),
        KeyCode::Esc       => None,
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: VimNormalAction) -> Result<EventOutcome> {
    match action {
        VimNormalAction::MoveLeft         => move_left(state),
        VimNormalAction::MoveRight        => move_right(state),
        VimNormalAction::MoveDown         => move_down(state),
        VimNormalAction::MoveUp           => move_up(state),
        VimNormalAction::MoveLineStart    => move_line_start(state),
        VimNormalAction::MoveLineEnd      => move_line_end(state),
        VimNormalAction::MoveFileStart    => move_file_start(state),
        VimNormalAction::MoveFileEnd      => move_file_end(state),
        VimNormalAction::MoveWordForward  => move_word_forward(state),
        VimNormalAction::MoveWordBackward => move_word_backward(state),
        VimNormalAction::MoveWordEnd      => move_word_end(state),
        VimNormalAction::SetPendingOp(op) => { state.vim.pending_op = Some(op); }
        VimNormalAction::ClearPendingOp   => { state.vim.pending_op = None; }
        VimNormalAction::EnterInsert      => enter_insert(state),
        VimNormalAction::EnterInsertAfter => enter_insert_after(state),
        VimNormalAction::EnterInsertEOL   => enter_insert_eol(state),
        VimNormalAction::InsertLineBelow  => insert_line_below(state),
        VimNormalAction::InsertLineAbove  => insert_line_above(state),
        VimNormalAction::DeleteChar       => delete_char(state),
        VimNormalAction::DeleteLine       => delete_line(state),
        VimNormalAction::YankLine         => yank_line(state),
        VimNormalAction::PasteBelow       => paste_below(state),
        VimNormalAction::PasteAbove       => paste_above(state),
        VimNormalAction::Undo             => undo(state),
        VimNormalAction::ToggleTodo       => toggle_todo(state),
        VimNormalAction::BeginEditLine    => { crate::app::actions::vim_begin_edit_line(state)?; }
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
