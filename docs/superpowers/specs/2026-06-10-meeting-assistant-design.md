# Meeting Assistant Design

**Date:** 2026-06-10  
**Status:** Approved

---

## Overview

When a meeting is started with `/start`, the chat panel transforms into a meeting-aware AI assistant. The assistant receives the current state of the meeting document as context on every LLM call, sends a proactive opening message, and at `/end` generates a structured summary that is streamed into the chat and written into the document.

---

## Goals

- Make the chat panel useful during live meetings with zero extra commands
- Scope each meeting's AI conversation independently from the daily chat
- Preserve all existing `/start` and `/end` command behavior
- Write AI-generated summaries directly into the meeting section of the document

---

## Non-Goals (v1)

- Event-driven notes: automatically firing LLM calls when each note is added
- A clarification Q&A phase before writing the summary
- Configurable system prompt for meeting assistant mode
- Attendee tracking in the summary

---

## Approach: Document-as-Context, Explicit Triggers

Every LLM call made during a meeting session injects the **current meeting section content** freshly from the document. Notes do not individually trigger LLM calls — the document state at call time is the context. The AI is triggered at three points:

1. `/start` — sends a proactive welcome/opening message
2. `/ask <message>` — user-initiated chat (works as before, but with meeting context)
3. `/end` — generates a structured summary, writes it to the document

---

## Section 1: Storage & State

### Per-Meeting Chat Sidecars

Each meeting gets its own JSON chat sidecar file, independent of the daily chat sidecar:

```
{date}.md                        # daily notes (e.g., 2026-06-10-Thu.md)
{date}.chat.json                 # daily general chat
{date}-meeting0.chat.json        # meeting 0 assistant conversation
{date}-meeting1.chat.json        # meeting 1 assistant conversation
```

The daily `.chat.json` is never touched during a meeting session. The per-meeting sidecar persists the AI conversation for post-meeting review.

### `storage.rs` — New Helper

```rust
pub fn meeting_chat_path(date: NaiveDate, ordinal: usize) -> PathBuf
```

Returns the path to `{date}-meeting{ordinal}.chat.json` in the same directory as the daily note.

### `ChatState` — New Fields

```rust
pub struct ChatState {
    // existing fields ...
    pub meeting_ordinal: Option<usize>,  // Some(n) = meeting assistant active
}
```

When `meeting_ordinal` is `Some(n)`, all chat saves/loads use the per-meeting sidecar. When it reverts to `None` on `/end`, the daily sidecar is reloaded.

---

## Section 2: Command Flow

### `/start` (enhanced)

1. **Existing:** write `meta:Started: HH:MM` to the meeting heading *(preserved)*
2. Save current `ChatState.messages` to the daily sidecar
3. Load per-meeting sidecar (empty if first time) into `ChatState`; set `meeting_ordinal = Some(ord)` where `ord` comes from `state.context` (`Context::Meeting(ord)`)
4. Ensure chat panel is visible (`chat.visible = true`)
5. Shift focus to the chat panel
6. Extract the current meeting section content from the document
7. Trigger an LLM call: system prompt (meeting assistant) + meeting content injection + synthetic user message `"Meeting started."` (required by LLM API to produce a response; the system prompt instructs the AI what to do with it)

### `/end` (enhanced)

1. **Existing:** write `meta:Ended: HH:MM` to the meeting heading *(preserved)*
2. If `meeting_ordinal` is `None`: no meeting assistant session active, done
3. Extract the full current meeting section content from the document
4. Trigger an LLM call with the summary generation directive (see Section 5)
5. Stream the summary into the chat panel
6. On `LlmEvent::Done`: extract the summary text from the last assistant message, call `writer::add_meeting_summary()` to append it to the document
7. Save the per-meeting sidecar; clear `meeting_ordinal` (set to `None`)
8. Load daily sidecar back into `ChatState`

---

## Section 3: Context Injection

### Message List Construction

Every LLM call while `meeting_ordinal` is `Some(n)` builds the message list as follows. **The injected context message is NOT stored in `ChatState.messages`** — it is assembled fresh at call time so the document's current state is always used.

```
[0] role: system   → meeting assistant system prompt (replaces llm_system_prompt)
[1] role: user     → "Here is the current state of the meeting:\n{meeting_content}"
[2..N] role: *     → stored ChatState.messages (actual conversation history)
[N+1] role: user   → new user message (if any; absent for /start and /end triggers)
```

### Meeting Content Format

The injected meeting content is the raw Markdown of the meeting section, from the `### HH:MM Name` heading line through the last non-blank line before the next `###` or `##` heading (or end of document):

```
### 09:15 Standup
meta:Purpose: align on sprint
meta:Scheduled: 09:15
meta:Started: 09:17
- Alice: shipped auth PR
- Bob: blocked on infra
- Need to sync with platform team
```

### Implementation

`actions.rs` gets a helper `build_meeting_context_messages(state) -> Vec<ChatMessage>` that:
1. Checks `state.chat.meeting_ordinal`
2. If `Some(n)`, extracts meeting section text from `state.document`
3. Returns the injected context messages to prepend

`llm::spawn()` already receives a `Vec<ChatMessage>`. The assembled list (context injection + history + new message) is passed directly — no signature change required.

---

## Section 4: Summary Writing

### `model/writer.rs` — New Function

```rust
pub fn add_meeting_summary(doc: &mut Document, meeting_ordinal: usize, summary: &str)
```

**Logic:**
1. Find the meeting's heading line via `doc.meetings[meeting_ordinal].heading_line`
2. Find the section end: the next `### ` or `## ` heading line, or end of document
3. Scan backward from the section end to find the last non-blank line
4. Check if a `#### Meeting Summary` block already exists in the section:
   - If yes: remove the old block (from `#### Meeting Summary` to the section end)
   - If no: proceed to insert
5. Ensure there is a blank line before the insertion point
6. Insert the summary text (which starts with `#### Meeting Summary`)

### Summary Integrity

If the AI response does not start with `#### Meeting Summary` (malformed output), the writer prefixes the heading automatically before inserting. The raw AI response is always what's streamed in the chat panel; the document write uses the normalized version.

---

## Section 5: UI & System Prompt

### Chat Panel Header

When `meeting_ordinal` is `Some(n)`:
- Panel title changes from `"Chat"` to `"Chat [MeetingName]"`
- `MeetingName` is `doc.meetings[n].name`

This makes meeting assistant mode visually distinct.

### Auto-Visibility

On `/start`, the chat panel is made visible and focus shifts to it so the AI's opening message is immediately seen. The user can `Tab` back to the notes pane to continue entering notes.

### Meeting Assistant System Prompt

Hardcoded in v1 (not user-configurable). Replaces `llm_system_prompt` while `meeting_ordinal` is `Some(n)`:

```
You are a meeting assistant embedded in a note-taking app.
You will be given the current state of a meeting (title, metadata, bullet-point notes).
Your job:
- Understand what the meeting is about
- Send a concise opening message when the meeting starts, acknowledging what you know and asking one clarifying question if something is unclear
- Respond helpfully when the user asks questions during the meeting
- When asked to summarize, produce a structured summary using ONLY the sections that have relevant content:

#### Meeting Summary
**Key Decisions:** ...
**Action Items:** ...
**Discussion Highlights:** ...
**Open Questions:** ...

Keep responses short and direct — this is a live meeting.
```

The `/end` summary request is sent as a user message appended to the conversation:
```
"The meeting has ended. Please generate the meeting summary now."
```

---

## Data Flow Diagram

```
/start issued
    │
    ├─ write meta:Started (existing)
    ├─ save daily chat sidecar
    ├─ load meeting0 sidecar → ChatState
    ├─ set meeting_ordinal = Some(0)
    ├─ show + focus chat panel
    └─ LLM call: [system: meeting prompt] + [user: meeting content]
                        │
                        └─ AI streams opening message → chat panel

Notes added (no automatic LLM trigger)
    │
    └─ document updated only; context injected fresh on next LLM call

/ask <message>
    │
    └─ LLM call: [system] + [user: meeting content] + [history] + [user: message]
                        │
                        └─ AI streams response → chat panel

/end issued
    │
    ├─ write meta:Ended (existing)
    ├─ LLM call: [system] + [user: meeting content] + [history] + [user: "summarize now"]
    │                   │
    │                   └─ AI streams summary → chat panel
    ├─ on Done: writer::add_meeting_summary() → document
    ├─ save meeting0 sidecar
    ├─ set meeting_ordinal = None
    └─ load daily chat sidecar → ChatState
```

---

## Files Changed

| File | Change |
|------|--------|
| `src/storage.rs` | Add `meeting_chat_path()` |
| `src/app/state.rs` | Add `meeting_ordinal: Option<usize>` to `ChatState` |
| `src/app/actions.rs` | Enhance `handle_start()`, `handle_end()`, add `build_meeting_context_messages()` |
| `src/app/llm.rs` | No signature change required; called with pre-assembled message list |
| `src/model/writer.rs` | Add `add_meeting_summary()` |
| `src/ui/chat_panel.rs` | Render `"Chat [MeetingName]"` header when in meeting mode |

---

## Error Cases

- `/start` outside a meeting context (`Context::Meeting`): no-op; existing "Not in a meeting" status message
- `/end` with `meeting_ordinal = None`: existing behavior only (write Ended time)
- LLM error during summary generation: status message shown; no document write; meeting_ordinal cleared anyway
- LLM unavailable: same handling as existing `/ask` errors

---

## Future Enhancements (v2+)

- Configurable system prompt for meeting assistant mode via `config.toml`
- End-of-meeting clarification phase before writing summary (Approach C)
- Ability to re-run `/end` to regenerate summary after adding more notes
