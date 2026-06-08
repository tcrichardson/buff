use crate::app::state::{AppState, Focus, Overlay};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(PartialEq, Eq, Debug)]
pub enum EventOutcome {
    Continue,
    Quit,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UiAction {
    // Universal
    Quit,

    // Help overlay
    CloseHelp,

    // Global hotkeys
    GoToday,
    PrevDay,
    NextDay,

    // Escape handling (context-dependent)
    CancelEdit,
    ExitCaptureMode,
    ExitVimNormal,

    // Capture mode
    TypeChar(char),
    DeleteChar,
    TypeNewline,
    TypeIndent,
    PrependIndent,
    RemoveIndent,
    SubmitInput,
    CommitEdit,

    // Capture mode — cursor movement
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorLineStart,
    MoveCursorLineEnd,

    // Navigate mode (legacy placeholders)
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    ToggleSelected,
    BeginEdit,
    ResumeHeading,
    OpenHelp,
    SwitchToCapture,
    FocusVimNormal,

    // VimNormal actions
    VimMoveLeft,
    VimMoveRight,
    VimMoveUp,
    VimMoveDown,
    VimMoveWordForward,
    VimMoveWordBackward,
    VimMoveWordEnd,
    VimMoveLineStart,
    VimMoveLineEnd,
    VimMoveFileStart,
    VimMoveFileEnd,
    VimSetPendingOp(char),
    VimClearPendingOp,
    VimEnterInsert,
    VimEnterInsertAfter,
    VimEnterInsertEOL,
    VimInsertLineBelow,
    VimInsertLineAbove,
    VimDeleteChar,
    VimDeleteLine,
    VimYankLine,
    VimPasteBelow,
    VimPasteAbove,
    VimUndo,
    VimToggleTodo,
    // VimInsert actions
    VimInsertChar(char),
    VimInsertNewline,
    VimInsertBackspace,
    VimInsertDeleteWordBefore,
    VimExitInsert,

    // Right panel
    FocusRightPanel,
    RightPanelUp,
    RightPanelDown,
    RightPanelToggle,
    RightPanelBlur,

    // Chat panel
    ToggleChat,
    FocusChat,
    ChatBlur,
    ChatScrollUp,
    ChatScrollDown,
    ChatPageUp,
    ChatPageDown,
}

/// Step back one Unicode scalar from `pos`. Returns 0 if already at start.
fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos - 1;
    while !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Step forward one Unicode scalar from `pos`. Returns `s.len()` if already at end.
fn next_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos + 1;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    p
}

/// Clamp `col` to a valid char boundary within `line`, respecting vim normal-mode
/// convention that the cursor must land on a character (not past the last one).
fn vim_clamp_col(line: &str, col: usize) -> usize {
    if line.is_empty() {
        return 0;
    }
    let max = prev_char_boundary(line, line.len()); // byte offset of last char start
    let col = col.min(max);
    // walk backward to a valid boundary
    let mut c = col;
    while c > 0 && !line.is_char_boundary(c) {
        c -= 1;
    }
    c
}

/// Find the byte offset of the start of the next word on `line` from `col`.
fn next_word_start(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line[col..].chars().collect();
    let mut i = 0;
    // skip current word chars (non-whitespace)
    while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
    // skip whitespace
    while i < chars.len() && chars[i].is_whitespace() { i += 1; }
    col + chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>()
}

/// Find the byte offset of the start of the previous word on `line` from `col`.
fn prev_word_start(line: &str, col: usize) -> usize {
    let before: Vec<char> = line[..col].chars().collect();
    let mut i = before.len();
    // skip whitespace going backward
    while i > 0 && before[i - 1].is_whitespace() { i -= 1; }
    // skip word chars going backward
    while i > 0 && !before[i - 1].is_whitespace() { i -= 1; }
    before[..i].iter().map(|c| c.len_utf8()).sum::<usize>()
}

/// Find the byte offset of the end of the current/next word on `line` from `col`.
fn word_end(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line[col..].chars().collect();
    let mut i = 0;
    // skip one char if at a non-whitespace (to find NEXT end)
    if i < chars.len() && !chars[i].is_whitespace() { i += 1; }
    // skip whitespace
    while i < chars.len() && chars[i].is_whitespace() { i += 1; }
    // skip non-whitespace to end of word
    while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
    let end_byte = col + chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>();
    // position on last char of word (one back)
    if end_byte > col {
        prev_char_boundary(line, end_byte)
    } else {
        col
    }
}

pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<UiAction> {
    // 1. Ctrl-C always quits regardless of mode
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(UiAction::Quit);
    }

    // 2. Help overlay — only handles its own keys
    if state.overlay == Overlay::Help {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') => Some(UiAction::CloseHelp),
            _ => None,
        };
    }

    // 3. Global Ctrl hotkeys (Ctrl-J handled later in Capture mode)
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('t') => return Some(UiAction::GoToday),
            KeyCode::Char('l') => return Some(UiAction::ToggleChat),
            _ => {} // fall through — Ctrl-J is handled in Capture; others ignored per mode
        }
    }

    // 4. Tab — focus cycle (or indent in capture mode)
    if key.code == KeyCode::Tab {
        return match state.focus {
            Focus::Capture => Some(UiAction::TypeIndent),
            Focus::VimNormal | Focus::VimInsert => Some(UiAction::SwitchToCapture),
            Focus::Chat => Some(UiAction::FocusRightPanel),
            Focus::RightPanel => Some(UiAction::FocusVimNormal),
        };
    }

    // 4b. BackTab — reverse focus cycle (or un-indent in capture mode)
    if key.code == KeyCode::BackTab {
        return match state.focus {
            Focus::Capture => Some(UiAction::RemoveIndent),
            Focus::VimNormal | Focus::VimInsert => Some(UiAction::FocusRightPanel),
            Focus::Chat => Some(UiAction::FocusVimNormal),
            Focus::RightPanel => {
                if state.chat.visible {
                    Some(UiAction::FocusChat)
                } else {
                    Some(UiAction::FocusVimNormal)
                }
            }
        };
    }

    // 5. Esc handling (context-dependent)
    if key.code == KeyCode::Esc {
        return match state.focus {
            Focus::Capture => {
                if state.editing.is_some() {
                    Some(UiAction::CancelEdit)
                } else {
                    Some(UiAction::ExitCaptureMode)
                }
            }
            Focus::VimNormal => None, // Esc is no-op in normal mode
            Focus::VimInsert => Some(UiAction::VimExitInsert),
            Focus::RightPanel => Some(UiAction::RightPanelBlur),
            Focus::Chat => Some(UiAction::ChatBlur),
        };
    }

    // 6. [ and ] day navigation — only when can_navigate
    let can_navigate = matches!(state.focus, Focus::VimNormal)
        || (matches!(state.focus, Focus::Capture) && state.input.is_empty());
    if can_navigate {
        match key.code {
            KeyCode::Char('[') => return Some(UiAction::PrevDay),
            KeyCode::Char(']') => return Some(UiAction::NextDay),
            _ => {}
        }
    }

    // 7. Mode-specific handling
    match state.focus {
        Focus::Capture => match key.code {
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
            KeyCode::Up | KeyCode::Down => None, // ignored in capture mode
            _ => None,
        },
        Focus::RightPanel => match key.code {
            KeyCode::Down | KeyCode::Char('j') => Some(UiAction::RightPanelDown),
            KeyCode::Up | KeyCode::Char('k') => Some(UiAction::RightPanelUp),
            KeyCode::Char(' ') | KeyCode::Char('x') => Some(UiAction::RightPanelToggle),
            _ => None,
        },
        Focus::Chat => match key.code {
            KeyCode::Down | KeyCode::Char('j') => Some(UiAction::ChatScrollDown),
            KeyCode::Up | KeyCode::Char('k') => Some(UiAction::ChatScrollUp),
            KeyCode::PageDown => Some(UiAction::ChatPageDown),
            KeyCode::PageUp => Some(UiAction::ChatPageUp),
            _ => None,
        },
        Focus::VimNormal => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return None;
            }
            // Multi-key pending op
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
        Focus::VimInsert => {
            match key.code {
                KeyCode::Esc     => Some(UiAction::VimExitInsert),
                KeyCode::Enter   => Some(UiAction::VimInsertNewline),
                KeyCode::Backspace => Some(UiAction::VimInsertBackspace),
                KeyCode::Left    => Some(UiAction::VimMoveLeft),
                KeyCode::Right   => Some(UiAction::VimMoveRight),
                KeyCode::Up      => Some(UiAction::VimMoveUp),
                KeyCode::Down    => Some(UiAction::VimMoveDown),
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
    }
}

pub fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
        UiAction::Quit => return Ok(EventOutcome::Quit),

        // Help overlay
        UiAction::CloseHelp => {
            state.overlay = Overlay::None;
        }

        // Global hotkeys
        UiAction::GoToday => {
            crate::app::actions::go_today(state)?;
            state.status.clear();
        }
        UiAction::PrevDay => {
            crate::app::actions::go_prev_day(state)?;
        }
        UiAction::NextDay => {
            crate::app::actions::go_next_day(state)?;
        }

        // Escape handling
        UiAction::CancelEdit => {
            state.editing = None;
            state.input.clear();
            state.cursor_pos = 0;
        }
        UiAction::ExitCaptureMode => {
            state.focus = Focus::VimNormal;
        }
        UiAction::ExitVimNormal => {
            state.focus = Focus::Capture;
        }

        // Capture mode
        UiAction::TypeChar(c) => {
            state.input.insert(state.cursor_pos, c);
            state.cursor_pos += c.len_utf8();
        }
        UiAction::DeleteChar => {
            if state.cursor_pos > 0 {
                let prev = prev_char_boundary(&state.input, state.cursor_pos);
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
        }
        UiAction::CommitEdit => {
            crate::app::actions::commit_edit(state)?;
            // Note: commit_edit clears state.input internally (see actions.rs)
        }

        // Capture mode — cursor movement
        UiAction::MoveCursorLeft => {
            state.cursor_pos = prev_char_boundary(&state.input, state.cursor_pos);
        }
        UiAction::MoveCursorRight => {
            state.cursor_pos = next_char_boundary(&state.input, state.cursor_pos);
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

        // Navigate mode
        UiAction::SelectNext => {
            crate::app::actions::select_next(state);
        }
        UiAction::SelectPrev => {
            crate::app::actions::select_prev(state);
        }
        UiAction::SelectFirst => {
            crate::app::actions::select_first(state);
        }
        UiAction::SelectLast => {
            crate::app::actions::select_last(state);
        }
        UiAction::ToggleSelected => {
            crate::app::actions::toggle_selected(state);
        }
        UiAction::BeginEdit => {
            crate::app::actions::begin_edit_selected(state);
        }
        UiAction::ResumeHeading => {
            crate::app::actions::resume_selected_heading(state);
        }
        UiAction::OpenHelp => {
            state.overlay = Overlay::Help;
        }
        UiAction::SwitchToCapture => {
            state.focus = Focus::Capture;
        }
        UiAction::FocusVimNormal => {
            state.focus = Focus::VimNormal;
        }

        UiAction::VimMoveLeft => {
            let col = state.vim.cursor_col;
            if col > 0 {
                state.vim.cursor_col = prev_char_boundary(
                    &state.doc.lines[state.vim.cursor_line],
                    col,
                );
            }
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveRight => {
            let line = &state.doc.lines[state.vim.cursor_line];
            let col = state.vim.cursor_col;
            let next = next_char_boundary(line, col);
            // Normal mode: cannot move past last character
            if next < line.len() {
                state.vim.cursor_col = next;
            }
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimMoveDown => {
            let n = state.doc.lines.len();
            if state.vim.cursor_line + 1 < n {
                state.vim.cursor_line += 1;
                state.vim.cursor_col = vim_clamp_col(
                    &state.doc.lines[state.vim.cursor_line],
                    state.vim.cursor_col,
                );
                crate::app::actions::vim_update_context(state);
            }
        }
        UiAction::VimMoveUp => {
            if state.vim.cursor_line > 0 {
                state.vim.cursor_line -= 1;
                state.vim.cursor_col = vim_clamp_col(
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
                prev_char_boundary(line, line.len())
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
            state.vim.cursor_col = vim_clamp_col(
                &state.doc.lines[state.vim.cursor_line],
                state.vim.cursor_col,
            );
            crate::app::actions::vim_update_context(state);
        }
        UiAction::VimSetPendingOp(op) => {
            state.vim.pending_op = Some(op);
        }
        UiAction::VimClearPendingOp => {
            state.vim.pending_op = None;
        }
        UiAction::VimMoveWordForward => {
            let line = &state.doc.lines[state.vim.cursor_line];
            let new_col = next_word_start(line, state.vim.cursor_col);
            state.vim.cursor_col = vim_clamp_col(line, new_col);
        }
        UiAction::VimMoveWordBackward => {
            let line = &state.doc.lines[state.vim.cursor_line];
            state.vim.cursor_col = prev_word_start(line, state.vim.cursor_col);
        }
        UiAction::VimMoveWordEnd => {
            let line = &state.doc.lines[state.vim.cursor_line];
            state.vim.cursor_col = word_end(line, state.vim.cursor_col);
        }
        UiAction::VimEnterInsert => {
            // Save undo snapshot
            state.vim.undo_stack.push(crate::app::state::UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            state.focus = Focus::VimInsert;
        }
        UiAction::VimEnterInsertAfter => {
            state.vim.undo_stack.push(crate::app::state::UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            // advance cursor one (insert after)
            let line = &state.doc.lines[state.vim.cursor_line];
            let next = next_char_boundary(line, state.vim.cursor_col);
            state.vim.cursor_col = next.min(line.len()); // insert mode can be at end
            state.focus = Focus::VimInsert;
        }
        UiAction::VimEnterInsertEOL => {
            state.vim.undo_stack.push(crate::app::state::UndoEntry {
                lines: state.doc.lines.clone(),
                cursor_line: state.vim.cursor_line,
                cursor_col: state.vim.cursor_col,
            });
            state.vim.pending_op = None;
            state.vim.cursor_col = state.doc.lines[state.vim.cursor_line].len(); // past last char
            state.focus = Focus::VimInsert;
        }
        UiAction::VimExitInsert => {
            // Move cursor left one (vim convention: exit insert lands on last typed char)
            let col = state.vim.cursor_col;
            let line = &state.doc.lines[state.vim.cursor_line];
            if col > 0 {
                state.vim.cursor_col = prev_char_boundary(line, col);
            }
            state.vim.cursor_col = vim_clamp_col(
                &state.doc.lines[state.vim.cursor_line],
                state.vim.cursor_col,
            );
            state.vim.pending_op = None;
            state.focus = Focus::VimNormal;
            let _ = crate::app::actions::after_vim_edit(state);
        }
        UiAction::VimInsertLineBelow => {
            // Save undo snapshot
            state.vim.undo_stack.push(crate::app::state::UndoEntry {
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
            state.vim.undo_stack.push(crate::app::state::UndoEntry {
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
            if line.is_empty() {
                // nothing to delete
            } else {
                let col = state.vim.cursor_col;
                let end = next_char_boundary(line, col);
                let line = &mut state.doc.lines[state.vim.cursor_line];
                line.drain(col..end);
                // Clamp cursor after deletion
                let line_len = state.doc.lines[state.vim.cursor_line].len();
                if col > 0 && col >= line_len {
                    state.vim.cursor_col = prev_char_boundary(
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
            if n == 0 { /* nothing */ } else {
                let removed = state.doc.lines.remove(state.vim.cursor_line);
                state.vim.yank_buffer = vec![removed];
                // Clamp cursor_line
                let new_n = state.doc.lines.len();
                if state.vim.cursor_line >= new_n && new_n > 0 {
                    state.vim.cursor_line = new_n - 1;
                }
                if !state.doc.lines.is_empty() {
                    state.vim.cursor_col = vim_clamp_col(
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
        UiAction::VimInsertChar(c) => {
            let line = &mut state.doc.lines[state.vim.cursor_line];
            line.insert(state.vim.cursor_col, c);
            state.vim.cursor_col += c.len_utf8();
        }
        UiAction::VimInsertNewline => {
            let tail = state.doc.lines[state.vim.cursor_line][state.vim.cursor_col..].to_string();
            state.doc.lines[state.vim.cursor_line].truncate(state.vim.cursor_col);
            state.vim.cursor_line += 1;
            state.doc.lines.insert(state.vim.cursor_line, tail);
            state.vim.cursor_col = 0;
        }
        UiAction::VimInsertBackspace => {
            let col = state.vim.cursor_col;
            if col > 0 {
                // Delete char before cursor on current line
                let prev = prev_char_boundary(&state.doc.lines[state.vim.cursor_line], col);
                state.doc.lines[state.vim.cursor_line].remove(prev);
                state.vim.cursor_col = prev;
            } else if state.vim.cursor_line > 0 {
                // Merge with previous line
                let current = state.doc.lines.remove(state.vim.cursor_line);
                state.vim.cursor_line -= 1;
                let prev_len = state.doc.lines[state.vim.cursor_line].len();
                state.doc.lines[state.vim.cursor_line].push_str(&current);
                state.vim.cursor_col = prev_len;
            }
        }
        UiAction::VimInsertDeleteWordBefore => {
            let col = state.vim.cursor_col;
            let new_col = prev_word_start(&state.doc.lines[state.vim.cursor_line], col);
            let line = &mut state.doc.lines[state.vim.cursor_line];
            line.drain(new_col..col);
            state.vim.cursor_col = new_col;
        }

        // Right panel
        UiAction::FocusRightPanel => {
            state.right_panel_selected = 0;
            state.focus = Focus::RightPanel;
        }
        UiAction::RightPanelBlur => {
            state.focus = Focus::Capture;
        }
        UiAction::RightPanelUp => {
            if state.right_panel_selected > 0 {
                state.right_panel_selected -= 1;
            }
        }
        UiAction::RightPanelDown => {
            let max = state.panel_todos.len().saturating_sub(1);
            if state.right_panel_selected < max {
                state.right_panel_selected += 1;
            }
        }
        UiAction::RightPanelToggle => {
            crate::app::actions::toggle_panel_todo(state)?;
        }

        // Chat panel
        UiAction::ToggleChat => {
            state.chat.visible = !state.chat.visible;
            if !state.chat.visible && state.focus == Focus::Chat {
                state.focus = Focus::Capture;
            }
        }
        UiAction::FocusChat => {
            state.focus = Focus::Chat;
        }
        UiAction::ChatBlur => {
            state.focus = Focus::Capture;
        }
        UiAction::ChatScrollUp => {
            state.chat.scroll += 1;
        }
        UiAction::ChatScrollDown => {
            state.chat.scroll = state.chat.scroll.saturating_sub(1);
        }
        UiAction::ChatPageUp => {
            state.chat.scroll += 10;
        }
        UiAction::ChatPageDown => {
            state.chat.scroll = state.chat.scroll.saturating_sub(10);
        }
    }

    if state.should_quit {
        return Ok(EventOutcome::Quit);
    }
    Ok(EventOutcome::Continue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use chrono::NaiveDate;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn test_state(tmp: &tempfile::TempDir) -> AppState {
        AppState::open_day(
            tmp.path().to_path_buf(),
            Config::default(),
            NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
        )
        .unwrap()
    }

    // --- key_to_action tests ---

    #[test]
    fn ctrl_c_always_quits() {
        let tmp = tempfile::tempdir().unwrap();
        let state = test_state(&tmp);
        assert_eq!(key_to_action(&state, ctrl(KeyCode::Char('c'))), Some(UiAction::Quit));
    }

    #[test]
    fn vimnormal_j_moves_down() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('j'))),
            Some(UiAction::VimMoveDown)
        );
    }

    #[test]
    fn vimnormal_down_moves_down() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Down)),
            Some(UiAction::VimMoveDown)
        );
    }

    #[test]
    fn capture_bracket_empty_input_is_prev_day() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        // input is empty by default
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('['))),
            Some(UiAction::PrevDay)
        );
    }

    #[test]
    fn capture_bracket_nonempty_input_types_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        state.input.push('x');
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('['))),
            Some(UiAction::TypeChar('['))
        );
    }

    #[test]
    fn capture_ctrl_j_is_newline() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, ctrl(KeyCode::Char('j'))),
            Some(UiAction::TypeNewline)
        );
    }

    #[test]
    fn help_overlay_esc_closes() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Help;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::CloseHelp)
        );
    }

    #[test]
    fn help_overlay_ignores_other_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Help;
        assert_eq!(key_to_action(&state, make_key(KeyCode::Char('j'))), None);
    }

    #[test]
    fn esc_in_capture_with_editing_cancels_edit() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        state.editing = Some(0);
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::CancelEdit)
        );
    }

    #[test]
    fn esc_in_capture_without_editing_exits_capture() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::ExitCaptureMode)
        );
    }

    #[test]
    fn esc_in_vimnormal_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        assert_eq!(key_to_action(&state, make_key(KeyCode::Esc)), None);
    }

    #[test]
    fn ctrl_t_goes_today() {
        let tmp = tempfile::tempdir().unwrap();
        let state = test_state(&tmp);
        assert_eq!(key_to_action(&state, ctrl(KeyCode::Char('t'))), Some(UiAction::GoToday));
    }

    #[test]
    fn navigate_ctrl_combo_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        // Ctrl-X is not a recognized combo — should be None in navigate mode
        assert_eq!(key_to_action(&state, ctrl(KeyCode::Char('x'))), None);
    }

    // --- execute_action tests (simple state mutations only) ---

    #[test]
    fn quit_returns_quit_outcome() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        let outcome = execute_action(&mut state, UiAction::Quit).unwrap();
        assert_eq!(outcome, EventOutcome::Quit);
    }

    #[test]
    fn type_char_appends_to_input() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::TypeChar('a')).unwrap();
        assert_eq!(state.input, "a");
    }

    #[test]
    fn type_char_multiple_appends_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::TypeChar('h')).unwrap();
        execute_action(&mut state, UiAction::TypeChar('i')).unwrap();
        assert_eq!(state.input, "hi");
    }

    #[test]
    fn type_char_inserts_at_cursor_pos() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "ac".to_string();
        state.cursor_pos = 1; // between 'a' and 'c'
        execute_action(&mut state, UiAction::TypeChar('b')).unwrap();
        assert_eq!(state.input, "abc");
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn delete_char_pops_last_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "ab".to_string();
        state.cursor_pos = 2; // cursor at end
        execute_action(&mut state, UiAction::DeleteChar).unwrap();
        assert_eq!(state.input, "a");
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn delete_char_removes_char_before_cursor() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc".to_string();
        state.cursor_pos = 2; // between 'b' and 'c'
        execute_action(&mut state, UiAction::DeleteChar).unwrap();
        assert_eq!(state.input, "ac");
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn delete_char_at_start_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc".to_string();
        state.cursor_pos = 0;
        execute_action(&mut state, UiAction::DeleteChar).unwrap();
        assert_eq!(state.input, "abc");
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn delete_char_on_empty_input_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::DeleteChar).unwrap();
        assert_eq!(state.input, "");
    }

    #[test]
    fn type_newline_pushes_newline_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::TypeNewline).unwrap();
        assert_eq!(state.input, "\n");
    }

    #[test]
    fn type_newline_inserts_at_cursor_pos() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "ab".to_string();
        state.cursor_pos = 1; // between 'a' and 'b'
        execute_action(&mut state, UiAction::TypeNewline).unwrap();
        assert_eq!(state.input, "a\nb");
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn type_indent_inserts_two_spaces() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::TypeIndent).unwrap();
        assert_eq!(state.input, "->");
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn type_indent_inserts_at_cursor_pos() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "ab".to_string();
        state.cursor_pos = 1; // between 'a' and 'b'
        execute_action(&mut state, UiAction::TypeIndent).unwrap();
        assert_eq!(state.input, "a->b");
        assert_eq!(state.cursor_pos, 3);
    }

    #[test]
    fn cancel_edit_clears_editing_and_input() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.editing = Some(0);
        state.input = "hello".to_string();
        execute_action(&mut state, UiAction::CancelEdit).unwrap();
        assert!(state.editing.is_none());
        assert!(state.input.is_empty());
    }

    #[test]
    fn cursor_pos_reset_to_zero_on_submit() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "hello".to_string();
        state.cursor_pos = 3;
        execute_action(&mut state, UiAction::SubmitInput).unwrap();
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn cursor_pos_reset_to_zero_on_cancel_edit() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.editing = Some(0);
        state.input = "hello".to_string();
        state.cursor_pos = 3;
        execute_action(&mut state, UiAction::CancelEdit).unwrap();
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn exit_capture_mode_switches_focus_to_navigate() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        assert_eq!(state.focus, Focus::Capture); // default focus
        execute_action(&mut state, UiAction::ExitCaptureMode).unwrap();
        assert_eq!(state.focus, Focus::VimNormal);
    }

    #[test]
    fn exit_vimnormal_mode_switches_focus_to_capture() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        execute_action(&mut state, UiAction::ExitVimNormal).unwrap();
        assert_eq!(state.focus, Focus::Capture);
    }

    #[test]
    fn open_help_sets_overlay() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        execute_action(&mut state, UiAction::OpenHelp).unwrap();
        assert_eq!(state.overlay, Overlay::Help);
    }

    #[test]
    fn switch_to_capture_sets_focus() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        execute_action(&mut state, UiAction::SwitchToCapture).unwrap();
        assert_eq!(state.focus, Focus::Capture);
    }

    #[test]
    fn close_help_clears_overlay() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.overlay = Overlay::Help;
        execute_action(&mut state, UiAction::CloseHelp).unwrap();
        assert_eq!(state.overlay, Overlay::None);
    }

    #[test]
    fn tab_in_capture_inserts_indent() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::TypeIndent)
        );
    }

    #[test]
    fn tab_in_vimnormal_switches_to_capture() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::SwitchToCapture)
        );
    }

    #[test]
    fn tab_from_chat_goes_to_right_panel() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Chat;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::FocusRightPanel)
        );
    }

    #[test]
    fn esc_in_chat_blurs_to_capture() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Chat;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::ChatBlur)
        );
    }

    #[test]
    fn chat_scroll_keys_map() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Chat;
        assert_eq!(key_to_action(&state, make_key(KeyCode::Char('k'))), Some(UiAction::ChatScrollUp));
        assert_eq!(key_to_action(&state, make_key(KeyCode::Char('j'))), Some(UiAction::ChatScrollDown));
    }

    #[test]
    fn chat_scroll_down_saturates_at_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.chat.scroll = 0;
        execute_action(&mut state, UiAction::ChatScrollDown).unwrap();
        assert_eq!(state.chat.scroll, 0);
    }

    #[test]
    fn chat_scroll_up_increments() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.chat.scroll = 0;
        execute_action(&mut state, UiAction::ChatScrollUp).unwrap();
        assert_eq!(state.chat.scroll, 1);
    }

    #[test]
    fn focus_chat_sets_focus() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        execute_action(&mut state, UiAction::FocusChat).unwrap();
        assert_eq!(state.focus, Focus::Chat);
    }

    #[test]
    fn backtab_in_capture_emits_remove_indent() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        let key = KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(key_to_action(&state, key), Some(UiAction::RemoveIndent));
    }

    #[test]
    fn remove_indent_removes_arrow_from_line_start() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "->item".to_string();
        state.cursor_pos = 6;
        execute_action(&mut state, UiAction::RemoveIndent).unwrap();
        assert_eq!(state.input, "item");
    }

    #[test]
    fn remove_indent_adjusts_cursor_pos_past_line_start() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "->item".to_string();
        state.cursor_pos = 6; // at end
        execute_action(&mut state, UiAction::RemoveIndent).unwrap();
        assert_eq!(state.cursor_pos, 4); // 6 - 2
    }

    #[test]
    fn remove_indent_clamps_cursor_to_line_start() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "->item".to_string();
        state.cursor_pos = 1; // inside the "->"
        execute_action(&mut state, UiAction::RemoveIndent).unwrap();
        assert_eq!(state.cursor_pos, 0); // clamped to line start
    }

    #[test]
    fn remove_indent_noop_when_no_arrow() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "item".to_string();
        state.cursor_pos = 2;
        execute_action(&mut state, UiAction::RemoveIndent).unwrap();
        assert_eq!(state.input, "item");
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn remove_indent_on_second_line_uses_line_start() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "parent\n->child".to_string();
        state.cursor_pos = 14; // at end of "->child"
        execute_action(&mut state, UiAction::RemoveIndent).unwrap();
        assert_eq!(state.input, "parent\nchild");
        assert_eq!(state.cursor_pos, 12); // 14 - 2
    }

    #[test]
    fn remove_indent_cursor_at_line_start_not_adjusted() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "->item".to_string();
        state.cursor_pos = 0; // at line start
        execute_action(&mut state, UiAction::RemoveIndent).unwrap();
        assert_eq!(state.input, "item");
        assert_eq!(state.cursor_pos, 0); // at line start, no adjustment
    }

    #[test]
    fn tab_in_right_panel_wraps_to_vimnormal() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::FocusVimNormal)
        );
    }

    #[test]
    fn backtab_in_vimnormal_wraps_to_right_panel() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        let key = KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(key_to_action(&state, key), Some(UiAction::FocusRightPanel));
    }

    #[test]
    fn backtab_in_chat_goes_to_navigate() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Chat;
        let key = KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(key_to_action(&state, key), Some(UiAction::FocusVimNormal));
    }

    #[test]
    fn backtab_in_right_panel_goes_to_chat_when_visible() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        state.chat.visible = true;
        let key = KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(key_to_action(&state, key), Some(UiAction::FocusChat));
    }

    #[test]
    fn backtab_in_right_panel_goes_to_vimnormal_when_chat_hidden() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        state.chat.visible = false;
        let key = KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(key_to_action(&state, key), Some(UiAction::FocusVimNormal));
    }

    #[test]
    fn focus_vimnormal_sets_focus_to_vimnormal() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        execute_action(&mut state, UiAction::FocusVimNormal).unwrap();
        assert_eq!(state.focus, Focus::VimNormal);
    }

    #[test]
    fn vimnormal_h_moves_left() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('h'))),
            Some(UiAction::VimMoveLeft)
        );
    }

    #[test]
    fn vimnormal_arrow_left_moves_left() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Left)),
            Some(UiAction::VimMoveLeft)
        );
    }

    #[test]
    fn vimnormal_dd_with_pending_deletes_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.vim.pending_op = Some('d');
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('d'))),
            Some(UiAction::VimDeleteLine)
        );
    }

    #[test]
    fn vimnormal_gg_with_pending_moves_file_start() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.vim.pending_op = Some('g');
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('g'))),
            Some(UiAction::VimMoveFileStart)
        );
    }

    #[test]
    fn vimnormal_pending_op_unknown_second_key_clears() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.vim.pending_op = Some('d');
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('x'))),
            Some(UiAction::VimClearPendingOp)
        );
    }

    #[test]
    fn vimnormal_esc_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        assert_eq!(key_to_action(&state, make_key(KeyCode::Esc)), None);
    }

    #[test]
    fn viminsert_esc_exits_insert() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimInsert;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::VimExitInsert)
        );
    }

    #[test]
    fn viminsert_char_emits_insert_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimInsert;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('a'))),
            Some(UiAction::VimInsertChar('a'))
        );
    }

    #[test]
    fn viminsert_arrow_right_moves_right() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimInsert;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Right)),
            Some(UiAction::VimMoveRight)
        );
    }

    #[test]
    fn vimnormal_tab_switches_to_capture() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::SwitchToCapture)
        );
    }

    #[test]
    fn vim_move_right_advances_cursor() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["hello".to_string()];
        state.vim.cursor_line = 0;
        state.vim.cursor_col = 0;
        execute_action(&mut state, UiAction::VimMoveRight).unwrap();
        assert_eq!(state.vim.cursor_col, 1);
    }

    #[test]
    fn vim_move_right_does_not_go_past_last_char_in_normal_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["hi".to_string()];
        state.vim.cursor_line = 0;
        state.vim.cursor_col = 1; // on 'i', last char
        execute_action(&mut state, UiAction::VimMoveRight).unwrap();
        assert_eq!(state.vim.cursor_col, 1, "cursor should not move past last char");
    }

    #[test]
    fn vim_move_down_advances_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["line 0".to_string(), "line 1".to_string()];
        state.vim.cursor_line = 0;
        execute_action(&mut state, UiAction::VimMoveDown).unwrap();
        assert_eq!(state.vim.cursor_line, 1);
    }

    #[test]
    fn vim_move_up_stays_at_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["line 0".to_string()];
        state.vim.cursor_line = 0;
        execute_action(&mut state, UiAction::VimMoveUp).unwrap();
        assert_eq!(state.vim.cursor_line, 0);
    }

    #[test]
    fn vim_move_file_end_goes_to_last_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        execute_action(&mut state, UiAction::VimMoveFileEnd).unwrap();
        assert_eq!(state.vim.cursor_line, 2);
    }

    #[test]
    fn vim_enter_insert_sets_vim_insert_focus() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["hello".to_string()];
        execute_action(&mut state, UiAction::VimEnterInsert).unwrap();
        assert_eq!(state.focus, Focus::VimInsert);
    }

    #[test]
    fn vim_enter_insert_pushes_undo_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["hello".to_string()];
        assert!(state.vim.undo_stack.is_empty());
        execute_action(&mut state, UiAction::VimEnterInsert).unwrap();
        assert_eq!(state.vim.undo_stack.len(), 1);
    }

    #[test]
    fn vim_pending_op_is_set_then_cleared() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        execute_action(&mut state, UiAction::VimSetPendingOp('d')).unwrap();
        assert_eq!(state.vim.pending_op, Some('d'));
        execute_action(&mut state, UiAction::VimClearPendingOp).unwrap();
        assert!(state.vim.pending_op.is_none());
    }

    #[test]
    fn vim_insert_char_adds_to_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimInsert;
        state.doc.lines = vec!["hello".to_string()];
        state.vim.cursor_line = 0;
        state.vim.cursor_col = 5; // end of "hello"
        execute_action(&mut state, UiAction::VimInsertChar('!')).unwrap();
        assert_eq!(state.doc.lines[0], "hello!");
        assert_eq!(state.vim.cursor_col, 6);
    }

    #[test]
    fn vim_insert_newline_splits_line() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimInsert;
        state.doc.lines = vec!["hello world".to_string()];
        state.vim.cursor_line = 0;
        state.vim.cursor_col = 5; // between "hello" and " world"
        execute_action(&mut state, UiAction::VimInsertNewline).unwrap();
        assert_eq!(state.doc.lines[0], "hello");
        assert_eq!(state.doc.lines[1], " world");
        assert_eq!(state.vim.cursor_line, 1);
        assert_eq!(state.vim.cursor_col, 0);
    }

    #[test]
    fn vim_insert_backspace_removes_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimInsert;
        state.doc.lines = vec!["hello".to_string()];
        state.vim.cursor_line = 0;
        state.vim.cursor_col = 3; // after "hel"
        execute_action(&mut state, UiAction::VimInsertBackspace).unwrap();
        assert_eq!(state.doc.lines[0], "helo");
        assert_eq!(state.vim.cursor_col, 2);
    }

    #[test]
    fn vim_insert_backspace_at_line_start_merges_with_prev() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimInsert;
        state.doc.lines = vec!["first".to_string(), "second".to_string()];
        state.vim.cursor_line = 1;
        state.vim.cursor_col = 0;
        execute_action(&mut state, UiAction::VimInsertBackspace).unwrap();
        assert_eq!(state.doc.lines.len(), 1);
        assert_eq!(state.doc.lines[0], "firstsecond");
        assert_eq!(state.vim.cursor_line, 0);
        assert_eq!(state.vim.cursor_col, 5);
    }

    #[test]
    fn vim_delete_line_removes_and_yanks() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["keep".to_string(), "delete me".to_string(), "keep2".to_string()];
        state.vim.cursor_line = 1;
        execute_action(&mut state, UiAction::VimDeleteLine).unwrap();
        assert_eq!(state.doc.lines.len(), 2);
        assert_eq!(state.doc.lines[0], "keep");
        assert_eq!(state.vim.yank_buffer, vec!["delete me".to_string()]);
    }

    #[test]
    fn vim_yank_line_does_not_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["yanked".to_string()];
        state.vim.cursor_line = 0;
        execute_action(&mut state, UiAction::VimYankLine).unwrap();
        assert_eq!(state.doc.lines.len(), 1, "line should still be there");
        assert_eq!(state.vim.yank_buffer, vec!["yanked".to_string()]);
    }

    #[test]
    fn vim_paste_below_inserts_after_cursor() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["line 0".to_string(), "line 2".to_string()];
        state.vim.yank_buffer = vec!["line 1".to_string()];
        state.vim.cursor_line = 0;
        execute_action(&mut state, UiAction::VimPasteBelow).unwrap();
        assert_eq!(state.doc.lines[1], "line 1");
        assert_eq!(state.vim.cursor_line, 1);
    }

    #[test]
    fn vim_undo_restores_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["original".to_string()];
        // Simulate entering insert and making a change
        execute_action(&mut state, UiAction::VimEnterInsert).unwrap(); // pushes snapshot
        state.doc.lines[0] = "modified".to_string();
        execute_action(&mut state, UiAction::VimExitInsert).unwrap();
        // Now undo
        execute_action(&mut state, UiAction::VimUndo).unwrap();
        assert_eq!(state.doc.lines[0], "original");
    }

    #[test]
    fn vim_toggle_todo_checks_unchecked() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec![
            "# Day".to_string(),
            String::new(),
            "## To-dos".to_string(),
            String::new(),
            "- [ ] a task".to_string(),
        ];
        state.vim.cursor_line = 4;
        execute_action(&mut state, UiAction::VimToggleTodo).unwrap();
        assert_eq!(state.doc.lines[4], "- [x] a task");
    }
}
