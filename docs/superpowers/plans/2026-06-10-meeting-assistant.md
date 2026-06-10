# Meeting Assistant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `/start` activate a per-meeting AI assistant in the chat panel that injects live meeting context into every LLM call, and make `/end` generate a structured `#### Meeting Summary` written into the document.

**Architecture:** A `meeting_ordinal: Option<usize>` field in `ChatState` drives meeting assistant mode. When set, `save_chat()` uses a per-meeting sidecar, `handle_ask()` injects fresh meeting document content as a context message before every LLM call, and `handle_llm_event()` writes the summary to the document when the summary stream completes.

**Tech Stack:** Rust, ratatui TUI, ureq (LLM streaming), existing `storage`, `model/writer`, `app/actions`, `app/state`, `ui/chat_panel` modules.

---

## File Map

| File | Change |
|------|--------|
| `src/storage.rs` | Add `meeting_chat_path_for()` |
| `src/model/writer.rs` | Add `Document::add_meeting_summary()` and `normalize_summary()` |
| `src/app/state.rs` | Add `meeting_ordinal`, `summarizing` to `ChatState`; update `save_chat()`; add `apply_meeting_summary()`; update `handle_llm_event()` |
| `src/app/actions.rs` | Add system prompt constant, context helpers, `fire_meeting_llm_call()`; enhance `handle_start()`, `handle_end()`; update `handle_ask()` |
| `src/ui/chat_panel.rs` | Render `"Chat [MeetingName]"` header when in meeting mode |

---

## Task 1: `storage.rs` — `meeting_chat_path_for()`

**Files:**
- Modify: `src/storage.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `src/storage.rs`:

```rust
#[test]
fn meeting_chat_path_for_uses_meeting_suffix() {
    let date = NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
    let dir = std::path::PathBuf::from("/tmp/notes");
    assert_eq!(
        meeting_chat_path_for(&dir, date, "%Y-%m-%d-%a", 0),
        std::path::PathBuf::from("/tmp/notes/2026-06-10-Tue-meeting0.chat.json")
    );
    assert_eq!(
        meeting_chat_path_for(&dir, date, "%Y-%m-%d-%a", 2),
        std::path::PathBuf::from("/tmp/notes/2026-06-10-Tue-meeting2.chat.json")
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test meeting_chat_path_for_uses_meeting_suffix
```

Expected: FAIL with "cannot find function `meeting_chat_path_for`"

- [ ] **Step 3: Add the function**

In `src/storage.rs`, after `chat_path_for()` (after line 19):

```rust
pub fn meeting_chat_path_for(
    notes_dir: &Path,
    date: NaiveDate,
    date_format: &str,
    ordinal: usize,
) -> PathBuf {
    notes_dir.join(format!(
        "{}-meeting{}.chat.json",
        date.format(date_format),
        ordinal
    ))
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cargo test meeting_chat_path_for_uses_meeting_suffix
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/storage.rs
git commit -m "feat: add meeting_chat_path_for to storage"
```

---

## Task 2: `model/writer.rs` — `Document::add_meeting_summary()`

**Files:**
- Modify: `src/model/writer.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/model/writer.rs`:

```rust
#[test]
fn add_meeting_summary_appends_at_end_of_meeting_section() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\nmeta:Started: 09:00\n- note one\n- note two\n\n## Notes\n\n## To-dos\n",
    );
    doc.add_meeting_summary(0, "#### Meeting Summary\n**Key Decisions:** Ship it");
    let text = doc.to_text();
    let note_two_pos = text.find("- note two").unwrap();
    let summary_pos = text.find("#### Meeting Summary").unwrap();
    assert!(summary_pos > note_two_pos, "summary should be after notes: {}", text);
    // Must not bleed into Notes section
    let notes_pos = text.find("## Notes").unwrap();
    assert!(summary_pos < notes_pos, "summary must be before ## Notes: {}", text);
}

#[test]
fn add_meeting_summary_replaces_existing_summary() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- note\n\n#### Meeting Summary\n**Key Decisions:** old\n\n## Notes\n\n## To-dos\n",
    );
    doc.add_meeting_summary(0, "#### Meeting Summary\n**Key Decisions:** new");
    let text = doc.to_text();
    assert!(text.contains("**Key Decisions:** new"), "new summary missing: {}", text);
    assert!(!text.contains("**Key Decisions:** old"), "old summary should be gone: {}", text);
    // Only one #### Meeting Summary heading
    assert_eq!(text.matches("#### Meeting Summary").count(), 1, "duplicate summary: {}", text);
}

#[test]
fn add_meeting_summary_normalizes_missing_heading() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
    );
    // AI response without the heading
    doc.add_meeting_summary(0, "**Key Decisions:** Ship it");
    let text = doc.to_text();
    assert!(text.contains("#### Meeting Summary"), "heading should be added: {}", text);
    assert!(text.contains("**Key Decisions:** Ship it"), "content should be present: {}", text);
}

#[test]
fn add_meeting_summary_noop_for_invalid_ordinal() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
    );
    let before = doc.to_text();
    doc.add_meeting_summary(99, "#### Meeting Summary\n**Key Decisions:** x");
    assert_eq!(doc.to_text(), before, "invalid ordinal should be noop");
}

#[test]
fn add_meeting_summary_does_not_bleed_into_next_meeting() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- standup note\n\n### Review\n- review note\n\n## Notes\n\n## To-dos\n",
    );
    doc.add_meeting_summary(0, "#### Meeting Summary\n**Key Decisions:** standup decision");
    let text = doc.to_text();
    let summary_pos = text.find("#### Meeting Summary").unwrap();
    let review_pos = text.find("### Review").unwrap();
    assert!(summary_pos < review_pos, "summary should not pass the Review meeting: {}", text);
    assert!(text.contains("- review note"), "review notes must be intact: {}", text);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test add_meeting_summary
```

Expected: FAIL with "no method named `add_meeting_summary`"

- [ ] **Step 3: Implement `normalize_summary` and `Document::add_meeting_summary`**

In `src/model/writer.rs`, add the private helper after the `METADATA_FIELD_ORDER` constant block (after `set_metadata_field`), and add the method inside `impl Document`:

Add a standalone private function near the bottom of `writer.rs` (before `#[cfg(test)]`):

```rust
fn normalize_summary(summary: &str) -> String {
    let summary = summary.trim();
    if summary.starts_with("#### Meeting Summary") {
        summary.to_string()
    } else {
        format!("#### Meeting Summary\n{}", summary)
    }
}
```

Add to the `impl Document` block (anywhere after `add_section_heading`):

```rust
/// Append an AI-generated meeting summary at the end of the given meeting's
/// section. If a `#### Meeting Summary` block already exists, it is replaced.
/// The summary is normalized to ensure it starts with `#### Meeting Summary`.
pub fn add_meeting_summary(&mut self, meeting_ordinal: usize, summary: &str) {
    let meetings = self.meetings();
    let Some(meeting) = meetings.get(meeting_ordinal) else {
        return;
    };
    let heading_line = meeting.heading_line;

    // Exclusive end index of this meeting's section.
    let section_end = self.lines[heading_line + 1..]
        .iter()
        .position(|line| line.starts_with("### ") || line.starts_with("## "))
        .map(|i| heading_line + 1 + i)
        .unwrap_or(self.lines.len());

    // Check if an existing "#### Meeting Summary" is already present.
    let existing_start = self.lines[heading_line..section_end]
        .iter()
        .position(|line| line == "#### Meeting Summary")
        .map(|i| heading_line + i);

    let summary_lines: Vec<String> = normalize_summary(summary)
        .lines()
        .map(|l| l.to_string())
        .collect();

    if let Some(start) = existing_start {
        // Replace from the existing heading to the section end.
        self.lines.splice(start..section_end, summary_lines);
    } else {
        // Insert after the last non-blank content line in the section.
        let mut insert_at = section_end;
        while insert_at > heading_line + 1
            && self.lines.get(insert_at - 1).map_or(false, |l| l.trim().is_empty())
        {
            insert_at -= 1;
        }

        let mut to_insert = Vec::new();
        // Add a blank separator if the preceding line is non-blank.
        if self
            .lines
            .get(insert_at.saturating_sub(1))
            .map_or(false, |l| !l.trim().is_empty())
        {
            to_insert.push(String::new());
        }
        to_insert.extend(summary_lines);

        self.lines.splice(insert_at..insert_at, to_insert);
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```
cargo test add_meeting_summary
```

Expected: all 5 tests PASS

- [ ] **Step 5: Run full test suite to check for regressions**

```
cargo test
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/model/writer.rs
git commit -m "feat: add Document::add_meeting_summary to writer"
```

---

## Task 3: `state.rs` — Meeting assistant fields and event handling

**Files:**
- Modify: `src/app/state.rs`

- [ ] **Step 1: Add field tests**

Add to `#[cfg(test)] mod tests` in `src/app/state.rs`:

```rust
#[test]
fn chat_state_default_has_no_meeting_ordinal() {
    let state = ChatState::default();
    assert_eq!(state.meeting_ordinal, None);
    assert!(!state.summarizing);
}

#[test]
fn save_chat_uses_meeting_sidecar_when_ordinal_set() {
    use crate::app::state::{ChatMessage, ChatRole};
    let tmp = tempfile::tempdir().unwrap();
    let config = Config::default();
    let date = NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
    let mut state = AppState::open_day(tmp.path().to_path_buf(), config, date).unwrap();

    state.chat.meeting_ordinal = Some(0);
    state.chat.messages = vec![
        ChatMessage { role: ChatRole::Assistant, content: "hello meeting".into() },
    ];
    state.save_chat().unwrap();

    // Meeting sidecar should exist
    let meeting_path = crate::storage::meeting_chat_path_for(
        tmp.path(), date, "%Y-%m-%d-%a", 0
    );
    assert!(meeting_path.exists(), "meeting sidecar should be written");

    // Daily sidecar should NOT exist (no daily messages)
    let daily_path = crate::storage::chat_path_for(tmp.path(), date, "%Y-%m-%d-%a");
    assert!(!daily_path.exists(), "daily sidecar should be empty/absent");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test chat_state_default_has_no_meeting_ordinal
cargo test save_chat_uses_meeting_sidecar_when_ordinal_set
```

Expected: FAIL (fields don't exist yet)

- [ ] **Step 3: Add `meeting_ordinal` and `summarizing` to `ChatState`**

In `src/app/state.rs`, replace the `ChatState` struct definition:

```rust
#[derive(Default)]
pub struct ChatState {
    pub visible: bool,
    pub messages: Vec<ChatMessage>,
    pub pending: bool,
    pub active_request: u64,
    pub scroll: usize,
    pub status: Option<String>,
    pub event_tx: Option<std::sync::mpsc::Sender<crate::app::llm::LlmEvent>>,
    /// Some(n) when a per-meeting assistant session is active for meeting ordinal n.
    pub meeting_ordinal: Option<usize>,
    /// True when the current pending LLM request is a summary generation request.
    /// On Done, the response is written to the document and meeting mode is cleared.
    pub summarizing: bool,
}
```

- [ ] **Step 4: Update `save_chat()` to use meeting sidecar when appropriate**

In `src/app/state.rs`, replace the `save_chat` method:

```rust
pub fn save_chat(&self) -> anyhow::Result<()> {
    let path = if let Some(ord) = self.chat.meeting_ordinal {
        storage::meeting_chat_path_for(
            &self.notes_dir,
            self.date,
            &self.config.date_format,
            ord,
        )
    } else {
        storage::chat_path_for(&self.notes_dir, self.date, &self.config.date_format)
    };
    storage::save_chat(&path, &self.chat.messages)
}
```

- [ ] **Step 5: Run the new state tests**

```
cargo test chat_state_default_has_no_meeting_ordinal
cargo test save_chat_uses_meeting_sidecar_when_ordinal_set
```

Expected: both PASS

- [ ] **Step 6: Add `apply_meeting_summary()` method**

In `src/app/state.rs`, add the following method to `impl AppState`, after `handle_llm_event`:

```rust
/// Called when a summary LLM request finishes. Writes the summary to the document,
/// saves it, then switches back to the daily chat sidecar.
fn apply_meeting_summary(&mut self) {
    let Some(ord) = self.chat.meeting_ordinal else {
        return;
    };
    let summary = match self.chat.messages.last() {
        Some(m) if m.role == ChatRole::Assistant && !m.content.is_empty() => {
            m.content.clone()
        }
        _ => return,
    };
    self.doc.add_meeting_summary(ord, &summary);
    let _ = self.save();
    self.selectables = self.doc.selectables();
    self.panel_agenda = crate::ui::right_panel::collect_agenda_items(&self.doc);
    self.dates_with_notes =
        crate::storage::dates_with_notes(&self.notes_dir, &self.config.date_format);

    // Switch back to daily chat
    let _ = self.save_chat(); // flush the meeting sidecar one last time
    self.chat.meeting_ordinal = None;
    let daily_path =
        storage::chat_path_for(&self.notes_dir, self.date, &self.config.date_format);
    self.chat.messages = storage::load_chat(&daily_path);
    self.chat.scroll = 0;
}
```

- [ ] **Step 7: Update `handle_llm_event()` to call `apply_meeting_summary` on Done**

In `src/app/state.rs`, replace the `Done` arm in `handle_llm_event`:

```rust
LlmEvent::Done { .. } => {
    self.chat.pending = false;
    let _ = self.save_chat();
    if self.chat.summarizing {
        self.chat.summarizing = false;
        self.apply_meeting_summary();
    }
}
```

- [ ] **Step 8: Write an integration test for the summary-on-done flow**

Add to `#[cfg(test)] mod tests` in `src/app/state.rs`:

```rust
#[test]
fn apply_meeting_summary_writes_to_doc_and_resets_mode() {
    use crate::app::state::{ChatMessage, ChatRole};
    let tmp = tempfile::tempdir().unwrap();
    let config = Config::default();
    let date = NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
    let mut state = AppState::open_day(tmp.path().to_path_buf(), config, date).unwrap();

    // Set up a meeting
    state.doc = crate::model::day::Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
    );
    state.chat.meeting_ordinal = Some(0);
    state.chat.summarizing = true;
    state.chat.active_request = 42;
    state.chat.messages = vec![
        ChatMessage {
            role: ChatRole::Assistant,
            content: "#### Meeting Summary\n**Key Decisions:** Ship it".into(),
        },
    ];

    state.handle_llm_event(LlmEvent::Done { id: 42 });

    // Summary should now be in the document
    let text = state.doc.to_text();
    assert!(text.contains("#### Meeting Summary"), "summary missing: {}", text);
    assert!(text.contains("**Key Decisions:** Ship it"), "content missing: {}", text);

    // Meeting mode should be cleared
    assert_eq!(state.chat.meeting_ordinal, None);
    assert!(!state.chat.summarizing);
}
```

- [ ] **Step 9: Run the new test**

```
cargo test apply_meeting_summary_writes_to_doc_and_resets_mode
```

Expected: PASS

- [ ] **Step 10: Run full test suite**

```
cargo test
```

Expected: all pass

- [ ] **Step 11: Commit**

```bash
git add src/app/state.rs
git commit -m "feat: add meeting assistant fields and summary-on-done to ChatState"
```

---

## Task 4: `actions.rs` — `handle_start`, `handle_end`, `handle_ask`, context injection

**Files:**
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Write tests for the new behaviors**

Add to `#[cfg(test)] mod tests` in `src/app/actions.rs`:

```rust
#[test]
fn start_in_meeting_activates_meeting_assistant_mode() {
    use crate::app::llm::LlmEvent;
    use crate::app::state::Focus;

    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);

    // Need an LLM channel so handle_start can fire the welcome call
    let (tx, _rx) = std::sync::mpsc::channel::<LlmEvent>();
    state.chat.event_tx = Some(tx);

    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Start).unwrap();

    assert_eq!(state.chat.meeting_ordinal, Some(0), "meeting ordinal should be set");
    assert!(state.chat.visible, "chat should be visible");
    assert_eq!(state.focus, Focus::Chat, "focus should shift to chat");
    assert!(state.chat.pending, "LLM call should be pending");
}

#[test]
fn start_without_llm_channel_still_activates_meeting_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    // No event_tx set

    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Start).unwrap();

    // Meeting mode should still activate even if LLM is unavailable
    assert_eq!(state.chat.meeting_ordinal, Some(0));
    assert!(state.chat.visible);
}

#[test]
fn end_in_meeting_mode_triggers_summary_generation() {
    use crate::app::llm::LlmEvent;

    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);

    let (tx, _rx) = std::sync::mpsc::channel::<LlmEvent>();
    state.chat.event_tx = Some(tx);

    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Start).unwrap();

    // Drain the pending welcome call
    state.chat.pending = false;
    state.chat.messages.clear();

    dispatch(&mut state, Command::End).unwrap();

    assert!(state.chat.pending, "summary LLM call should be pending");
    assert!(state.chat.summarizing, "summarizing flag should be set");
}

#[test]
fn end_outside_meeting_mode_does_not_trigger_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);

    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    // Do NOT call /start — so meeting_ordinal is None

    dispatch(&mut state, Command::End).unwrap();

    assert!(!state.chat.pending, "no LLM call should fire without meeting assistant mode");
    assert_eq!(state.chat.meeting_ordinal, None);
}

#[test]
fn ask_in_meeting_mode_uses_meeting_system_prompt() {
    use crate::app::llm::LlmEvent;

    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);

    let (tx, _rx) = std::sync::mpsc::channel::<LlmEvent>();
    state.chat.event_tx = Some(tx);

    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    state.chat.meeting_ordinal = Some(0);

    dispatch(&mut state, Command::Ask("what's on the agenda?".to_string())).unwrap();

    // User message should be in chat history
    assert!(
        state.chat.messages.iter().any(|m| m.content.contains("what's on the agenda?")),
        "user message should be in history"
    );
    assert!(state.chat.pending);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test start_in_meeting_activates_meeting_assistant_mode
cargo test start_without_llm_channel_still_activates_meeting_mode
cargo test end_in_meeting_mode_triggers_summary_generation
cargo test end_outside_meeting_mode_does_not_trigger_summary
cargo test ask_in_meeting_mode_uses_meeting_system_prompt
```

Expected: FAILs (new behaviors not yet implemented)

- [ ] **Step 3: Add the system prompt constant and helpers**

In `src/app/actions.rs`, add near the top (after the `use` statements):

```rust
const MEETING_ASSISTANT_SYSTEM_PROMPT: &str = "\
You are a meeting assistant embedded in a note-taking app.
You will be given the current state of a meeting (title, metadata, bullet-point notes).
Your job:
- Understand what the meeting is about
- Send a concise opening message when the meeting starts, acknowledging what you know \
and asking one clarifying question if something is unclear
- Respond helpfully when the user asks questions during the meeting
- When asked to summarize, produce a structured summary using ONLY the sections that \
have relevant content:

#### Meeting Summary
**Key Decisions:** ...
**Action Items:** ...
**Discussion Highlights:** ...
**Open Questions:** ...

Keep responses short and direct — this is a live meeting.";
```

Then add the two private helpers after `handle_clear` (before `handle_start`):

```rust
/// Build the meeting context injection message for ordinal `meeting_ordinal`.
/// Returns None if the meeting doesn't exist.
fn build_meeting_context_message(
    state: &AppState,
    meeting_ordinal: usize,
) -> Option<crate::app::state::ChatMessage> {
    let meeting = state.doc.meetings().into_iter().nth(meeting_ordinal)?;
    let heading_line = meeting.heading_line;

    let section_end = state.doc.lines[heading_line + 1..]
        .iter()
        .position(|line| line.starts_with("### ") || line.starts_with("## "))
        .map(|i| heading_line + 1 + i)
        .unwrap_or(state.doc.lines.len());

    let section_text = state.doc.lines[heading_line..section_end].join("\n");

    Some(crate::app::state::ChatMessage {
        role: crate::app::state::ChatRole::User,
        content: format!("Here is the current state of the meeting:\n{}", section_text),
    })
}

/// Fire an LLM call in meeting assistant mode.
/// The trigger_message is sent to the LLM but NOT stored in chat history
/// (it is only an API mechanic). If `is_summary` is true, the `summarizing`
/// flag is set so the response is written to the document on completion.
fn fire_meeting_llm_call(
    state: &mut AppState,
    trigger_message: &str,
    is_summary: bool,
) -> anyhow::Result<()> {
    let Some(tx) = state.chat.event_tx.clone() else {
        state.chat.status = Some("LLM channel unavailable".to_string());
        return Ok(());
    };
    let Some(ord) = state.chat.meeting_ordinal else {
        return Ok(());
    };

    state.chat.status = None;
    state.chat.scroll = 0;
    state.chat.summarizing = is_summary;

    // Build: [context injection] + [conversation history] + [trigger message]
    let mut request_messages = Vec::new();
    if let Some(ctx_msg) = build_meeting_context_message(state, ord) {
        request_messages.push(ctx_msg);
    }
    request_messages.extend(state.chat.messages.clone());
    request_messages.push(crate::app::state::ChatMessage {
        role: crate::app::state::ChatRole::User,
        content: trigger_message.to_string(),
    });

    let id = crate::app::llm::next_request_id();
    state.chat.active_request = id;
    state.chat.pending = true;
    state.chat.messages.push(crate::app::state::ChatMessage {
        role: crate::app::state::ChatRole::Assistant,
        content: String::new(),
    });

    let req = crate::app::llm::ChatRequest {
        id,
        base_url: state.config.llm_base_url.clone(),
        model: state.config.llm_model.clone(),
        system: Some(MEETING_ASSISTANT_SYSTEM_PROMPT.to_string()),
        messages: request_messages,
    };
    crate::app::llm::spawn(req, tx);
    Ok(())
}
```

- [ ] **Step 4: Replace `handle_start()`**

In `src/app/actions.rs`, replace the existing `handle_start` function:

```rust
fn handle_start(state: &mut AppState) -> anyhow::Result<()> {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => {
            state.status = "Not in a meeting".to_string();
            return Ok(());
        }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        let time = state.current_time_hhmm();
        crate::model::writer::set_metadata_field(
            &mut state.doc.lines,
            heading,
            "Started",
            &time,
        );
        after_doc_mutation(state)?;
    }

    // Save current daily chat before switching to meeting sidecar.
    let _ = state.save_chat();

    // Load (or start empty) per-meeting sidecar.
    let meeting_path = crate::storage::meeting_chat_path_for(
        &state.notes_dir,
        state.date,
        &state.config.date_format,
        ord,
    );
    state.chat.messages = crate::storage::load_chat(&meeting_path);
    state.chat.meeting_ordinal = Some(ord);
    state.chat.scroll = 0;
    state.chat.status = None;

    // Show and focus the chat panel.
    state.chat.visible = true;
    state.focus = crate::app::state::Focus::Chat;

    // Trigger the AI welcome message.
    fire_meeting_llm_call(state, "Meeting started.", false)?;

    Ok(())
}
```

- [ ] **Step 5: Replace `handle_end()`**

In `src/app/actions.rs`, replace the existing `handle_end` function:

```rust
fn handle_end(state: &mut AppState) -> anyhow::Result<()> {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => {
            state.status = "Not in a meeting".to_string();
            return Ok(());
        }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        let time = state.current_time_hhmm();
        crate::model::writer::set_metadata_field(
            &mut state.doc.lines,
            heading,
            "Ended",
            &time,
        );
        after_doc_mutation(state)?;
    }

    // If meeting assistant mode is active for this meeting, trigger summary generation.
    if state.chat.meeting_ordinal == Some(ord) {
        fire_meeting_llm_call(
            state,
            "The meeting has ended. Please generate the meeting summary now.",
            true,
        )?;
    }

    Ok(())
}
```

- [ ] **Step 6: Update `handle_ask()` to inject meeting context**

In `src/app/actions.rs`, replace the existing `handle_ask` function:

```rust
fn handle_ask(state: &mut AppState, text: &str) -> anyhow::Result<()> {
    let Some(tx) = state.chat.event_tx.clone() else {
        state.chat.status = Some("LLM channel unavailable".to_string());
        return Ok(());
    };
    state.chat.visible = true;
    state.chat.status = None;
    state.chat.scroll = 0;

    // Add the user message to history (both modes).
    state.chat.messages.push(crate::app::state::ChatMessage {
        role: crate::app::state::ChatRole::User,
        content: text.to_string(),
    });
    let _ = state.save_chat();

    // Meeting assistant mode: inject fresh meeting context before history.
    if let Some(ord) = state.chat.meeting_ordinal {
        let mut request_messages = Vec::new();
        if let Some(ctx_msg) = build_meeting_context_message(state, ord) {
            request_messages.push(ctx_msg);
        }
        request_messages.extend(state.chat.messages.clone());

        let id = crate::app::llm::next_request_id();
        state.chat.active_request = id;
        state.chat.pending = true;
        state.chat.summarizing = false;
        state.chat.messages.push(crate::app::state::ChatMessage {
            role: crate::app::state::ChatRole::Assistant,
            content: String::new(),
        });

        let req = crate::app::llm::ChatRequest {
            id,
            base_url: state.config.llm_base_url.clone(),
            model: state.config.llm_model.clone(),
            system: Some(MEETING_ASSISTANT_SYSTEM_PROMPT.to_string()),
            messages: request_messages,
        };
        crate::app::llm::spawn(req, tx);
        return Ok(());
    }

    // Standard mode (existing behavior).
    let request_messages = state.chat.messages.clone();
    let id = crate::app::llm::next_request_id();
    state.chat.active_request = id;
    state.chat.pending = true;
    state.chat.messages.push(crate::app::state::ChatMessage {
        role: crate::app::state::ChatRole::Assistant,
        content: String::new(),
    });
    let system = if state.config.llm_system_prompt.is_empty() {
        None
    } else {
        Some(state.config.llm_system_prompt.clone())
    };
    let req = crate::app::llm::ChatRequest {
        id,
        base_url: state.config.llm_base_url.clone(),
        model: state.config.llm_model.clone(),
        system,
        messages: request_messages,
    };
    crate::app::llm::spawn(req, tx);
    Ok(())
}
```

- [ ] **Step 7: Run the new action tests**

```
cargo test start_in_meeting_activates_meeting_assistant_mode
cargo test start_without_llm_channel_still_activates_meeting_mode
cargo test end_in_meeting_mode_triggers_summary_generation
cargo test end_outside_meeting_mode_does_not_trigger_summary
cargo test ask_in_meeting_mode_uses_meeting_system_prompt
```

Expected: all PASS

- [ ] **Step 8: Run the full test suite**

```
cargo test
```

Expected: all tests pass

- [ ] **Step 9: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat: activate meeting assistant on /start, generate summary on /end"
```

---

## Task 5: `ui/chat_panel.rs` — Meeting-aware header

**Files:**
- Modify: `src/ui/chat_panel.rs`

- [ ] **Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests` in `src/ui/chat_panel.rs`:

```rust
#[test]
fn render_shows_meeting_name_in_header_when_in_meeting_mode() {
    use crate::app::state::ChatMessage;

    let tmp = tempfile::tempdir().unwrap();
    let config = crate::config::Config::default();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
    let mut app = AppState::open_day(tmp.path().to_path_buf(), config, date).unwrap();

    // Set up a meeting in the document
    app.doc = crate::model::day::Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n",
    );
    app.chat.visible = true;
    app.chat.meeting_ordinal = Some(0);

    let backend = ratatui::backend::TestBackend::new(40, 20);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
        .unwrap();
    let content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        content.contains("Chat [Standup]"),
        "header should show meeting name, got: {}",
        content
    );
}

#[test]
fn render_shows_plain_chat_header_outside_meeting_mode() {
    let app = app_with_messages(vec![]);
    // meeting_ordinal is None by default
    let backend = ratatui::backend::TestBackend::new(40, 20);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render(f, f.area(), &app, &crate::ui::theme::light()))
        .unwrap();
    let content: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content.contains("Chat"), "header should show 'Chat': {}", content);
    assert!(
        !content.contains("Chat ["),
        "should not show meeting bracket outside meeting mode: {}",
        content
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test render_shows_meeting_name_in_header_when_in_meeting_mode
cargo test render_shows_plain_chat_header_outside_meeting_mode
```

Expected: `render_shows_meeting_name_in_header_when_in_meeting_mode` FAILS (still renders "Chat"), second passes.

- [ ] **Step 3: Update the header rendering**

In `src/ui/chat_panel.rs`, replace the header widget construction (lines 50–55):

```rust
let header_text = if let Some(ord) = app.chat.meeting_ordinal {
    let meetings = app.doc.meetings();
    if let Some(m) = meetings.get(ord) {
        format!("Chat [{}]", m.name)
    } else {
        "Chat".to_string()
    }
} else {
    "Chat".to_string()
};
let header = Paragraph::new(header_text).style(
    Style::default()
        .bg(theme.chat_panel_bg)
        .add_modifier(Modifier::BOLD),
);
frame.render_widget(header, chunks[0]);
```

Note: `app.doc.meetings()` is already a public method on `Document` (defined in `src/model/writer.rs`). The `AppState` is available as `app` and has `app.doc: Document`.

- [ ] **Step 4: Run tests**

```
cargo test render_shows_meeting_name_in_header_when_in_meeting_mode
cargo test render_shows_plain_chat_header_outside_meeting_mode
```

Expected: both PASS

- [ ] **Step 5: Run full test suite**

```
cargo test
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/ui/chat_panel.rs
git commit -m "feat: show meeting name in chat panel header when in meeting mode"
```

---

## Final Verification

- [ ] **Build the binary**

```
cargo build
```

Expected: compiles without errors or warnings

- [ ] **Run all tests one last time**

```
cargo test
```

Expected: all tests pass

- [ ] **Manual smoke test**

Launch the app (`cargo run`) and:
1. Type `/meeting Standup` → context shows "Standup"
2. Type `/start` → chat panel shows "Chat [Standup]", AI welcome message begins streaming
3. Add a few notes under the meeting (plain entries)
4. Type `/ask what's the main topic?` → AI responds with meeting context
5. Type `/end` → AI generates summary, `#### Meeting Summary` appears in the document

---

## Error Cases Covered by Existing Tests

- `/start` outside a meeting: `start_outside_meeting_sets_status` (pre-existing test) — behavior unchanged
- `/end` outside a meeting: `end_outside_meeting_sets_status` (pre-existing test) — behavior unchanged
- `/end` without having called `/start` (no `meeting_ordinal`): `end_outside_meeting_mode_does_not_trigger_summary` — new test
- LLM channel unavailable on `/start`: `start_without_llm_channel_still_activates_meeting_mode` — new test
- Invalid meeting ordinal in `add_meeting_summary`: `add_meeting_summary_noop_for_invalid_ordinal` — new test
