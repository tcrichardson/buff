# Named Notes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `/note "Note Name"` to create named note sub-sections under `## Notes`, symmetric to how `/meeting "Name"` works in `## Meetings`.

**Architecture:** Extend the existing command parser, data model (`Command`, `Context`, `EntryTarget`, `SelectableKind`), document writer (`note_headings`, `add_note_heading`, `add_block`), and action dispatch to treat `### ` headings in the Notes section as `NoteHeading` selectables with resumable context.

**Tech Stack:** Rust (edition 2024), Ratatui 0.30 + crossterm, chrono, anyhow, tempfile.

---

## File Structure

- `src/app/command.rs` — parse `/note "Name"` into `Note(Some(name))`; `/note` stays `Note(None)`
- `src/model/day.rs` — add `NoteBlock(usize)` to `EntryTarget`; add `NoteHeading { ordinal }` to `SelectableKind`
- `src/model/writer.rs` — add `note_headings()`, `add_note_heading()`; update `add_block` for `NoteBlock`; update `selectables()` to classify `### ` in Notes as `NoteHeading`
- `src/app/state.rs` — add `NoteBlock(usize)` to `Context`; update `update_context_display`
- `src/app/actions.rs` — handle `Note(Some(name))` in dispatch; update `resume_selected_meeting` for `NoteHeading`
- `src/ui/document.rs` — render `NoteHeading` with yellow bold (same as `MeetingHeading`)
- `src/ui/help.rs` — update help text for `/note`

---

## Task 1: Command parsing

**Files:**
- Modify: `src/app/command.rs`
- Test: `src/app/command.rs` (`mod tests`)

- [ ] **Step 1: Update `Command` enum and parse function**

Change `Command::Note` to carry an `Option<String>`:

```rust
pub enum Command {
    Entry(String),
    Meeting(String),
    Note(Option<String>),
    // ... rest unchanged
}
```

In `parse`, change the `/note` arm from:
```rust
"/note" => Command::Note,
```
to:
```rust
"/note" => {
    let name = rest.trim_matches('"').trim();
    if name.is_empty() {
        Command::Note(None)
    } else {
        Command::Note(Some(name.to_string()))
    }
}
```

- [ ] **Step 2: Update existing parse test**

Change:
```rust
#[test]
fn parse_note() {
    assert_eq!(parse("/note"), Command::Note);
}
```
to:
```rust
#[test]
fn parse_note() {
    assert_eq!(parse("/note"), Command::Note(None));
}
```

- [ ] **Step 3: Add new parse tests**

Add inside `mod tests`:
```rust
#[test]
fn parse_note_quoted() {
    assert_eq!(
        parse("/note \"Idea Bucket\""),
        Command::Note(Some("Idea Bucket".to_string()))
    );
}

#[test]
fn parse_note_unquoted() {
    assert_eq!(
        parse("/note Idea Bucket"),
        Command::Note(Some("Idea Bucket".to_string()))
    );
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib command::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app/command.rs
git commit -m "feat: parse /note with optional name"
```

---

## Task 2: Data model extensions

**Files:**
- Modify: `src/model/day.rs`
- Modify: `src/app/state.rs`

- [ ] **Step 1: Extend `EntryTarget` in `src/model/day.rs`**

Change:
```rust
pub enum EntryTarget {
    Notes,
    Meeting(usize),
}
```
to:
```rust
pub enum EntryTarget {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
}
```

- [ ] **Step 2: Extend `SelectableKind` in `src/model/day.rs`**

Change:
```rust
pub enum SelectableKind {
    Bullet,
    Todo { done: bool },
    MeetingHeading { ordinal: usize },
    MarkdownHeading,
    Quote,
    Numbered,
    CodeBlock,
    Raw,
}
```
to:
```rust
pub enum SelectableKind {
    Bullet,
    Todo { done: bool },
    MeetingHeading { ordinal: usize },
    NoteHeading { ordinal: usize },
    MarkdownHeading,
    Quote,
    Numbered,
    CodeBlock,
    Raw,
}
```

- [ ] **Step 3: Extend `Context` in `src/app/state.rs`**

Change:
```rust
pub enum Context {
    Notes,
    Meeting(usize),
}
```
to:
```rust
pub enum Context {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
}
```

- [ ] **Step 4: Update `update_context_display` in `src/app/state.rs`**

Change:
```rust
pub fn update_context_display(&mut self) {
    self.context_display = match self.context {
        Context::Notes => "context: Notes".to_string(),
        Context::Meeting(ord) => {
            let meetings = self.doc.meetings();
            match meetings.get(ord) {
                Some(m) => format!("context: {}", m.name),
                None => "context: Notes".to_string(),
            }
        }
    };
}
```
to:
```rust
pub fn update_context_display(&mut self) {
    self.context_display = match self.context {
        Context::Notes => "context: Notes".to_string(),
        Context::Meeting(ord) => {
            let meetings = self.doc.meetings();
            match meetings.get(ord) {
                Some(m) => format!("context: {}", m.name),
                None => "context: Notes".to_string(),
            }
        }
        Context::NoteBlock(ord) => {
            let notes = self.doc.note_headings();
            match notes.get(ord) {
                Some(n) => format!("context: {}", n.name),
                None => "context: Notes".to_string(),
            }
        }
    };
}
```

- [ ] **Step 5: Build to check compilation**

Run: `cargo check`
Expected: compile errors in writer/actions where `EntryTarget`/`Context` are matched — expected, fixed in next tasks.

- [ ] **Step 6: Commit**

```bash
git add src/model/day.rs src/app/state.rs
git commit -m "feat: add NoteBlock context and NoteHeading selectable kind"
```

---

## Task 3: Document writer — note headings and add_block

**Files:**
- Modify: `src/model/writer.rs`
- Test: `src/model/writer.rs` (`mod tests`)

- [ ] **Step 1: Add `note_headings()` and `add_note_heading()`**

Add after the `meetings()` method:

```rust
pub fn note_headings(&self) -> Vec<Meeting> {
    let start = match heading_line(&self.lines, SectionKind::Notes) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let end = section_end(&self.lines, start);
    let mut notes = Vec::new();
    for i in start + 1..end {
        let line = &self.lines[i];
        if let Some(rest) = line.strip_prefix("### ") {
            notes.push(Meeting {
                ordinal: notes.len(),
                heading_line: i,
                time: String::new(),
                name: rest.to_string(),
            });
        }
    }
    notes
}

pub fn add_note_heading(&mut self, name: &str) -> usize {
    let start = ensure_section(&mut self.lines, SectionKind::Notes);
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

- [ ] **Step 2: Update `add_block` for `NoteBlock`**

Change the `add_block` method to handle `EntryTarget::NoteBlock`:

```rust
pub fn add_block(&mut self, target: &EntryTarget, block: &[String]) {
    let insert_idx = match target {
        EntryTarget::Notes => {
            let start = ensure_section(&mut self.lines, SectionKind::Notes);
            let end = section_end(&self.lines, start);
            block_insert_index(&self.lines, start, end)
        }
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
    };
    for (k, line) in block.iter().enumerate() {
        self.lines.insert(insert_idx + k, line.clone());
    }
}
```

- [ ] **Step 3: Update `selectables()` to classify `### ` in Notes as `NoteHeading`**

In the `selectables()` method, after the existing `### ` / `in_meetings` block, add an `else if` for Notes section:

Replace the meeting-heading block (lines ~414–426 in the current file):
```rust
        // Meeting heading inside the Meetings section.
        if line.starts_with("### ") {
            let in_meetings = matches!((meetings_start, meetings_end), (Some(s), Some(e)) if i > s && i < e);
            if in_meetings {
                result.push(Selectable {
                    lines: i..i + 1,
                    kind: SelectableKind::MeetingHeading { ordinal: meeting_ord },
                    text: line.clone(),
                });
                meeting_ord += 1;
                i += 1;
                continue;
            }
        }
```

With:
```rust
        // Meeting heading inside the Meetings section.
        if line.starts_with("### ") {
            let in_meetings = matches!((meetings_start, meetings_end), (Some(s), Some(e)) if i > s && i < e);
            if in_meetings {
                result.push(Selectable {
                    lines: i..i + 1,
                    kind: SelectableKind::MeetingHeading { ordinal: meeting_ord },
                    text: line.clone(),
                });
                meeting_ord += 1;
                i += 1;
                continue;
            }
            let in_notes = matches!((notes_start, notes_end), (Some(s), Some(e)) if i > s && i < e);
            if in_notes {
                result.push(Selectable {
                    lines: i..i + 1,
                    kind: SelectableKind::NoteHeading { ordinal: note_ord },
                    text: line.clone(),
                });
                note_ord += 1;
                i += 1;
                continue;
            }
        }
```

Also add the `notes_start`/`notes_end` bindings at the top of the method, next to `meetings_start`/`meetings_end`:

```rust
    let notes_start = heading_line(lines, SectionKind::Notes);
    let notes_end = notes_start.map(|s| section_end(lines, s));
```

And add `let mut note_ord = 0usize;` next to `let mut meeting_ord = 0usize;`.

- [ ] **Step 4: Add model tests**

Add inside `mod tests` in `src/model/writer.rs`:

```rust
#[test]
fn add_note_heading_to_empty_doc() {
    let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let ord = doc.add_note_heading("Idea Bucket");
    assert_eq!(ord, 0);
    let notes = doc.note_headings();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].ordinal, 0);
    assert_eq!(notes[0].name, "Idea Bucket");
}

#[test]
fn add_two_note_headings() {
    let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let ord0 = doc.add_note_heading("First");
    let ord1 = doc.add_note_heading("Second");
    assert_eq!(ord0, 0);
    assert_eq!(ord1, 1);
    let notes = doc.note_headings();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].name, "First");
    assert_eq!(notes[1].name, "Second");
}

#[test]
fn add_entry_to_note_block() {
    let mut doc = Document::from_text("# 2026-06-04\n\## Meetings\n\n## Notes\n\n### Idea Bucket\n\n## To-dos\n");
    doc.add_entry(&EntryTarget::NoteBlock(0), "point", None);
    let text = doc.to_text();
    let heading_pos = text.find("### Idea Bucket").unwrap();
    let entry_pos = text.find("- point").unwrap();
    assert!(entry_pos > heading_pos, "entry should be after note heading");
}

#[test]
fn note_heading_is_selectable() {
    let doc = Document::from_text("# Day\n\n## Notes\n\n### Idea Bucket\n\n## To-dos\n");
    let sel = doc.selectables();
    assert_eq!(sel.len(), 1);
    assert_eq!(sel[0].kind, SelectableKind::NoteHeading { ordinal: 0 });
    assert_eq!(sel[0].text, "### Idea Bucket");
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: PASS (may have compile errors in actions.rs until Task 4).

- [ ] **Step 6: Commit**

```bash
git add src/model/writer.rs
git commit -m "feat: note headings, add_block for NoteBlock, selectable classification"
```

---

## Task 4: Action dispatch and resume

**Files:**
- Modify: `src/app/actions.rs`
- Test: `src/app/actions.rs` (`mod tests`)

- [ ] **Step 1: Update `dispatch` Entry handler for `NoteBlock` context**

In the `Command::Entry(text)` arm, change the `target` match from:
```rust
            let target = match &state.context {
                Context::Notes => EntryTarget::Notes,
                Context::Meeting(ord) => EntryTarget::Meeting(*ord),
            };
```
to:
```rust
            let target = match &state.context {
                Context::Notes => EntryTarget::Notes,
                Context::Meeting(ord) => EntryTarget::Meeting(*ord),
                Context::NoteBlock(ord) => EntryTarget::NoteBlock(*ord),
            };
```

- [ ] **Step 2: Update `dispatch` Note handler**

Change the `Command::Note` arm from:
```rust
        Command::Note => {
            state.context = Context::Notes;
            state.update_context_display();
            state.status.clear();
        }
```
to:
```rust
        Command::Note(name) => {
            if let Some(n) = name {
                let ord = state.doc.add_note_heading(&n);
                state.context = Context::NoteBlock(ord);
                state.selectables = state.doc.selectables();
                state.update_context_display();
                state.save()?;
                state.dates_with_notes =
                    crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
                state.status.clear();
            } else {
                state.context = Context::Notes;
                state.update_context_display();
                state.status.clear();
            }
        }
```

- [ ] **Step 3: Update `resume_selected_meeting` to also handle `NoteHeading`**

Rename `resume_selected_meeting` to `resume_selected_heading` and update:

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
            _ => {}
        }
    }
    state.status = "not a meeting or note".to_string();
}
```

Update the import at the top of `src/app/actions.rs` to ensure `SelectableKind` is in scope (it should already be from earlier plan steps, but verify).

- [ ] **Step 4: Update all call sites of `resume_selected_meeting`**

In `src/main.rs`, change:
```rust
buff::app::actions::resume_selected_meeting(&mut app);
```
to:
```rust
buff::app::actions::resume_selected_heading(&mut app);
```

- [ ] **Step 5: Update `dispatch` Todo handler for `NoteBlock` context**

In the `Command::Todo(text)` arm, change the `meeting_name` match from:
```rust
            let meeting_name = match &state.context {
                Context::Meeting(ord) => state.doc.meetings().get(*ord).map(|m| m.name.clone()),
                _ => None,
            };
```
to:
```rust
            let meeting_name = match &state.context {
                Context::Meeting(ord) => state.doc.meetings().get(*ord).map(|m| m.name.clone()),
                Context::NoteBlock(ord) => state.doc.note_headings().get(*ord).map(|n| n.name.clone()),
                _ => None,
            };
```

- [ ] **Step 6: Add action tests**

Add inside `mod tests` in `src/app/actions.rs`:

```rust
#[test]
fn note_then_entry_nests_bullet() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Note(Some("Idea Bucket".to_string()))).unwrap();
    dispatch(&mut state, Command::Entry("point".to_string())).unwrap();
    let text = state.doc.to_text();
    let heading_pos = text.find("### Idea Bucket").unwrap();
    let entry_pos = text.find("- point").unwrap();
    assert!(entry_pos > heading_pos, "entry should be after note heading");
    assert_eq!(state.context, Context::NoteBlock(0));
}

#[test]
fn note_without_name_resets_context() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Note(Some("Idea Bucket".to_string()))).unwrap();
    dispatch(&mut state, Command::Note(None)).unwrap();
    assert_eq!(state.context, Context::Notes);
}

#[test]
fn resume_note_sets_context_and_focus() {
    use crate::model::day::SelectableKind;
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Note(Some("Idea Bucket".to_string()))).unwrap();
    dispatch(&mut state, Command::Note(None)).unwrap(); // leave the note context
    assert_eq!(state.context, Context::Notes);

    let idx = state
        .selectables
        .iter()
        .position(|s| matches!(s.kind, SelectableKind::NoteHeading { .. }))
        .expect("note heading should be selectable");
    state.selected = idx;
    state.focus = crate::app::state::Focus::Navigate;

    resume_selected_heading(&mut state);
    assert_eq!(state.context, Context::NoteBlock(0));
    assert_eq!(state.focus, crate::app::state::Focus::Capture);

    dispatch(&mut state, Command::Entry("under note".to_string())).unwrap();
    let text = state.doc.to_text();
    let heading = text.find("### Idea Bucket").unwrap();
    let entry = text.find("- under note").unwrap();
    assert!(entry > heading, "entry should be under the note heading");
}

#[test]
fn todo_in_note_gets_tag() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Note(Some("Idea Bucket".to_string()))).unwrap();
    dispatch(&mut state, Command::Todo("follow up".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(
        text.contains("- [ ] follow up _(Idea Bucket)_"),
        "got: {}",
        text
    );
    assert_eq!(state.context, Context::NoteBlock(0));
}
```

- [ ] **Step 7: Update existing tests that reference `Command::Note`**

In `src/app/actions.rs` `mod tests`, fix:
- `note_resets_context`: change `Command::Note` to `Command::Note(None)`
- `status_cleared_after_successful_note` (if any): change similarly

- [ ] **Step 8: Run tests**

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/app/actions.rs src/main.rs
git commit -m "feat: dispatch /note name, resume note headings, todo tags"
```

---

## Task 5: UI rendering and help

**Files:**
- Modify: `src/ui/document.rs`
- Modify: `src/ui/help.rs`

- [ ] **Step 1: Render `NoteHeading` in `src/ui/document.rs`**

In the `.map(|(i, line)| { ... })` closure, add a branch for `NoteHeading`. Since `NoteHeading` and `MeetingHeading` both render as `### ` lines, we can just handle them together. Find the existing `### ` branch:

```rust
            } else if let Some(rest) = line.strip_prefix("### ") {
                Line::from(vec![Span::styled(
                    format!("### {}", rest),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
```

This already handles any `### ` line, so `NoteHeading` will render identically to `MeetingHeading`. No change needed unless you want distinct colors. **No code change required** — the existing `### ` branch covers it.

- [ ] **Step 2: Update help text in `src/ui/help.rs`**

Change:
```
  /note            switch to Notes context
```
to:
```
  /note "Name"     start note context
  /note            switch to Notes context
```

- [ ] **Step 3: Build and run tests**

Run: `cargo test --lib`
Expected: PASS.

Run: `cargo build`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/ui/help.rs
git commit -m "feat: update help text for /note name"
```

---

## Task 6: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt`

- [ ] **Step 2: Lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Full test suite**

Run: `cargo test`
Expected: all tests PASS.

- [ ] **Step 4: Commit any formatting fixes**

```bash
git add -A && git commit -m "style: cargo fmt" || true
```

---

## Self-Review Checklist

1. **Spec coverage:**
   - `/note "Name"` parsing → Task 1
   - Named note creation under `## Notes` → Task 3
   - Context switching to `NoteBlock` → Task 2, Task 4
   - Entry nesting under note heading → Task 3, Task 4
   - `NoteHeading` selectable + re-entry via Enter → Task 3, Task 4
   - Todo tagging with note name → Task 4
   - Help text update → Task 5
   - All covered.

2. **Placeholder scan:** No TBD/TODO/fill-in found.

3. **Type consistency:** `NoteHeading { ordinal: usize }`, `NoteBlock(usize)`, `EntryTarget::NoteBlock(usize)`, `Context::NoteBlock(usize)` used consistently.
