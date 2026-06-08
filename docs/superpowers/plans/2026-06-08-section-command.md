# `/section` Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `/section <name>` command that inserts a sub-heading one level deeper than the current context (H3→H4→H5→H6) and routes subsequent entries under it.

**Architecture:** New `EntryTarget::Section` and `Context::Section` variants carry `(heading_line: usize, level: u8)`. Entry insertion scans forward from `heading_line` to find where the section ends. Navigate mode (`Esc`+select+`Enter`) can resume any section heading.

**Tech Stack:** Rust, inline `#[cfg(test)]` tests, `cargo test`.

---

## File Map

| File | Change |
|---|---|
| `src/model/day.rs` | Add `EntryTarget::Section { heading_line, level }` |
| `src/model/writer.rs` | Extract `insertion_index_for_target` helper; add `EntryTarget::Section` arm to `add_block`; add `add_section_heading` method |
| `src/app/state.rs` | Add `Context::Section { heading_line, level }`; update `update_context_display` |
| `src/app/command.rs` | Add `Command::Section(String)`; add `/section` parse arm |
| `src/app/actions.rs` | Add `Context::Section` arm to `Command::Entry` routing; add `Command::Section` dispatch arm; update `resume_selected_heading` for `MarkdownHeading` |
| `src/ui/help.rs` | Add `/section` entry to help text |

---

## Task 1: Model — `EntryTarget::Section` + writer foundation

**Files:**
- Modify: `src/model/day.rs`
- Modify: `src/model/writer.rs`

### Background

`add_block` in `writer.rs` currently has three `EntryTarget` arms and inlines the "find section end" logic for `Meeting` and `NoteBlock`. We will:

1. Add `EntryTarget::Section` to the enum in `day.rs`.
2. Extract a private `insertion_index_for_target(&self, target) -> usize` helper in `writer.rs` that handles `Meeting`, `NoteBlock`, and the new `Section` variant (not `Notes` — that needs `ensure_section` which requires `&mut self`).
3. Refactor `add_block` to use the helper for those three cases.
4. Add `add_section_heading(&mut self, target, level, name) -> usize` that calls the helper, inserts the heading line, and returns its index.

- [ ] **Step 1: Write failing tests for `add_block` with `EntryTarget::Section`**

Add these tests to the `#[cfg(test)] mod tests` block at the bottom of `src/model/writer.rs`:

```rust
#[test]
fn add_entry_to_section_stays_within_it() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n\n#### Updates\n\n## Notes\n\n## To-dos\n",
    );
    let heading_line = doc.lines.iter().position(|l| l == "#### Updates").unwrap();
    doc.add_block(
        &EntryTarget::Section { heading_line, level: 4 },
        &["- note".to_string()],
    );
    let text = doc.to_text();
    let section_pos = text.find("#### Updates").unwrap();
    let entry_pos = text.find("- note").unwrap();
    let notes_pos = text.find("## Notes").unwrap();
    assert!(entry_pos > section_pos, "entry should be after section heading: {}", text);
    assert!(entry_pos < notes_pos, "entry should be before ## Notes: {}", text);
}

#[test]
fn add_entry_to_section_stops_at_peer_heading() {
    // A #### section ends before the next ####
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n\n#### Alice\n\n#### Bob\n\n## Notes\n\n## To-dos\n",
    );
    let alice_line = doc.lines.iter().position(|l| l == "#### Alice").unwrap();
    doc.add_block(
        &EntryTarget::Section { heading_line: alice_line, level: 4 },
        &["- alice note".to_string()],
    );
    let text = doc.to_text();
    let alice_pos = text.find("#### Alice").unwrap();
    let entry_pos = text.find("- alice note").unwrap();
    let bob_pos = text.find("#### Bob").unwrap();
    assert!(entry_pos > alice_pos, "entry should be after Alice: {}", text);
    assert!(entry_pos < bob_pos, "entry should be before Bob: {}", text);
}

#[test]
fn add_section_heading_in_meeting_creates_h4_returns_line() {
    let mut doc = Document::new_for_date(
        chrono::NaiveDate::from_ymd_opt(2026, 6, 8).unwrap(),
    );
    let ord = doc.add_meeting("Standup");
    let heading_line = doc.add_section_heading(&EntryTarget::Meeting(ord), 4, "Updates");
    let text = doc.to_text();
    assert!(text.contains("#### Updates\n"), "got: {}", text);
    let standup_pos = text.find("### Standup").unwrap();
    let section_pos = text.find("#### Updates").unwrap();
    assert!(section_pos > standup_pos, "section should be after meeting heading");
    assert_eq!(doc.lines[heading_line], "#### Updates", "heading_line should point to the heading");
}

#[test]
fn add_section_heading_nested_in_section_creates_h5() {
    let mut doc = Document::new_for_date(
        chrono::NaiveDate::from_ymd_opt(2026, 6, 8).unwrap(),
    );
    let ord = doc.add_meeting("Standup");
    let h4_line = doc.add_section_heading(&EntryTarget::Meeting(ord), 4, "Updates");
    let h5_line = doc.add_section_heading(
        &EntryTarget::Section { heading_line: h4_line, level: 4 },
        5,
        "Details",
    );
    let text = doc.to_text();
    assert!(text.contains("##### Details\n"), "got: {}", text);
    assert_eq!(doc.lines[h5_line], "##### Details");
    let updates_pos = text.find("#### Updates").unwrap();
    let details_pos = text.find("##### Details").unwrap();
    assert!(details_pos > updates_pos, "Details should be after Updates");
}

#[test]
fn add_entry_to_nested_section_stays_within_it() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n\n#### Updates\n\n##### Details\n\n## Notes\n\n## To-dos\n",
    );
    let details_line = doc.lines.iter().position(|l| l == "##### Details").unwrap();
    doc.add_block(
        &EntryTarget::Section { heading_line: details_line, level: 5 },
        &["- detail".to_string()],
    );
    let text = doc.to_text();
    let details_pos = text.find("##### Details").unwrap();
    let entry_pos = text.find("- detail").unwrap();
    let notes_pos = text.find("## Notes").unwrap();
    assert!(entry_pos > details_pos, "entry should be after Details");
    assert!(entry_pos < notes_pos, "entry should be before ## Notes");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test add_entry_to_section add_section_heading add_entry_to_nested
```

Expected: compile error — `EntryTarget::Section` does not exist, `add_section_heading` does not exist.

- [ ] **Step 3: Add `EntryTarget::Section` to `src/model/day.rs`**

In `src/model/day.rs`, change the `EntryTarget` enum:

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntryTarget {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
    Section { heading_line: usize, level: u8 },
}
```

- [ ] **Step 4: Add `heading_level` to imports in `src/model/writer.rs`**

Change the first line of `src/model/writer.rs`:

```rust
use crate::model::parser::{block_insert_index, ensure_section, heading_level, heading_line, section_end};
```

- [ ] **Step 5: Extract `insertion_index_for_target` helper and refactor `add_block` in `src/model/writer.rs`**

Replace the existing `add_block` implementation and add the new private helper. The full updated block (replace everything from `pub fn add_block` through its closing brace):

```rust
/// Returns the insertion index for `Meeting`, `NoteBlock`, and `Section` targets.
/// Panics if called with `EntryTarget::Notes` — that case requires `ensure_section` first.
fn insertion_index_for_target(&self, target: &EntryTarget) -> usize {
    match target {
        EntryTarget::Notes => panic!("Notes target requires ensure_section; use add_block"),
        EntryTarget::Meeting(ord) => {
            let meetings = self.meetings();
            let meeting = meetings.get(*ord).expect("meeting not found");
            let start = meeting.heading_line;
            let end = self
                .lines
                .iter()
                .enumerate()
                .skip(start + 1)
                .position(|(_, line)| line.starts_with("### ") || line.starts_with("## "))
                .map(|i| start + 1 + i)
                .unwrap_or(self.lines.len());
            block_insert_index(&self.lines, start, end)
        }
        EntryTarget::NoteBlock(ord) => {
            let notes = self.note_headings();
            let note = notes.get(*ord).expect("note not found");
            let start = note.heading_line;
            let end = self
                .lines
                .iter()
                .enumerate()
                .skip(start + 1)
                .position(|(_, line)| line.starts_with("### ") || line.starts_with("## "))
                .map(|i| start + 1 + i)
                .unwrap_or(self.lines.len());
            block_insert_index(&self.lines, start, end)
        }
        EntryTarget::Section { heading_line, level } => {
            let start = *heading_line;
            let end = self
                .lines
                .iter()
                .enumerate()
                .skip(start + 1)
                .position(|(_, line)| {
                    heading_level(line).map_or(false, |lv| lv <= *level as usize)
                })
                .map(|i| start + 1 + i)
                .unwrap_or(self.lines.len());
            block_insert_index(&self.lines, start, end)
        }
    }
}

pub fn add_block(&mut self, target: &EntryTarget, block: &[String]) {
    let insert_idx = match target {
        EntryTarget::Notes => {
            let start = ensure_section(&mut self.lines, SectionKind::Notes);
            let end = section_end(&self.lines, start);
            block_insert_index(&self.lines, start, end)
        }
        other => self.insertion_index_for_target(other),
    };
    for (k, line) in block.iter().enumerate() {
        self.lines.insert(insert_idx + k, line.clone());
    }
}
```

- [ ] **Step 6: Add `add_section_heading` method to `Document` in `src/model/writer.rs`**

Add this method after `add_block`:

```rust
/// Insert a heading of `level` hashes with the given `name` at the end of `target`'s
/// content, and return the line index of the newly inserted heading.
/// The returned index is stable: subsequent insertions always go *after* the heading,
/// so it never shifts.
pub fn add_section_heading(&mut self, target: &EntryTarget, level: u8, name: &str) -> usize {
    let insert_idx = self.insertion_index_for_target(target);
    let hashes = "#".repeat(level as usize);
    self.lines.insert(insert_idx, format!("{} {}", hashes, name));
    insert_idx
}
```

- [ ] **Step 7: Run tests to verify they pass**

```bash
cargo test add_entry_to_section add_section_heading add_entry_to_nested
```

Expected: all 5 new tests pass; existing tests also pass.

```bash
cargo test
```

Expected: all tests pass, no regressions.

- [ ] **Step 8: Commit**

```bash
git add src/model/day.rs src/model/writer.rs
git commit -m "feat: add EntryTarget::Section and add_section_heading to writer"
```

---

## Task 2: State — `Context::Section` + entry routing

**Files:**
- Modify: `src/app/state.rs`
- Modify: `src/app/actions.rs`

Adding `Context::Section` to the enum requires updating every exhaustive `match` on `Context`:
- `update_context_display` in `state.rs`
- The `Command::Entry` routing match in `actions.rs` (maps context → `EntryTarget`)

- [ ] **Step 1: Write a failing test for `update_context_display` with Section context**

Add to `src/app/actions.rs` tests block:

```rust
#[test]
fn section_context_display_shows_name() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    // Manually set up a doc with a meeting and a section heading, then set context
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    // Manually inject the Section context (dispatch for Section isn't written yet)
    // Find the line index of ### Standup
    let heading_line = state.doc.lines.iter().position(|l| l == "### Standup").unwrap();
    // Insert #### Updates manually
    state.doc.lines.insert(heading_line + 1, "#### Updates".to_string());
    state.context = Context::Section { heading_line: heading_line + 1, level: 4 };
    state.update_context_display();
    assert_eq!(state.context_display, "context: Updates");
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test section_context_display_shows_name
```

Expected: compile error — `Context::Section` does not exist.

- [ ] **Step 3: Add `Context::Section` to `src/app/state.rs`**

Change the `Context` enum:

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
    Section { heading_line: usize, level: u8 },
}
```

In `update_context_display`, the existing match ends with the `NoteBlock` arm before the closing `};`. Add the `Section` arm immediately before that closing `};`:

```rust
        Context::NoteBlock(ord) => {
            let notes = self.doc.note_headings();
            match notes.get(ord) {
                Some(n) => format!("context: {}", n.name),
                None => "context: Notes".to_string(),
            }
        }
        // ADD THIS ARM:
        Context::Section { heading_line, .. } => {
            let name = self.doc.lines
                .get(heading_line)
                .map(|l| l.trim_start_matches('#').trim_start())
                .unwrap_or("section");
            format!("context: {}", name)
        }
    };
}
```

Note: the existing match uses `match self.context` (fields bound by copy since `usize` and `u8` are `Copy`), so `heading_line` is a `usize` — no dereference needed.

- [ ] **Step 4: Add `Context::Section` arm to the `Command::Entry` routing in `src/app/actions.rs`**

In `dispatch`, find the `Command::Entry` arm. It contains:

```rust
let target = match &state.context {
    Context::Notes => EntryTarget::Notes,
    Context::Meeting(ord) => EntryTarget::Meeting(*ord),
    Context::NoteBlock(ord) => EntryTarget::NoteBlock(*ord),
};
```

Replace it with:

```rust
let target = match &state.context {
    Context::Notes => EntryTarget::Notes,
    Context::Meeting(ord) => EntryTarget::Meeting(*ord),
    Context::NoteBlock(ord) => EntryTarget::NoteBlock(*ord),
    Context::Section { heading_line, level } => {
        EntryTarget::Section { heading_line: *heading_line, level: *level }
    }
};
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test section_context_display_shows_name
```

Expected: passes.

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/state.rs src/app/actions.rs
git commit -m "feat: add Context::Section and wire Entry routing for sections"
```

---

## Task 3: Command — parse + dispatch

**Files:**
- Modify: `src/app/command.rs`
- Modify: `src/app/actions.rs`

Adding `Command::Section` to the `Command` enum will cause a compile error in `dispatch` (exhaustive match) until the dispatch arm is added. Both changes land in the same task.

- [ ] **Step 1: Write failing tests for command parsing**

Add to the `#[cfg(test)] mod tests` block in `src/app/command.rs`:

```rust
#[test]
fn parse_section_unquoted() {
    assert_eq!(
        parse("/section Tanner's Update"),
        Command::Section("Tanner's Update".to_string())
    );
}

#[test]
fn parse_section_quoted() {
    assert_eq!(
        parse("/section \"Tanner's Update\""),
        Command::Section("Tanner's Update".to_string())
    );
}

#[test]
fn parse_section_empty() {
    assert_eq!(
        parse("/section"),
        Command::InvalidArgs("/section needs a name".to_string())
    );
}
```

- [ ] **Step 2: Write failing integration tests for dispatch**

Add to `src/app/actions.rs` tests block:

```rust
#[test]
fn section_in_meeting_creates_h4_and_sets_context() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Section("Updates".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(text.contains("#### Updates\n"), "got: {}", text);
    let standup_pos = text.find("### Standup").unwrap();
    let section_pos = text.find("#### Updates").unwrap();
    assert!(section_pos > standup_pos, "section should be after meeting heading");
    assert!(matches!(state.context, Context::Section { level: 4, .. }));
    assert_eq!(state.context_display, "context: Updates");
}

#[test]
fn entry_after_section_routes_under_section() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Section("Updates".to_string())).unwrap();
    dispatch(&mut state, Command::Entry("my point".to_string())).unwrap();
    let text = state.doc.to_text();
    let section_pos = text.find("#### Updates").unwrap();
    let entry_pos = text.find("my point").unwrap();
    assert!(entry_pos > section_pos, "entry should be under section: {}", text);
}

#[test]
fn section_nested_in_section_creates_h5() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Section("Updates".to_string())).unwrap();
    dispatch(&mut state, Command::Section("Details".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(text.contains("##### Details\n"), "got: {}", text);
    assert!(matches!(state.context, Context::Section { level: 5, .. }));
}

#[test]
fn section_in_noteblock_creates_h4() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Note(Some("Ideas".to_string()))).unwrap();
    dispatch(&mut state, Command::Section("Sub-topic".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(text.contains("#### Sub-topic\n"), "got: {}", text);
    assert!(matches!(state.context, Context::Section { level: 4, .. }));
}

#[test]
fn section_outside_meeting_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    let before = state.doc.to_text();
    dispatch(&mut state, Command::Section("Foo".to_string())).unwrap();
    assert_eq!(state.status, "Not in a meeting or note");
    assert_eq!(state.doc.to_text(), before);
}

#[test]
fn section_at_max_depth_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    // H3 → H4 → H5 → H6 (three /section calls to reach H6)
    dispatch(&mut state, Command::Section("A".to_string())).unwrap(); // H4
    dispatch(&mut state, Command::Section("B".to_string())).unwrap(); // H5
    dispatch(&mut state, Command::Section("C".to_string())).unwrap(); // H6
    let before = state.doc.to_text();
    dispatch(&mut state, Command::Section("D".to_string())).unwrap(); // should fail
    assert_eq!(state.status, "/section: already at maximum depth (######)");
    assert_eq!(state.doc.to_text(), before, "doc should not change");
}

#[test]
fn section_saves_to_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Section("Updates".to_string())).unwrap();
    let path = tmp.path().join("2026-06-04-Thu.md");
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("#### Updates\n"), "saved: {}", saved);
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test parse_section section_in_meeting entry_after_section section_nested section_in_noteblock section_outside section_at_max section_saves
```

Expected: compile error — `Command::Section` does not exist.

- [ ] **Step 4: Add `Command::Section` to `src/app/command.rs`**

In `src/app/command.rs`, add to the `Command` enum:

```rust
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
    Section(String),   // NEW
    Unknown(String),
    InvalidArgs(String),
}
```

Add the `/section` parse arm in `parse()`, before the `_ =>` catch-all:

```rust
"/section" => {
    let name = rest.trim_matches('"').trim();
    if name.is_empty() {
        Command::InvalidArgs("/section needs a name".to_string())
    } else {
        Command::Section(name.to_string())
    }
}
```

- [ ] **Step 5: Add `Command::Section` dispatch arm to `src/app/actions.rs`**

In `dispatch`, add this arm before `Command::Unknown`:

```rust
Command::Section(name) => {
    let current_level: u8 = match &state.context {
        Context::Meeting(_) | Context::NoteBlock(_) => 3,
        Context::Section { level, .. } => *level,
        Context::Notes => {
            state.status = "Not in a meeting or note".to_string();
            return Ok(());
        }
    };
    if current_level >= 6 {
        state.status = "/section: already at maximum depth (######)".to_string();
        return Ok(());
    }
    let target = match &state.context {
        Context::Meeting(ord) => EntryTarget::Meeting(*ord),
        Context::NoteBlock(ord) => EntryTarget::NoteBlock(*ord),
        Context::Section { heading_line, level } => {
            EntryTarget::Section { heading_line: *heading_line, level: *level }
        }
        Context::Notes => unreachable!(),
    };
    let next_level = current_level + 1;
    let heading_line = state.doc.add_section_heading(&target, next_level, &name);
    state.context = Context::Section { heading_line, level: next_level };
    state.update_context_display();
    after_doc_mutation(state)?;
}
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test parse_section section_in_meeting entry_after_section section_nested section_in_noteblock section_outside section_at_max section_saves
```

Expected: all 10 new tests pass.

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/app/command.rs src/app/actions.rs
git commit -m "feat: add /section command — creates sub-headings and updates context"
```

---

## Task 4: Navigate — `resume_selected_heading` for `MarkdownHeading`

**Files:**
- Modify: `src/app/actions.rs`

Pressing `Enter` on a `MarkdownHeading` selectable (H4-H6 heading created by `/section`) in Navigate mode should set `Context::Section` so subsequent entries route to that section. Currently it shows "not a meeting or note".

- [ ] **Step 1: Write failing test**

Add to `src/app/actions.rs` tests block:

```rust
#[test]
fn resume_markdown_heading_sets_section_context() {
    use crate::model::day::SelectableKind;
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Section("Updates".to_string())).unwrap();
    // Leave the section context
    dispatch(&mut state, Command::Note(None)).unwrap();
    assert_eq!(state.context, Context::Notes);

    // Navigate to the #### Updates heading
    let idx = state
        .selectables
        .iter()
        .position(|s| matches!(s.kind, SelectableKind::MarkdownHeading))
        .expect("#### Updates should be a MarkdownHeading selectable");
    state.selected = idx;
    state.focus = crate::app::state::Focus::Navigate;

    resume_selected_heading(&mut state);

    assert!(
        matches!(state.context, Context::Section { level: 4, .. }),
        "expected Section level 4, got {:?}",
        state.context
    );
    assert_eq!(state.focus, crate::app::state::Focus::Capture);
    assert_eq!(state.context_display, "context: Updates");
}

#[test]
fn resume_markdown_heading_entry_routes_under_it() {
    use crate::model::day::SelectableKind;
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Section("Updates".to_string())).unwrap();
    dispatch(&mut state, Command::Note(None)).unwrap(); // leave context

    let idx = state
        .selectables
        .iter()
        .position(|s| matches!(s.kind, SelectableKind::MarkdownHeading))
        .expect("section heading should be selectable");
    state.selected = idx;
    state.focus = crate::app::state::Focus::Navigate;
    resume_selected_heading(&mut state);

    dispatch(&mut state, Command::Entry("after resume".to_string())).unwrap();
    let text = state.doc.to_text();
    let section_pos = text.find("#### Updates").unwrap();
    let entry_pos = text.find("after resume").unwrap();
    assert!(entry_pos > section_pos, "entry should be under section after resume: {}", text);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test resume_markdown_heading
```

Expected: FAIL — status is "not a meeting or note" instead of setting Section context.

- [ ] **Step 3: Update `resume_selected_heading` in `src/app/actions.rs`**

Find `resume_selected_heading`. Currently it matches `MeetingHeading` and `NoteHeading` and falls through otherwise. Add a `MarkdownHeading` branch and update the fallback message:

```rust
pub fn resume_selected_heading(state: &mut AppState) {
    if let Some(sel) = state.selectables.get(state.selected) {
        match sel.kind {
            SelectableKind::MeetingHeading { ordinal } => {
                state.context = Context::Meeting(ordinal);
                state.update_context_display();
                state.focus = crate::app::state::Focus::Capture;
                state.status.clear();
                return;
            }
            SelectableKind::NoteHeading { ordinal } => {
                state.context = Context::NoteBlock(ordinal);
                state.update_context_display();
                state.focus = crate::app::state::Focus::Capture;
                state.status.clear();
                return;
            }
            SelectableKind::MarkdownHeading => {
                let level = crate::model::parser::heading_level(&sel.text)
                    .unwrap_or(4) as u8;
                let heading_line = sel.lines.start;
                state.context = Context::Section { heading_line, level };
                state.update_context_display();
                state.focus = crate::app::state::Focus::Capture;
                state.status.clear();
                return;
            }
            _ => {}
        }
    }
    state.status = "not a meeting, note, or section".to_string();
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test resume_markdown_heading
```

Expected: both new tests pass.

```bash
cargo test
```

Expected: all tests pass. Note: the existing test `resume_on_non_meeting_sets_status` in `actions.rs` checks for the message `"not a meeting or note"` — update that test's assertion to match the new message `"not a meeting, note, or section"`.

If that test fails, find it and update its assertion:

```rust
assert_eq!(state.status, "not a meeting, note, or section");
```

- [ ] **Step 5: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat: resume_selected_heading handles MarkdownHeading as Section context"
```

---

## Task 5: Help text

**Files:**
- Modify: `src/ui/help.rs`

- [ ] **Step 1: Add `/section` to the help text**

In `src/ui/help.rs`, find the Commands section in the `help_text` string. After the `/note` lines, add:

```
  /section "Name"  add sub-section (one heading deeper, max ######)
```

The updated Commands block should read:

```
Commands:
  /meeting "Name"  start meeting context
  /note "Name"     start note context
  /note            switch to Notes context
  /section "Name"  add sub-section (one heading deeper, max ######)
  /todo text       add todo
  /start           record meeting start (current time)
  ...
```

- [ ] **Step 2: Build and verify no regressions**

```bash
cargo build && cargo test
```

Expected: builds cleanly, all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/ui/help.rs
git commit -m "docs: add /section to help text"
```

---

## Self-Review Checklist

After completing all tasks, verify:

- [ ] `cargo test` passes with zero failures
- [ ] `/section Foo` in a meeting adds `#### Foo` under the meeting and routes entries to it
- [ ] `/section Bar` inside that section adds `##### Bar`
- [ ] Three more `/section` calls from a meeting hits the H6 cap and shows the error
- [ ] `/section` from the Notes context shows the "Not in a meeting or note" error
- [ ] Esc → navigate to `#### Foo` → Enter sets Section context and routes entries under it
- [ ] `/help` overlay shows the `/section` line
