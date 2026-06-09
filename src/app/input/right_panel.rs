use crate::app::state::AppState;
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(UiAction::RightPanelDown),
        KeyCode::Up   | KeyCode::Char('k') => Some(UiAction::RightPanelUp),
        KeyCode::Char(' ') | KeyCode::Char('x') => Some(UiAction::RightPanelToggle),
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
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
        _ => unreachable!("right_panel::execute_action called with non-right-panel action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
