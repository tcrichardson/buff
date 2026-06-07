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
    ExitNavigateMode,

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

    // Navigate mode
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    ToggleSelected,
    BeginEdit,
    InitiateDelete,
    ConfirmDelete,
    CancelDelete,
    ResumeHeading,
    OpenHelp,
    SwitchToCapture,
    FocusNavigate,

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
            Focus::Navigate => {
                if state.chat.visible {
                    Some(UiAction::FocusChat)
                } else {
                    Some(UiAction::FocusRightPanel)
                }
            }
            Focus::Chat => Some(UiAction::FocusRightPanel),
            Focus::RightPanel => Some(UiAction::FocusNavigate),
        };
    }

    // 4b. BackTab — reverse focus cycle (or un-indent in capture mode)
    if key.code == KeyCode::BackTab {
        return match state.focus {
            Focus::Capture => Some(UiAction::RemoveIndent),
            Focus::Navigate => Some(UiAction::FocusRightPanel),
            Focus::Chat => Some(UiAction::FocusNavigate),
            Focus::RightPanel => {
                if state.chat.visible {
                    Some(UiAction::FocusChat)
                } else {
                    Some(UiAction::FocusNavigate)
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
            Focus::Navigate => Some(UiAction::ExitNavigateMode),
            Focus::RightPanel => Some(UiAction::RightPanelBlur),
            Focus::Chat => Some(UiAction::ChatBlur),
        };
    }

    // 6. [ and ] day navigation — only when can_navigate
    let can_navigate = matches!(state.focus, Focus::Navigate)
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
        Focus::Navigate => {
            // Ignore all Ctrl combos in navigate mode (Ctrl-C/T already handled above)
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return None;
            }
            if state.pending_delete {
                return match key.code {
                    KeyCode::Char('d') => Some(UiAction::ConfirmDelete),
                    _ => Some(UiAction::CancelDelete),
                };
            }
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => Some(UiAction::SelectNext),
                KeyCode::Char('k') | KeyCode::Up => Some(UiAction::SelectPrev),
                KeyCode::Char('g') => Some(UiAction::SelectFirst),
                KeyCode::Char('G') => Some(UiAction::SelectLast),
                KeyCode::Char(' ') | KeyCode::Char('x') => Some(UiAction::ToggleSelected),
                KeyCode::Char('e') => Some(UiAction::BeginEdit),
                KeyCode::Char('d') => Some(UiAction::InitiateDelete),
                KeyCode::Enter => Some(UiAction::ResumeHeading),
                KeyCode::Char('?') => Some(UiAction::OpenHelp),
                KeyCode::Char('i') => Some(UiAction::SwitchToCapture),
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
            state.focus = Focus::Navigate;
        }
        UiAction::ExitNavigateMode => {
            state.pending_delete = false;
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
            if state.overlay != Overlay::None {
                state.pending_delete = false;
            }
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
        UiAction::InitiateDelete => {
            state.pending_delete = true;
        }
        UiAction::ConfirmDelete => {
            if let Err(e) = crate::app::actions::delete_selected(state) {
                state.status = e.to_string();
            }
            state.pending_delete = false;
        }
        UiAction::CancelDelete => {
            // Key is consumed — user must re-press to take the cancelled action.
            // This is an intentional UX simplification vs. the original run() which
            // would fall through and also process the keystroke normally.
            state.pending_delete = false;
        }
        UiAction::ResumeHeading => {
            crate::app::actions::resume_selected_heading(state);
        }
        UiAction::OpenHelp => {
            state.pending_delete = false;
            state.overlay = Overlay::Help;
        }
        UiAction::SwitchToCapture => {
            state.pending_delete = false;
            state.focus = Focus::Capture;
        }
        UiAction::FocusNavigate => {
            state.pending_delete = false;
            state.focus = Focus::Navigate;
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
    fn navigate_j_selects_next() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('j'))),
            Some(UiAction::SelectNext)
        );
    }

    #[test]
    fn navigate_down_selects_next() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Down)),
            Some(UiAction::SelectNext)
        );
    }

    #[test]
    fn navigate_pending_delete_d_confirms() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('d'))),
            Some(UiAction::ConfirmDelete)
        );
    }

    #[test]
    fn navigate_pending_delete_other_key_cancels() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('j'))),
            Some(UiAction::CancelDelete)
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
    fn esc_in_navigate_exits_navigate() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::ExitNavigateMode)
        );
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
        state.focus = Focus::Navigate;
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
    fn initiate_delete_sets_pending_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        assert!(!state.pending_delete);
        execute_action(&mut state, UiAction::InitiateDelete).unwrap();
        assert!(state.pending_delete);
    }

    #[test]
    fn cancel_delete_clears_pending_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.pending_delete = true;
        execute_action(&mut state, UiAction::CancelDelete).unwrap();
        assert!(!state.pending_delete);
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
        assert_eq!(state.focus, Focus::Navigate);
    }

    #[test]
    fn exit_navigate_mode_switches_focus_to_capture_and_clears_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        execute_action(&mut state, UiAction::ExitNavigateMode).unwrap();
        assert_eq!(state.focus, Focus::Capture);
        assert!(!state.pending_delete);
    }

    #[test]
    fn open_help_sets_overlay_and_clears_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        execute_action(&mut state, UiAction::OpenHelp).unwrap();
        assert_eq!(state.overlay, Overlay::Help);
        assert!(!state.pending_delete);
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
    fn switch_to_capture_sets_focus_and_clears_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.pending_delete = true;
        execute_action(&mut state, UiAction::SwitchToCapture).unwrap();
        assert_eq!(state.focus, Focus::Capture);
        assert!(!state.pending_delete);
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
    fn tab_in_navigate_focuses_right_panel() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        state.chat.visible = false; // skip chat to reach right panel
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::FocusRightPanel)
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
    fn tab_in_right_panel_wraps_to_navigate() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::FocusNavigate)
        );
    }

    #[test]
    fn backtab_in_navigate_wraps_to_right_panel() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
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
        assert_eq!(key_to_action(&state, key), Some(UiAction::FocusNavigate));
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
    fn backtab_in_right_panel_goes_to_navigate_when_chat_hidden() {
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
        assert_eq!(key_to_action(&state, key), Some(UiAction::FocusNavigate));
    }

    #[test]
    fn focus_navigate_sets_focus_to_navigate() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        execute_action(&mut state, UiAction::FocusNavigate).unwrap();
        assert_eq!(state.focus, Focus::Navigate);
    }

    #[test]
    fn focus_navigate_clears_pending_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        state.pending_delete = true;
        execute_action(&mut state, UiAction::FocusNavigate).unwrap();
        assert!(!state.pending_delete);
    }

}
