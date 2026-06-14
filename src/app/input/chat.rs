use crate::app::input::{ChatAction, EventOutcome, UiAction};
use crate::app::state::AppState;
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub(super) fn key_to_action(_state: &AppState, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(UiAction::Chat(ChatAction::ScrollDown)),
        KeyCode::Up | KeyCode::Char('k') => Some(UiAction::Chat(ChatAction::ScrollUp)),
        KeyCode::PageDown => Some(UiAction::Chat(ChatAction::PageDown)),
        KeyCode::PageUp => Some(UiAction::Chat(ChatAction::PageUp)),
        _ => None,
    }
}

pub(super) fn execute_action(state: &mut AppState, action: ChatAction) -> Result<EventOutcome> {
    match action {
        ChatAction::ScrollUp => {
            state.chat.scroll += 1;
        }
        ChatAction::ScrollDown => {
            state.chat.scroll = state.chat.scroll.saturating_sub(1);
        }
        ChatAction::PageUp => {
            state.chat.scroll += 10;
        }
        ChatAction::PageDown => {
            state.chat.scroll = state.chat.scroll.saturating_sub(10);
        }
    }
    Ok(EventOutcome::Continue)
}
