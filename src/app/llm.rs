/// A request sent from the UI thread to the LLM worker thread.
/// Placeholder — LLM feature work populates the variants.
pub enum LlmRequest {
    // future: Prompt { text: String, context: Vec<String> }, Cancel, etc.
}

/// An event sent from the LLM worker thread back to the UI event loop.
/// Placeholder — LLM feature work populates the variants.
pub enum LlmEvent {
    // future: Token(String), Done, Error(String), etc.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn llm_channel_smoke_test() {
        // Confirms channel types compile and messages flow correctly.
        let (_tx, rx) = mpsc::channel::<LlmRequest>();
        let (event_tx, event_rx) = mpsc::channel::<LlmEvent>();

        // Dropping event_tx causes try_recv to return Err(Disconnected).
        drop(event_tx);
        assert!(event_rx.try_recv().is_err());

        // Request receiver returns Err(Empty) — sender still alive, no messages sent.
        assert!(rx.try_recv().is_err());
    }
}
