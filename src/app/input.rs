use crate::app::state::{AppState, Focus, Overlay};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(PartialEq, Eq, Debug)]
pub enum EventOutcome {
    Continue,
    Quit,
}

#[derive(Debug, PartialEq)]
pub enum UiAction {
    // Universal
    Quit,

    // Calendar overlay
    MoveCalendar { dx: i8, dy: i8 },
    ConfirmCalendar,
    CloseCalendar,

    // Help overlay
    CloseHelp,

    // Global hotkeys
    GoToday,
    OpenCalendar,
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
}

pub fn key_to_action(_state: &AppState, _key: KeyEvent) -> Option<UiAction> {
    todo!()
}

pub fn execute_action(_state: &mut AppState, _action: UiAction) -> Result<EventOutcome> {
    todo!()
}
