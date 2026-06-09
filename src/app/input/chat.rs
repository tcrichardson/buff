use crate::app::state::AppState;
use crate::app::input::{EventOutcome, UiAction};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(UiAction::ChatScrollDown),
        KeyCode::Up   | KeyCode::Char('k') => Some(UiAction::ChatScrollUp),
        KeyCode::PageDown => Some(UiAction::ChatPageDown),
        KeyCode::PageUp   => Some(UiAction::ChatPageUp),
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: UiAction) -> Result<EventOutcome> {
    match action {
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
        _ => unreachable!("chat::execute_action called with non-chat action: {:?}", action),
    }
    Ok(EventOutcome::Continue)
}
