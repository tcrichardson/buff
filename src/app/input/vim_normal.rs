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
        UiAction::VimMoveLeft => {
            let col = state.vim.cursor_col;
            if col > 0 {
                state.vim.cursor_col =
                    super::prev_char_boundary(&state.doc.lines[state.vim.cursor_line], col);
            }
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveRight => {
            let line = &state.doc.lines[state.vim.cursor_line];
            let col = state.vim.cursor_col;
            let next = super::next_char_boundary(line, col);
            if next < line.len() {
                state.vim.cursor_col = next;
            }
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveDown => {
            let n = state.doc.lines.len();
            if state.vim.cursor_line + 1 < n {
                state.vim.cursor_line += 1;
                state.vim.cursor_col = super::vim_clamp_col(
                    &state.doc.lines[state.vim.cursor_line],
                    state.vim.cursor_col,
                );
                crate::app::actions::vim_update_context(state);
            }
        }
        UiAction::VimMoveUp => {
            if state.vim.cursor_line > 0 {
                state.vim.cursor_line -= 1;
                state.vim.cursor_col = super::vim_clamp_col(
                    &state.doc.lines[state.vim.cursor_line],
                    state.vim.cursor_col,
                );
                crate::app::actions::vim_update_context(state);
            }
        }
        UiAction::VimMoveLineStart => {
            state.vim.cursor_col = 0;
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveLineEnd => {
            let line = &state.doc.lines[state.vim.cursor_line];
            state.vim.cursor_col = if line.is_empty() {
                0
            } else {
                super::prev_char_boundary(line, line.len())
            };
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveFileStart => {
            state.vim.pending_op = None;
            state.vim.cursor_line = 0;
            state.vim.cursor_col = 0;
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveFileEnd => {
            let n = state.doc.lines.len();
            state.vim.pending_op = None;
            state.vim.cursor_line = n.saturating_sub(1);
            state.vim.cursor_col = super::vim_clamp_col(
                &state.doc.lines[state.vim.cursor_line],
                state.vim.cursor_col,
            );
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveWordForward => {
            let line = &state.doc.lines[state.vim.cursor_line];
            let new_col = super::next_word_start(line, state.vim.cursor_col);
            state.vim.cursor_col = super::vim_clamp_col(line, new_col);
        }
        UiAction::VimMoveWordBackward => {
            let line = &state.doc.lines[state.vim.cursor_line];
            state.vim.cursor_col = super::prev_word_start(line, state.vim.cursor_col);
        }
        UiAction::VimMoveWordEnd => {
            let line = &state.doc.lines[state.vim.cursor_line];
            state.vim.cursor_col = super::word_end(line, state.vim.cursor_col);
        }
        UiAction::VimSetPendingOp(op) => {
            state.vim.pending_op = Some(op);
        }
        UiAction::VimClearPendingOp => {
            state.vim.pending_op = None;
        }
        UiAction::VimEnterInsert => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            state.focus = Focus::VimInsert;
        }
        UiAction::VimEnterInsertAfter => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            let line = &state.doc.lines[state.vim.cursor_line];
            let next = super::next_char_boundary(line, state.vim.cursor_col);
            state.vim.cursor_col = next.min(line.len());
            state.focus = Focus::VimInsert;
        }
        UiAction::VimEnterInsertEOL => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            state.vim.cursor_col = state.doc.lines[state.vim.cursor_line].len();
            state.focus = Focus::VimInsert;
        }
        UiAction::VimInsertLineBelow => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            let insert_at = state.vim.cursor_line + 1;
            state.doc.lines.insert(insert_at, String::new());
            state.vim.cursor_line = insert_at;
            state.vim.cursor_col = 0;
            state.focus = Focus::VimInsert;
        }
        UiAction::VimInsertLineAbove => {
            state.vim.undo_stack.push(UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            state.doc.lines.insert(state.vim.cursor_line, String::new());
            state.vim.cursor_col = 0;
            state.focus = Focus::VimInsert;
        }
        UiAction::VimDeleteChar => {
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
        UiAction::VimDeleteLine => {
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
        UiAction::VimYankLine => {
            state.vim.pending_op = None;
            let line = state.doc.lines[state.vim.cursor_line].clone();
            state.vim.yank_buffer = vec![line];
        }
        UiAction::VimPasteBelow => {
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
        UiAction::VimPasteAbove => {
            if !state.vim.yank_buffer.is_empty() {
                let insert_at = state.vim.cursor_line;
                for (i, line) in state.vim.yank_buffer.clone().into_iter().enumerate() {
                    state.doc.lines.insert(insert_at + i, line);
                }
                state.vim.cursor_col = 0;
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        UiAction::VimUndo => {
            if let Some(entry) = state.vim.undo_stack.pop() {
                state.doc.lines = entry.lines;
                state.vim.cursor_line = entry.cursor_line;
                state.vim.cursor_col = entry.cursor_col;
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        UiAction::VimToggleTodo => {
            let line_idx = state.vim.cursor_line;
            if state.doc.toggle_todo_at_line(line_idx).is_ok() {
                let _ = crate::app::actions::after_vim_edit(state);
            }
        }
        _ => unreachable!("vim_normal::execute_action called with non-vim-normal action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
