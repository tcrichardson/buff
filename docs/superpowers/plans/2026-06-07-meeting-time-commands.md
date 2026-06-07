# Meeting Time Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the auto-timestamp prefix from meeting headings and add `/start`, `/end`, and `/scheduled` commands that insert labeled time lines at the top of the active meeting; show the current time in the header.

**Architecture:** New commands are parsed in `command.rs`, dispatched in `actions.rs`, and use a new `set_meeting_time_field` function in `writer.rs` that manages a canonical `Scheduled/Started/Ended` metadata block at the top of each meeting. The header reads `chrono::Local::now()` inline at render time — no new state needed since the event loop already redraws every ~100 ms.

**Tech Stack:** Rust, ratatui, chrono, crossterm. No new dependencies.

---

## File Map

| File | Change |
|---|---|
| `src/model/writer.rs` | Remove `time` param from `add_meeting`; add `set_meeting_time_field`; update tests |
| `src/app/command.rs` | Add `Start`, `End`, `Scheduled(String)` variants; parse them; add tests |
| `src/app/actions.rs` | Update `add_meeting` call; dispatch 3 new commands; add tests |
| `src/ui/layout.rs` | Append `HH:MM` clock to header date line; update test |
| `src/ui/help.rs` | Add new commands to help overlay text |

---

## Task 1: Remove time parameter from `add_meeting`

**Files:**
- Modify: `src/model/writer.rs`

- [ ] **Step 1: Update the `add_meeting` signature and body**

In `src/model/writer.rs`, replace the function at line 103:

```rust
pub fn add_meeting(&mut self, name: &str) -> usize {
    let start = ensure_section(&mut self.lines, SectionKind::Meetings);
    let end = section_end(&self.lines, start);
    let insert_idx = block_insert_index(&self.lines, start, end);
    let line = format!("### {}", name);
    self.lines.insert(insert_idx, line);

    let mut ordinal = 0;
    for i in start + 1..insert_idx {
        if self.lines[i].starts_with("### ") {
            ordinal += 1;
        }
    }
    ordinal
}
```

- [ ] **Step 2: Update existing tests in `writer.rs` that call `add_meeting` with two args**

Find all calls in the test module and update them — four tests are affected:

```rust
// add_meeting_to_empty_doc — was: doc.add_meeting("09:15", "Standup")
#[test]
fn add_meeting_to_empty_doc() {
    let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let ord = doc.add_meeting("Standup");
    assert_eq!(ord, 0);
    let meetings = doc.meetings();
    assert_eq!(meetings.len(), 1);
    assert_eq!(meetings[0].ordinal, 0);
    assert_eq!(meetings[0].time, "");
    assert_eq!(meetings[0].name, "Standup");
}

// add_two_meetings — was: doc.add_meeting("09:15", "Standup") / ("10:00", "Review")
#[test]
fn add_two_meetings() {
    let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let ord0 = doc.add_meeting("Standup");
    let ord1 = doc.add_meeting("Review");
    assert_eq!(ord0, 0);
    assert_eq!(ord1, 1);
    let meetings = doc.meetings();
    assert_eq!(meetings.len(), 2);
    assert_eq!(meetings[0].name, "Standup");
    assert_eq!(meetings[1].name, "Review");
}

// add_todo_always_in_todos_section — was: doc.add_meeting("09:15", "Standup")
#[test]
fn add_todo_always_in_todos_section() {
    let mut doc = Document::from_text(
        "# 2026-06-04\n\n## Meetings\n\n### 09:15 Standup\n\n## Notes\n\n## To-dos\n",
    );
    doc.add_meeting("Standup");
    doc.add_todo("Action item", None);
    let text = doc.to_text();

    let todos_start = text.find("## To-dos").unwrap();
    let todos_section = &text[todos_start..];
    assert!(todos_section.contains("- [ ] Action item"));
}

// add_meeting_creates_missing_meetings_section — was: doc.add_meeting("09:15", "Standup")
#[test]
fn add_meeting_creates_missing_meetings_section() {
    let mut doc = Document::from_text("# Title\n\n## Notes\n\n## To-dos\n");
    doc.add_meeting("Standup");
    let text = doc.to_text();
    assert!(
        text.contains("## Meetings\n### Standup\n"),
        "got: {}",
        text
    );
}
```

- [ ] **Step 3: Run `writer.rs` tests to verify they all pass**

```bash
cargo test -p buff --lib model::writer
```

Expected: all tests pass. If compile errors appear, check for any remaining `add_meeting` call with two args in the test module.

- [ ] **Step 4: Update the call site in `actions.rs`**

In `src/app/actions.rs` at line 65, change:
```rust
// before
let ord = state.doc.add_meeting(&state.current_time_hhmm(), &name);
// after
let ord = state.doc.add_meeting(&name);
```

- [ ] **Step 5: Run all tests to verify no regressions**

```bash
cargo test -p buff
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/model/writer.rs src/app/actions.rs
git commit -m "refactor: remove auto-timestamp from meeting headings"
```

---

## Task 2: Add `Start`, `End`, `Scheduled` commands to the parser

**Files:**
- Modify: `src/app/command.rs`

- [ ] **Step 1: Write failing tests first**

Add these tests to the `#[cfg(test)]` module in `src/app/command.rs`:

```rust
#[test]
fn parse_start() {
    assert_eq!(parse("/start"), Command::Start);
}

#[test]
fn parse_end() {
    assert_eq!(parse("/end"), Command::End);
}

#[test]
fn parse_scheduled_valid() {
    assert_eq!(
        parse("/scheduled 09:00"),
        Command::Scheduled("09:00".to_string())
    );
}

#[test]
fn parse_scheduled_no_arg() {
    assert_eq!(
        parse("/scheduled"),
        Command::InvalidArgs("invalid time, use HH:MM".to_string())
    );
}

#[test]
fn parse_scheduled_bad_time() {
    assert_eq!(
        parse("/scheduled 9am"),
        Command::InvalidArgs("invalid time, use HH:MM".to_string())
    );
}

#[test]
fn parse_scheduled_out_of_range_hour() {
    assert_eq!(
        parse("/scheduled 25:00"),
        Command::InvalidArgs("invalid time, use HH:MM".to_string())
    );
}

#[test]
fn parse_scheduled_out_of_range_minute() {
    assert_eq!(
        parse("/scheduled 12:60"),
        Command::InvalidArgs("invalid time, use HH:MM".to_string())
    );
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p buff --lib app::command
```

Expected: compile error — `Command::Start`, `Command::End`, `Command::Scheduled` do not exist yet.

- [ ] **Step 3: Add variants to the `Command` enum**

In `src/app/command.rs`, add to the `Command` enum:

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    Entry(String),
    Meeting(String),
    Note(Option<String>),
    Todo(String),
    Leave,
    Goto(Option<chrono::NaiveDate>),
    Today,
    Help,
    Quit,
    Summarize,
    Ask(String),
    Clear,
    Start,
    End,
    Scheduled(String),
    Unknown(String),
    InvalidArgs(String),
}
```

- [ ] **Step 4: Add the `parse_hhmm` helper and the new match arms**

Add the helper function just before `pub fn parse`:

```rust
fn parse_hhmm(s: &str) -> bool {
    if s.len() != 5 {
        return false;
    }
    let b = s.as_bytes();
    if b[2] != b':' {
        return false;
    }
    let hh = match (b[0] as char).to_digit(10).zip((b[1] as char).to_digit(10)) {
        Some((a, b)) => a * 10 + b,
        None => return false,
    };
    let mm = match (b[3] as char).to_digit(10).zip((b[4] as char).to_digit(10)) {
        Some((a, b)) => a * 10 + b,
        None => return false,
    };
    hh <= 23 && mm <= 59
}
```

In the `match cmd` block in `pub fn parse`, add before the `_ =>` catch-all:

```rust
"/start" => Command::Start,
"/end" => Command::End,
"/scheduled" => {
    if rest.is_empty() || !parse_hhmm(rest) {
        Command::InvalidArgs("invalid time, use HH:MM".to_string())
    } else {
        Command::Scheduled(rest.to_string())
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p buff --lib app::command
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/command.rs
git commit -m "feat: add Start, End, Scheduled command variants and parsing"
```

---

## Task 3: Implement `set_meeting_time_field` in `writer.rs`

**Files:**
- Modify: `src/model/writer.rs`

- [ ] **Step 1: Write failing tests**

Add to the test module in `src/model/writer.rs`:

```rust
#[test]
fn set_time_field_inserts_started_when_absent() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_meeting_time_field(&mut doc.lines, heading, "Started", "09:15");
    let text = doc.to_text();
    let heading_pos = text.find("### Standup").unwrap();
    let started_pos = text.find("Started: 09:15").unwrap();
    let note_pos = text.find("- note").unwrap();
    assert!(started_pos > heading_pos, "Started should be after heading");
    assert!(started_pos < note_pos, "Started should be before note");
}

#[test]
fn set_time_field_overwrites_existing_started() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\nStarted: 09:00\n- note\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_meeting_time_field(&mut doc.lines, heading, "Started", "09:15");
    let text = doc.to_text();
    assert!(text.contains("Started: 09:15\n"), "should have new time: {}", text);
    assert!(!text.contains("Started: 09:00\n"), "old time should be gone: {}", text);
}

#[test]
fn set_time_field_canonical_order() {
    // Add Ended first, then Started, then Scheduled — result should be Scheduled, Started, Ended
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_meeting_time_field(&mut doc.lines, heading, "Ended", "10:00");
    set_meeting_time_field(&mut doc.lines, heading, "Started", "09:15");

    // re-fetch heading_line since lines shifted
    let heading = doc.meetings()[0].heading_line;
    set_meeting_time_field(&mut doc.lines, heading, "Scheduled", "09:00");

    let text = doc.to_text();
    let scheduled_pos = text.find("Scheduled: 09:00").unwrap();
    let started_pos = text.find("Started: 09:15").unwrap();
    let ended_pos = text.find("Ended: 10:00").unwrap();
    assert!(scheduled_pos < started_pos, "Scheduled before Started");
    assert!(started_pos < ended_pos, "Started before Ended");
}

#[test]
fn set_time_field_does_not_eat_note_content() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- note one\n- note two\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_meeting_time_field(&mut doc.lines, heading, "Started", "09:15");
    let text = doc.to_text();
    assert!(text.contains("- note one\n"), "note one should remain: {}", text);
    assert!(text.contains("- note two\n"), "note two should remain: {}", text);
}

#[test]
fn set_time_field_all_three_fields() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_meeting_time_field(&mut doc.lines, heading, "Scheduled", "09:00");
    let heading = doc.meetings()[0].heading_line;
    set_meeting_time_field(&mut doc.lines, heading, "Started", "09:05");
    let heading = doc.meetings()[0].heading_line;
    set_meeting_time_field(&mut doc.lines, heading, "Ended", "09:45");
    let text = doc.to_text();
    assert!(text.contains("Scheduled: 09:00\n"), "got: {}", text);
    assert!(text.contains("Started: 09:05\n"), "got: {}", text);
    assert!(text.contains("Ended: 09:45\n"), "got: {}", text);
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p buff --lib model::writer -- set_time_field
```

Expected: compile error — `set_meeting_time_field` not defined.

- [ ] **Step 3: Implement `set_meeting_time_field`**

Add to `src/model/writer.rs` (outside the `impl Document` block, at the bottom of the non-test code section, before the `#[cfg(test)]`):

```rust
const TIME_FIELD_ORDER: &[&str] = &["Scheduled", "Started", "Ended"];

fn is_time_field_line(line: &str) -> bool {
    TIME_FIELD_ORDER
        .iter()
        .any(|k| line.starts_with(&format!("{}: ", k)))
}

/// Insert or replace a labeled time line (`Key: HH:MM`) in the meeting's
/// metadata block — the consecutive `Key: HH:MM` lines immediately after the
/// `### heading` line. The block is always rewritten in canonical order:
/// Scheduled, Started, Ended.
///
/// `heading_line` is the index in `lines` of the `### Name` heading.
pub fn set_meeting_time_field(
    lines: &mut Vec<String>,
    heading_line: usize,
    key: &str,
    value: &str,
) {
    // Find the end of the existing metadata block.
    let mut meta_end = heading_line + 1;
    while meta_end < lines.len() && is_time_field_line(&lines[meta_end]) {
        meta_end += 1;
    }

    // Parse existing fields into a map.
    let mut fields: std::collections::HashMap<String, String> =
        lines[heading_line + 1..meta_end]
            .iter()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ": ");
                let k = parts.next()?.to_string();
                let v = parts.next()?.to_string();
                Some((k, v))
            })
            .collect();

    // Insert or overwrite the target key.
    fields.insert(key.to_string(), value.to_string());

    // Rebuild in canonical order (only keys that exist).
    let new_lines: Vec<String> = TIME_FIELD_ORDER
        .iter()
        .filter_map(|k| fields.get(*k).map(|v| format!("{}: {}", k, v)))
        .collect();

    // Replace the old metadata range with the new lines.
    lines.drain(heading_line + 1..meta_end);
    for (i, line) in new_lines.into_iter().enumerate() {
        lines.insert(heading_line + 1 + i, line);
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p buff --lib model::writer -- set_time_field
```

Expected: all 5 new tests pass.

- [ ] **Step 5: Run full test suite to verify no regressions**

```bash
cargo test -p buff
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/model/writer.rs
git commit -m "feat: add set_meeting_time_field to writer"
```

---

## Task 4: Dispatch `Start`, `End`, `Scheduled` in `actions.rs`

**Files:**
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Write failing tests**

Add to the test module in `src/app/actions.rs`:

```rust
#[test]
fn start_outside_meeting_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    // context starts as Notes
    let before = state.doc.to_text();
    dispatch(&mut state, Command::Start).unwrap();
    assert_eq!(state.status, "Not in a meeting");
    assert_eq!(state.doc.to_text(), before);
}

#[test]
fn end_outside_meeting_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    let before = state.doc.to_text();
    dispatch(&mut state, Command::End).unwrap();
    assert_eq!(state.status, "Not in a meeting");
    assert_eq!(state.doc.to_text(), before);
}

#[test]
fn scheduled_outside_meeting_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    let before = state.doc.to_text();
    dispatch(&mut state, Command::Scheduled("09:00".to_string())).unwrap();
    assert_eq!(state.status, "Not in a meeting");
    assert_eq!(state.doc.to_text(), before);
}

#[test]
fn start_in_meeting_inserts_started_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Start).unwrap();
    let text = state.doc.to_text();
    // line format is "Started: HH:MM" — just verify prefix since time varies
    assert!(
        text.contains("Started: "),
        "Started line missing: {}",
        text
    );
    // Started should appear between the heading and any notes
    let heading_pos = text.find("### Standup").unwrap();
    let started_pos = text.find("Started: ").unwrap();
    assert!(started_pos > heading_pos, "Started should be after heading");
}

#[test]
fn end_in_meeting_inserts_ended_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::End).unwrap();
    let text = state.doc.to_text();
    assert!(text.contains("Ended: "), "Ended line missing: {}", text);
}

#[test]
fn scheduled_in_meeting_inserts_scheduled_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Scheduled("09:00".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(
        text.contains("Scheduled: 09:00\n"),
        "Scheduled line missing: {}",
        text
    );
}

#[test]
fn start_twice_overwrites_started_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Start).unwrap();
    dispatch(&mut state, Command::Start).unwrap();
    let text = state.doc.to_text();
    // Only one "Started:" line should exist
    let count = text.matches("Started: ").count();
    assert_eq!(count, 1, "should have exactly one Started line: {}", text);
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p buff --lib app::actions -- start_outside
```

Expected: compile error — `Command::Start`, `Command::End`, `Command::Scheduled` not handled in `dispatch`.

- [ ] **Step 3: Add dispatch arms for the three new commands**

In `src/app/actions.rs`, add these three arms to the `match cmd` block in `dispatch()`, before the `Command::Unknown` arm.

**Important borrow-checker note:** We must copy `ord: usize` out of `state.context` before calling anything that needs `&mut state`. Pattern: `let ord = match &state.context { Context::Meeting(ord) => *ord, _ => { state.status = ...; return Ok(()); } };` — this ends the borrow on `state.context` immediately, freeing `state` for later mutable use.

```rust
Command::Start => {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => {
            state.status = "Not in a meeting".to_string();
            return Ok(());
        }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        let time = state.current_time_hhmm();
        crate::model::writer::set_meeting_time_field(
            &mut state.doc.lines,
            heading,
            "Started",
            &time,
        );
        after_doc_mutation(state)?;
    }
}
Command::End => {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => {
            state.status = "Not in a meeting".to_string();
            return Ok(());
        }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        let time = state.current_time_hhmm();
        crate::model::writer::set_meeting_time_field(
            &mut state.doc.lines,
            heading,
            "Ended",
            &time,
        );
        after_doc_mutation(state)?;
    }
}
Command::Scheduled(time) => {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => {
            state.status = "Not in a meeting".to_string();
            return Ok(());
        }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        crate::model::writer::set_meeting_time_field(
            &mut state.doc.lines,
            heading,
            "Scheduled",
            &time,
        );
        after_doc_mutation(state)?;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p buff --lib app::actions
```

Expected: all tests pass.

- [ ] **Step 5: Run full test suite**

```bash
cargo test -p buff
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat: dispatch /start, /end, /scheduled commands"
```

---

## Task 5: Add live clock to header in `layout.rs`

**Files:**
- Modify: `src/ui/layout.rs`

- [ ] **Step 1: Update the header meta line**

In `src/ui/layout.rs` at line 48, change:

```rust
// before
let meta = format!("{}\n{}", app.date.format("%Y-%m-%d (%a)"), app.context_display);

// after
let meta = format!(
    "{}  {}\n{}",
    app.date.format("%Y-%m-%d (%a)"),
    chrono::Local::now().format("%H:%M"),
    app.context_display
);
```

- [ ] **Step 2: Update the layout render test to check for time**

In the `render_empty_day` test in `src/ui/layout.rs`, add an assertion that some `HH:MM` pattern is present. Since the exact time varies, just verify the `:` separator is present in the header area by checking the buffer contains at least one occurrence of `"context: Notes"` (which already exists) and that the date line contains `(Sun)` or the appropriate day. Actually, the simplest check: assert the content contains `"2026-06-04"` (already there) — the test will still pass since the time just appends to the same line and doesn't affect the date substring. No test change required for the existing test.

Add a new test to confirm time appears:

```rust
#[test]
fn render_header_contains_time() {
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let app = test_app(doc, Focus::Capture, 0);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render(frame, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    // Time is HH:MM — look for the colon surrounded by digits
    let has_time = content
        .chars()
        .collect::<Vec<_>>()
        .windows(5)
        .any(|w| {
            w[0].is_ascii_digit()
                && w[1].is_ascii_digit()
                && w[2] == ':'
                && w[3].is_ascii_digit()
                && w[4].is_ascii_digit()
        });
    assert!(has_time, "Expected HH:MM time in header, buffer: {}", content);
}
```

- [ ] **Step 3: Run layout tests**

```bash
cargo test -p buff --lib ui::layout
```

Expected: all tests pass including the new `render_header_contains_time`.

- [ ] **Step 4: Run full test suite**

```bash
cargo test -p buff
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/ui/layout.rs
git commit -m "feat: show current time HH:MM in header"
```

---

## Task 6: Update help overlay

**Files:**
- Modify: `src/ui/help.rs`

- [ ] **Step 1: Add the three new commands to the help text**

In `src/ui/help.rs`, update the `help_text` constant. Add the three new lines under the Commands section, after `/todo text`:

```rust
let help_text = r#"Capture mode:
  type to enter notes, Enter to submit, Esc to navigate
  Tab        insert indent (->)
  Ctrl+.     prepend indent at line start

Commands:
  /meeting "Name"  start meeting context
  /note "Name"     start note context
  /note            switch to Notes context
  /todo text       add todo
  /start           record meeting start (current time)
  /end             record meeting end (current time)
  /scheduled HH:MM  record scheduled start time
  /leave           exit meeting context
  /goto YYYY-MM-DD  jump to date
  /today, Ctrl-T   jump to today
  /ask message     ask the local LLM (streams to chat)
  /clear           clear the chat conversation

Chat panel:
  Ctrl-L           show/hide chat panel
  Tab              focus chat (then j/k to scroll)

Navigation:
  [ ]        prev/next day
  j/k        move up/down
  g/G        first/last
  Space/x    toggle
  e          edit
  d d        delete
  i/Esc      capture mode
  ?          help
  Ctrl-C     quit

Right panel:
  Tab        focus right panel
  j/k or ↑/↓  navigate panel todos
  Space/x    toggle selected todo
  Esc        return to document"#;
```

- [ ] **Step 2: Update the help overlay test to check for the new commands**

In `src/ui/layout.rs`, the `render_help_overlay` test already checks for `"/meeting"` and `"/ask"`. Add checks for the new commands:

```rust
#[test]
fn render_help_overlay() {
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::Navigate, 0);
    app.overlay = Overlay::Help;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render(frame, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(
        content.contains("/meeting"),
        "Expected '/meeting' in buffer, got: {}",
        content
    );
    assert!(
        content.contains("/ask"),
        "Expected '/ask' in help buffer, got: {}",
        content
    );
    assert!(
        content.contains("/start"),
        "Expected '/start' in help buffer, got: {}",
        content
    );
    assert!(
        content.contains("/end"),
        "Expected '/end' in help buffer, got: {}",
        content
    );
    assert!(
        content.contains("/scheduled"),
        "Expected '/scheduled' in help buffer, got: {}",
        content
    );
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p buff
```

Expected: all tests pass.

- [ ] **Step 4: Build the binary to verify it compiles cleanly**

```bash
cargo build -p buff
```

Expected: compiles with no errors or warnings (warnings about unused code are acceptable if pre-existing).

- [ ] **Step 5: Commit**

```bash
git add src/ui/help.rs src/ui/layout.rs
git commit -m "docs: add /start, /end, /scheduled to help overlay"
```
