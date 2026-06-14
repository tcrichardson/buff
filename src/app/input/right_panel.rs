use crate::app::input::{EventOutcome, RightPanelAction, UiAction};
use crate::app::state::AppState;
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(UiAction::RightPanel(RightPanelAction::Down)),
        KeyCode::Up | KeyCode::Char('k') => Some(UiAction::RightPanel(RightPanelAction::Up)),
        KeyCode::Char(' ') | KeyCode::Char('x') => {
            Some(UiAction::RightPanel(RightPanelAction::Toggle))
        }
        _ => None,
    }
}

pub(super) fn execute_action(
    state: &mut AppState,
    action: RightPanelAction,
) -> Result<EventOutcome> {
    match action {
        RightPanelAction::Up => {
            if state.right_panel_selected > 0 {
                state.right_panel_selected -= 1;
            }
        }
        RightPanelAction::Down => {
            let max = state.panel_todos.len().saturating_sub(1);
            if state.right_panel_selected < max {
                state.right_panel_selected += 1;
            }
        }
        RightPanelAction::Toggle => {
            crate::app::actions::toggle_panel_todo(state)?;
        }
    }
    Ok(EventOutcome::Continue)
}
