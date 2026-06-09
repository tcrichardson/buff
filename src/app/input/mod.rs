mod capture;
mod chat;
mod right_panel;
mod vim_insert;
mod vim_normal;

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
pub(super) fn prev_char_boundary(s: &str, pos: usize) -> usize {
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
pub(super) fn next_char_boundary(s: &str, pos: usize) -> usize {
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
pub(super) fn vim_clamp_col(line: &str, col: usize) -> usize {
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
pub(super) fn next_word_start(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line[col..].chars().collect();
    let mut i = 0;
    // skip current word chars (non-whitespace)
    while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
    // skip whitespace
    while i < chars.len() && chars[i].is_whitespace() { i += 1; }
    col + chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>()
}

/// Find the byte offset of the start of the previous word on `line` from `col`.
pub(super) fn prev_word_start(line: &str, col: usize) -> usize {
    let before: Vec<char> = line[..col].chars().collect();
    let mut i = before.len();
    // skip whitespace going backward
    while i > 0 && before[i - 1].is_whitespace() { i -= 1; }
    // skip word chars going backward
    while i > 0 && !before[i - 1].is_whitespace() { i -= 1; }
    before[..i].iter().map(|c| c.len_utf8()).sum::<usize>()
}

/// Find the byte offset of the end of the current/next word on `line` from `col`.
pub(super) fn word_end(line: &str, col: usize) -> usize {
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
        Focus::Capture => capture::key_to_action(state, key),
        Focus::RightPanel => right_panel::key_to_action(state, key),
        Focus::Chat => match key.code {
            KeyCode::Down | KeyCode::Char('j') => Some(UiAction::ChatScrollDown),
            KeyCode::Up | KeyCode::Char('k') => Some(UiAction::ChatScrollUp),
            KeyCode::PageDown => Some(UiAction::ChatPageDown),
            KeyCode::PageUp => Some(UiAction::ChatPageUp),
            _ => None,
        },
        Focus::VimNormal => vim_normal::key_to_action(state, key),
        Focus::VimInsert => vim_insert::key_to_action(state, key),
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

        // Capture mode actions
        UiAction::TypeChar(_)
        | UiAction::DeleteChar
        | UiAction::TypeNewline
        | UiAction::TypeIndent
        | UiAction::PrependIndent
        | UiAction::RemoveIndent
        | UiAction::SubmitInput
        | UiAction::CommitEdit
        | UiAction::MoveCursorLeft
        | UiAction::MoveCursorRight
        | UiAction::MoveCursorLineStart
        | UiAction::MoveCursorLineEnd
        | UiAction::SelectNext
        | UiAction::SelectPrev
        | UiAction::SelectFirst
        | UiAction::SelectLast
        | UiAction::ToggleSelected
        | UiAction::BeginEdit
        | UiAction::ResumeHeading => return capture::execute_action(state, action),
        UiAction::OpenHelp => {
            state.overlay = Overlay::Help;
        }
        UiAction::SwitchToCapture => {
            state.focus = Focus::Capture;
        }
        UiAction::FocusVimNormal => {
            state.focus = Focus::VimNormal;
        }

        UiAction::VimMoveLeft
        | UiAction::VimMoveRight
        | UiAction::VimMoveUp
        | UiAction::VimMoveDown
        | UiAction::VimMoveLineStart
        | UiAction::VimMoveLineEnd
        | UiAction::VimMoveFileStart
        | UiAction::VimMoveFileEnd
        | UiAction::VimMoveWordForward
        | UiAction::VimMoveWordBackward
        | UiAction::VimMoveWordEnd
        | UiAction::VimSetPendingOp(_)
        | UiAction::VimClearPendingOp
        | UiAction::VimEnterInsert
        | UiAction::VimEnterInsertAfter
        | UiAction::VimEnterInsertEOL
        | UiAction::VimInsertLineBelow
        | UiAction::VimInsertLineAbove
        | UiAction::VimDeleteChar
        | UiAction::VimDeleteLine
        | UiAction::VimYankLine
        | UiAction::VimPasteBelow
        | UiAction::VimPasteAbove
        | UiAction::VimUndo
        | UiAction::VimToggleTodo => return vim_normal::execute_action(state, action),
        UiAction::VimInsertChar(_)
        | UiAction::VimInsertNewline
        | UiAction::VimInsertBackspace
        | UiAction::VimInsertDeleteWordBefore
        | UiAction::VimExitInsert => return vim_insert::execute_action(state, action),

        // Right panel
        UiAction::FocusRightPanel => {
            state.right_panel_selected = 0;
            state.focus = Focus::RightPanel;
        }
        UiAction::RightPanelBlur => {
            state.focus = Focus::Capture;
        }
        UiAction::RightPanelUp
        | UiAction::RightPanelDown
        | UiAction::RightPanelToggle => return right_panel::execute_action(state, action),

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
    use crate::app::state::Context;
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

    #[test]
    fn vim_cursor_context_updates_on_move() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec![
            "# Day".to_string(),
            String::new(),
            "## Notes".to_string(),
            "a note".to_string(),
            String::new(),
            "## To-dos".to_string(),
            "- [ ] a task".to_string(),
        ];
        // Cursor starts in Notes section
        state.vim.cursor_line = 3;
        assert!(matches!(state.context, Context::Notes));
        // Move down through empty line into To-dos
        execute_action(&mut state, UiAction::VimMoveDown).unwrap();
        execute_action(&mut state, UiAction::VimMoveDown).unwrap();
        assert_eq!(state.vim.cursor_line, 5);
        assert!(matches!(state.context, Context::Todos));
    }

    #[test]
    fn submit_input_jumps_cursor_to_new_content() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::VimNormal;
        state.doc.lines = vec!["# Day".to_string(), String::new(), "## Notes".to_string(), "line".to_string()];
        state.vim.cursor_line = 0;
        state.vim.cursor_col = 0;
        state.input = "/todo test".to_string();
        execute_action(&mut state, UiAction::SubmitInput).unwrap();
        // Cursor should be on the newly added todo line (last line)
        assert_eq!(state.vim.cursor_line, state.doc.lines.len() - 1);
        // Column should reset to 0
        assert_eq!(state.vim.cursor_col, 0);
    }
}
