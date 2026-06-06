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

    // Right panel
    FocusRightPanel,
    RightPanelUp,
    RightPanelDown,
    RightPanelToggle,
    RightPanelBlur,
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
            _ => {} // fall through — Ctrl-J is handled in Capture; others ignored per mode
        }
    }

    // 4. Tab — focus cycle
    if key.code == KeyCode::Tab {
        return match state.focus {
            Focus::RightPanel => Some(UiAction::RightPanelBlur),
            _ => Some(UiAction::FocusRightPanel),
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
            KeyCode::Up | KeyCode::Down => None, // ignored in capture mode
            _ => None,
        },
        Focus::RightPanel => match key.code {
            KeyCode::Down | KeyCode::Char('j') => Some(UiAction::RightPanelDown),
            KeyCode::Up | KeyCode::Char('k') => Some(UiAction::RightPanelUp),
            KeyCode::Char(' ') | KeyCode::Char('x') => Some(UiAction::RightPanelToggle),
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
        UiAction::SubmitInput => {
            let cmd = crate::app::command::parse(&state.input);
            crate::app::actions::dispatch(state, cmd)?;
            if state.overlay != Overlay::None {
                state.pending_delete = false;
            }
            state.input.clear();
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
    fn tab_in_capture_focuses_right_panel() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::FocusRightPanel)
        );
    }

    #[test]
    fn tab_in_navigate_focuses_right_panel() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Navigate;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::FocusRightPanel)
        );
    }

    #[test]
    fn tab_in_right_panel_blurs() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Tab)),
            Some(UiAction::RightPanelBlur)
        );
    }

    #[test]
    fn esc_in_right_panel_blurs() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Esc)),
            Some(UiAction::RightPanelBlur)
        );
    }

    #[test]
    fn right_panel_down_moves_selection() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Down)),
            Some(UiAction::RightPanelDown)
        );
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('j'))),
            Some(UiAction::RightPanelDown)
        );
    }

    #[test]
    fn right_panel_up_moves_selection() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Up)),
            Some(UiAction::RightPanelUp)
        );
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('k'))),
            Some(UiAction::RightPanelUp)
        );
    }

    #[test]
    fn right_panel_space_triggers_toggle() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char(' '))),
            Some(UiAction::RightPanelToggle)
        );
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Char('x'))),
            Some(UiAction::RightPanelToggle)
        );
    }

    #[test]
    fn focus_right_panel_sets_focus_and_resets_selection() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        state.right_panel_selected = 3;
        execute_action(&mut state, UiAction::FocusRightPanel).unwrap();
        assert_eq!(state.focus, Focus::RightPanel);
        assert_eq!(state.right_panel_selected, 0);
    }

    #[test]
    fn right_panel_blur_returns_to_capture() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        execute_action(&mut state, UiAction::RightPanelBlur).unwrap();
        assert_eq!(state.focus, Focus::Capture);
    }

    #[test]
    fn right_panel_down_increments_selected() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        state.right_panel_selected = 0;
        state.panel_todos = vec![
            crate::ui::right_panel::PanelTodo {
                date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
                text: "a".to_string(),
                todo_index: 0,
            },
            crate::ui::right_panel::PanelTodo {
                date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
                text: "b".to_string(),
                todo_index: 1,
            },
        ];
        execute_action(&mut state, UiAction::RightPanelDown).unwrap();
        assert_eq!(state.right_panel_selected, 1);
    }

    #[test]
    fn right_panel_down_clamps_at_last() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        state.panel_todos = vec![crate::ui::right_panel::PanelTodo {
            date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            text: "only".to_string(),
            todo_index: 0,
        }];
        state.right_panel_selected = 0;
        execute_action(&mut state, UiAction::RightPanelDown).unwrap();
        assert_eq!(state.right_panel_selected, 0);
    }

    #[test]
    fn right_panel_up_decrements_selected() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        state.right_panel_selected = 1;
        state.panel_todos = vec![
            crate::ui::right_panel::PanelTodo {
                date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
                text: "a".to_string(),
                todo_index: 0,
            },
            crate::ui::right_panel::PanelTodo {
                date: chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
                text: "b".to_string(),
                todo_index: 1,
            },
        ];
        execute_action(&mut state, UiAction::RightPanelUp).unwrap();
        assert_eq!(state.right_panel_selected, 0);
    }

    #[test]
    fn right_panel_up_clamps_at_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::RightPanel;
        state.right_panel_selected = 0;
        execute_action(&mut state, UiAction::RightPanelUp).unwrap();
        assert_eq!(state.right_panel_selected, 0);
    }

    #[test]
    fn capture_left_arrow_moves_cursor_left() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Left)),
            Some(UiAction::MoveCursorLeft)
        );
    }

    #[test]
    fn capture_right_arrow_moves_cursor_right() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Right)),
            Some(UiAction::MoveCursorRight)
        );
    }

    #[test]
    fn capture_home_moves_to_line_start() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::Home)),
            Some(UiAction::MoveCursorLineStart)
        );
    }

    #[test]
    fn capture_end_moves_to_line_end() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, make_key(KeyCode::End)),
            Some(UiAction::MoveCursorLineEnd)
        );
    }

    #[test]
    fn capture_ctrl_a_moves_to_line_start() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, ctrl(KeyCode::Char('a'))),
            Some(UiAction::MoveCursorLineStart)
        );
    }

    #[test]
    fn capture_ctrl_e_moves_to_line_end() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.focus = Focus::Capture;
        assert_eq!(
            key_to_action(&state, ctrl(KeyCode::Char('e'))),
            Some(UiAction::MoveCursorLineEnd)
        );
    }

    #[test]
    fn cursor_pos_initializes_to_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let state = test_state(&tmp);
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn prev_char_boundary_steps_back_one_ascii() {
        assert_eq!(super::prev_char_boundary("hello", 3), 2);
    }

    #[test]
    fn prev_char_boundary_at_zero_stays_zero() {
        assert_eq!(super::prev_char_boundary("hello", 0), 0);
    }

    #[test]
    fn prev_char_boundary_steps_back_multibyte() {
        // "é" is U+00E9, encoded as 2 bytes: 0xC3 0xA9
        let s = "aé"; // bytes: [0x61, 0xC3, 0xA9]
        assert_eq!(super::prev_char_boundary(s, 3), 1); // from end back to start of 'é'
        assert_eq!(super::prev_char_boundary(s, 1), 0); // from 'é' back to 'a'
    }

    #[test]
    fn next_char_boundary_steps_forward_one_ascii() {
        assert_eq!(super::next_char_boundary("hello", 1), 2);
    }

    #[test]
    fn next_char_boundary_at_end_stays_end() {
        assert_eq!(super::next_char_boundary("hello", 5), 5);
    }

    #[test]
    fn next_char_boundary_steps_forward_multibyte() {
        // "aé" bytes: [0x61, 0xC3, 0xA9]
        let s = "aé";
        assert_eq!(super::next_char_boundary(s, 0), 1); // 'a' → start of 'é'
        assert_eq!(super::next_char_boundary(s, 1), 3); // start of 'é' → past 'é' = end
    }

    #[test]
    fn move_cursor_left_decrements() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc".to_string();
        state.cursor_pos = 2;
        execute_action(&mut state, UiAction::MoveCursorLeft).unwrap();
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn move_cursor_left_clamps_at_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc".to_string();
        state.cursor_pos = 0;
        execute_action(&mut state, UiAction::MoveCursorLeft).unwrap();
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn move_cursor_right_increments() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc".to_string();
        state.cursor_pos = 1;
        execute_action(&mut state, UiAction::MoveCursorRight).unwrap();
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn move_cursor_right_clamps_at_end() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc".to_string();
        state.cursor_pos = 3;
        execute_action(&mut state, UiAction::MoveCursorRight).unwrap();
        assert_eq!(state.cursor_pos, 3);
    }

    #[test]
    fn move_cursor_left_steps_over_multibyte_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "aé".to_string(); // bytes: [0x61, 0xC3, 0xA9] — len = 3
        state.cursor_pos = 3; // past 'é'
        execute_action(&mut state, UiAction::MoveCursorLeft).unwrap();
        assert_eq!(state.cursor_pos, 1); // back to start of 'é'
    }

    #[test]
    fn move_cursor_right_steps_over_multibyte_char() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "aé".to_string();
        state.cursor_pos = 1; // at start of 'é'
        execute_action(&mut state, UiAction::MoveCursorRight).unwrap();
        assert_eq!(state.cursor_pos, 3); // past 'é'
    }

    #[test]
    fn move_cursor_line_start_jumps_to_zero_when_no_newline() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "hello".to_string();
        state.cursor_pos = 3;
        execute_action(&mut state, UiAction::MoveCursorLineStart).unwrap();
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn move_cursor_line_start_jumps_past_newline() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc\ndefg".to_string();
        state.cursor_pos = 6; // at 'f' on second line
        execute_action(&mut state, UiAction::MoveCursorLineStart).unwrap();
        assert_eq!(state.cursor_pos, 4); // first char of second line ('d')
    }

    #[test]
    fn move_cursor_line_start_at_bol_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc\ndefg".to_string();
        state.cursor_pos = 4; // already at start of second line
        execute_action(&mut state, UiAction::MoveCursorLineStart).unwrap();
        assert_eq!(state.cursor_pos, 4);
    }

    #[test]
    fn move_cursor_line_end_jumps_to_end_when_no_newline() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "hello".to_string();
        state.cursor_pos = 2;
        execute_action(&mut state, UiAction::MoveCursorLineEnd).unwrap();
        assert_eq!(state.cursor_pos, 5);
    }

    #[test]
    fn move_cursor_line_end_jumps_to_newline_position() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc\ndefg".to_string();
        state.cursor_pos = 1; // at 'b' on first line
        execute_action(&mut state, UiAction::MoveCursorLineEnd).unwrap();
        assert_eq!(state.cursor_pos, 3); // position of '\n' (= right after 'c')
    }

    #[test]
    fn move_cursor_line_end_at_eol_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        state.input = "abc\ndefg".to_string();
        state.cursor_pos = 3; // already at position of '\n' on first line
        execute_action(&mut state, UiAction::MoveCursorLineEnd).unwrap();
        assert_eq!(state.cursor_pos, 3);
    }

}
