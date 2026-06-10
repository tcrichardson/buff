# Metadata Slash Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `/purpose` and `/topic` slash commands that write `meta:`-prefixed metadata lines onto meetings and note blocks, and render them styled (without the prefix) in the document display.

**Architecture:** Generalize the existing `set_meeting_time_field()` / `is_time_field_line()` writer functions into `set_metadata_field()` / `is_metadata_line()` that use a `meta:` prefix for all metadata lines. Update command parsing, dispatch handlers, theme, and display to match.

**Tech Stack:** Rust, ratatui for TUI display, existing codebase patterns (no new dependencies).

---

## File Map

| File | Change |
|------|--------|
| `src/model/writer.rs` | Replace `TIME_FIELD_ORDER` / `is_time_field_line` / `set_meeting_time_field` with `METADATA_FIELD_ORDER` / `is_metadata_line` / `set_metadata_field`; update `meetings_with_scheduled` to read both formats |
| `src/app/command.rs` | Add `Purpose(String)` and `Topic(String)` variants; add parsing |
| `src/app/actions.rs` | Add `handle_purpose`, `handle_topic`; update `handle_start`, `handle_end`, `handle_scheduled` to use `set_metadata_field` |
| `src/ui/theme.rs` | Add `metadata: Color` field to `Theme`; update `light()`, `dark()`, `resolve_theme()` |
| `src/config.rs` | Add `metadata: Option<String>` to `ThemeOverrides` |
| `src/ui/document.rs` | Add `LineKind::MetaField`; classify and render `meta:` lines |

---

## Task 1: Generalize the metadata write/read layer in `writer.rs`

**Files:**
- Modify: `src/model/writer.rs:491-543` (replace old constants + functions)
- Test: `src/model/writer.rs` (in the existing `#[cfg(test)]` block)

### Overview
Replace the three hard-coded functions for time fields with a general-purpose metadata system. The metadata block for any heading is any run of consecutive lines starting with `meta:`. During migration, lines matching legacy bare `Scheduled: ` / `Started: ` / `Ended: ` formats are also treated as metadata (read and rewritten as `meta:…`).

- [ ] **Step 1.1: Write the failing tests**

Add these tests inside the `mod tests` block at the bottom of `src/model/writer.rs` (replace or supplement the `set_time_field_*` tests):

```rust
#[test]
fn is_metadata_line_recognizes_meta_prefix() {
    assert!(is_metadata_line("meta:Scheduled: 09:00"));
    assert!(is_metadata_line("meta:Purpose: kick off Q3"));
    assert!(!is_metadata_line("Scheduled: 09:00"));
    assert!(!is_metadata_line("- bullet"));
    assert!(!is_metadata_line(""));
}

#[test]
fn set_metadata_field_inserts_purpose_when_absent() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Purpose", "align team");
    let text = doc.to_text();
    assert!(text.contains("meta:Purpose: align team\n"), "got: {}", text);
    let purpose_pos = text.find("meta:Purpose:").unwrap();
    let note_pos = text.find("- note").unwrap();
    assert!(purpose_pos < note_pos, "Purpose should precede body");
}

#[test]
fn set_metadata_field_inserts_topic_in_note_block() {
    let mut doc = Document::from_text(
        "# Day\n\n## Notes\n\n### Design\n- note\n\n## To-dos\n",
    );
    let heading = doc.note_headings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Topic", "API v2");
    let text = doc.to_text();
    assert!(text.contains("meta:Topic: API v2\n"), "got: {}", text);
}

#[test]
fn set_metadata_field_overwrites_existing() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\nmeta:Purpose: old\n- note\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Purpose", "new");
    let text = doc.to_text();
    assert!(text.contains("meta:Purpose: new\n"), "got: {}", text);
    assert!(!text.contains("meta:Purpose: old\n"), "old should be gone: {}", text);
}

#[test]
fn set_metadata_field_migrates_legacy_time_fields() {
    // Old-format file has bare "Scheduled: HH:MM" — set_metadata_field should rewrite it
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\nScheduled: 09:00\n- note\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Started", "09:05");
    let text = doc.to_text();
    // Legacy Scheduled line should be rewritten to meta: prefix
    assert!(text.contains("meta:Scheduled: 09:00\n"), "legacy migrated: {}", text);
    assert!(text.contains("meta:Started: 09:05\n"), "new field written: {}", text);
    assert!(!text.contains("Scheduled: 09:00\n") || text.contains("meta:Scheduled:"), "legacy gone: {}", text);
}

#[test]
fn set_metadata_field_canonical_order() {
    // Purpose, Topic, Scheduled, Started, Ended — regardless of insertion order
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Ended", "10:00");
    let heading = doc.meetings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Started", "09:05");
    let heading = doc.meetings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Scheduled", "09:00");
    let heading = doc.meetings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Purpose", "sync");
    let text = doc.to_text();
    let purpose_pos = text.find("meta:Purpose:").unwrap();
    let sched_pos = text.find("meta:Scheduled:").unwrap();
    let started_pos = text.find("meta:Started:").unwrap();
    let ended_pos = text.find("meta:Ended:").unwrap();
    assert!(purpose_pos < sched_pos, "Purpose before Scheduled");
    assert!(sched_pos < started_pos, "Scheduled before Started");
    assert!(started_pos < ended_pos, "Started before Ended");
}

#[test]
fn set_metadata_field_does_not_eat_note_content() {
    let mut doc = Document::from_text(
        "# Day\n\n## Meetings\n\n### Standup\n- note one\n- note two\n\n## Notes\n\n## To-dos\n",
    );
    let heading = doc.meetings()[0].heading_line;
    set_metadata_field(&mut doc.lines, heading, "Started", "09:15");
    let text = doc.to_text();
    assert!(text.contains("- note one\n"), "note one should remain: {}", text);
    assert!(text.contains("- note two\n"), "note two should remain: {}", text);
}
```

- [ ] **Step 1.2: Run tests to confirm they fail**

```bash
cargo test -p buff -- set_metadata_field is_metadata_line 2>&1 | head -40
```

Expected: compile errors (functions don't exist yet) or test failures.

- [ ] **Step 1.3: Add the new constants and functions**

In `src/model/writer.rs`, replace the block starting at line 491 (`const TIME_FIELD_ORDER`) through the end of `set_meeting_time_field` (line 543) with:

```rust
const METADATA_FIELD_ORDER: &[&str] = &["Purpose", "Topic", "Scheduled", "Started", "Ended"];

/// Legacy bare time-field keys written before the `meta:` prefix was introduced.
const LEGACY_TIME_KEYS: &[&str] = &["Scheduled", "Started", "Ended"];

/// True if the line begins with the `meta:` storage prefix that identifies a metadata line.
fn is_metadata_line(line: &str) -> bool {
    line.starts_with("meta:")
}

/// True if the line is a legacy (pre-migration) bare time-field: `Scheduled: `, `Started: `, `Ended: `.
fn is_legacy_time_field_line(line: &str) -> bool {
    LEGACY_TIME_KEYS
        .iter()
        .any(|k| line.starts_with(&format!("{}: ", k)))
}

/// Insert or replace a metadata field (`meta:Key: value`) in the block immediately
/// after the heading at `heading_line`.
///
/// The metadata block is any consecutive run of `meta:` lines (or legacy bare
/// time-field lines, which are transparently migrated to `meta:` format on write).
/// The block is always rewritten in the canonical order defined by `METADATA_FIELD_ORDER`.
pub fn set_metadata_field(
    lines: &mut Vec<String>,
    heading_line: usize,
    key: &str,
    value: &str,
) {
    // Find the end of the existing metadata block (supports both formats).
    let mut meta_end = heading_line + 1;
    while meta_end < lines.len()
        && (is_metadata_line(&lines[meta_end]) || is_legacy_time_field_line(&lines[meta_end]))
    {
        meta_end += 1;
    }

    // Parse existing fields into a map, stripping `meta:` prefix when present.
    let mut fields: std::collections::HashMap<String, String> =
        lines[heading_line + 1..meta_end]
            .iter()
            .filter_map(|line| {
                let data = line.strip_prefix("meta:").unwrap_or(line.as_str());
                let mut parts = data.splitn(2, ": ");
                let k = parts.next()?.to_string();
                let v = parts.next()?.to_string();
                Some((k, v))
            })
            .collect();

    // Insert or overwrite the target key.
    fields.insert(key.to_string(), value.to_string());

    // Rebuild in canonical order with `meta:` prefix (only keys that are present).
    let new_lines: Vec<String> = METADATA_FIELD_ORDER
        .iter()
        .filter_map(|k| fields.get(*k).map(|v| format!("meta:{}: {}", k, v)))
        .collect();

    // Replace the old metadata range with the new lines.
    lines.drain(heading_line + 1..meta_end);
    for (i, line) in new_lines.into_iter().enumerate() {
        lines.insert(heading_line + 1 + i, line);
    }
}
```

- [ ] **Step 1.4: Update `meetings_with_scheduled` to read both formats**

Replace the body of `meetings_with_scheduled` (lines 106–123 of `src/model/writer.rs`) with:

```rust
pub fn meetings_with_scheduled(&self) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for meeting in self.meetings() {
        for line in &self.lines[meeting.heading_line + 1..] {
            // Accept both new `meta:Scheduled: HH:MM` and legacy `Scheduled: HH:MM`.
            let value = line.strip_prefix("meta:Scheduled: ")
                .or_else(|| line.strip_prefix("Scheduled: "));
            if let Some(v) = value {
                if !v.is_empty() {
                    result.push((v.to_string(), meeting.name.clone()));
                }
                break;
            }
            // Stop scanning if neither a meta: line nor a legacy time-field line.
            if !is_metadata_line(line) && !is_legacy_time_field_line(line) {
                break;
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}
```

- [ ] **Step 1.5: Update existing `set_time_field_*` tests to use new format**

In `src/model/writer.rs` tests, replace all direct calls to `set_meeting_time_field` with `set_metadata_field` and update string assertions from `"Scheduled: 09:00"` to `"meta:Scheduled: 09:00"`, `"Started: 09:15"` to `"meta:Started: 09:15"`, etc. The affected tests are:

- `set_time_field_inserts_started_when_absent` → rename + update assertions
- `set_time_field_overwrites_existing_started` → rename + update assertions
- `set_time_field_canonical_order` → rename + update assertions
- `set_time_field_does_not_eat_note_content` → rename + update assertions
- `set_time_field_all_three_fields` → rename + update assertions

For reference, each test that previously called:
```rust
set_meeting_time_field(&mut doc.lines, heading, "Started", "09:15");
```
should now call:
```rust
set_metadata_field(&mut doc.lines, heading, "Started", "09:15");
```
And any assertion like:
```rust
assert!(text.contains("Started: 09:15\n"), ...)
```
should become:
```rust
assert!(text.contains("meta:Started: 09:15\n"), ...)
```

- [ ] **Step 1.6: Run the writer tests to verify they pass**

```bash
cargo test -p buff --lib model::writer 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 1.7: Commit**

```bash
git add src/model/writer.rs
git commit -m "refactor(writer): generalize metadata to meta: prefix with set_metadata_field"
```

---

## Task 2: Add `Purpose` and `Topic` commands to the parser

**Files:**
- Modify: `src/app/command.rs`
- Test: `src/app/command.rs` (existing `mod tests` block)

- [ ] **Step 2.1: Write the failing tests**

Add inside `mod tests` in `src/app/command.rs`:

```rust
#[test]
fn parse_purpose_with_text() {
    assert_eq!(
        parse("/purpose kick off Q3"),
        Command::Purpose("kick off Q3".to_string())
    );
}

#[test]
fn parse_purpose_empty_is_invalid() {
    assert_eq!(
        parse("/purpose"),
        Command::InvalidArgs("/purpose needs text".to_string())
    );
}

#[test]
fn parse_topic_with_text() {
    assert_eq!(
        parse("/topic API design for v2"),
        Command::Topic("API design for v2".to_string())
    );
}

#[test]
fn parse_topic_empty_is_invalid() {
    assert_eq!(
        parse("/topic"),
        Command::InvalidArgs("/topic needs text".to_string())
    );
}
```

- [ ] **Step 2.2: Run the tests to confirm they fail**

```bash
cargo test -p buff --lib app::command 2>&1 | tail -20
```

Expected: compile error — `Command::Purpose` and `Command::Topic` don't exist yet.

- [ ] **Step 2.3: Add the enum variants**

In `src/app/command.rs`, add two new variants to the `Command` enum after line 17 (`Scheduled(String),`):

```rust
Purpose(String),
Topic(String),
```

- [ ] **Step 2.4: Add the parse arms**

In the `match cmd` block in `parse()` (before the `_ =>` wildcard arm at line 116), add:

```rust
"/purpose" => {
    if rest.is_empty() {
        Command::InvalidArgs("/purpose needs text".to_string())
    } else {
        Command::Purpose(rest.to_string())
    }
}
"/topic" => {
    if rest.is_empty() {
        Command::InvalidArgs("/topic needs text".to_string())
    } else {
        Command::Topic(rest.to_string())
    }
}
```

- [ ] **Step 2.5: Run the tests to verify they pass**

```bash
cargo test -p buff --lib app::command 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 2.6: Commit**

```bash
git add src/app/command.rs
git commit -m "feat(command): add Purpose and Topic command variants"
```

---

## Task 3: Add dispatch handlers and update existing ones

**Files:**
- Modify: `src/app/actions.rs`
- Test: `src/app/actions.rs` (existing `mod tests` block)

- [ ] **Step 3.1: Write the failing tests**

Add inside `mod tests` in `src/app/actions.rs`:

```rust
#[test]
fn purpose_in_meeting_inserts_meta_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Purpose("align team".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(
        text.contains("meta:Purpose: align team\n"),
        "Purpose line missing: {}",
        text
    );
    let heading_pos = text.find("### Standup").unwrap();
    let purpose_pos = text.find("meta:Purpose:").unwrap();
    assert!(purpose_pos > heading_pos, "Purpose should be after heading");
}

#[test]
fn purpose_outside_meeting_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    let before = state.doc.to_text();
    dispatch(&mut state, Command::Purpose("oops".to_string())).unwrap();
    assert_eq!(state.status, "Not in a meeting");
    assert_eq!(state.doc.to_text(), before);
}

#[test]
fn purpose_replaces_existing_value() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Purpose("first goal".to_string())).unwrap();
    dispatch(&mut state, Command::Purpose("revised goal".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(text.contains("meta:Purpose: revised goal\n"), "got: {}", text);
    assert!(!text.contains("meta:Purpose: first goal\n"), "old should be gone: {}", text);
}

#[test]
fn topic_in_note_block_inserts_meta_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Note(Some("Design".to_string()))).unwrap();
    dispatch(&mut state, Command::Topic("API v2".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(
        text.contains("meta:Topic: API v2\n"),
        "Topic line missing: {}",
        text
    );
    let heading_pos = text.find("### Design").unwrap();
    let topic_pos = text.find("meta:Topic:").unwrap();
    assert!(topic_pos > heading_pos, "Topic should be after heading");
}

#[test]
fn topic_outside_note_block_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    // Notes context (not a NoteBlock)
    let before = state.doc.to_text();
    dispatch(&mut state, Command::Topic("oops".to_string())).unwrap();
    assert_eq!(state.status, "Not in a note block");
    assert_eq!(state.doc.to_text(), before);
}

#[test]
fn topic_outside_meeting_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    let before = state.doc.to_text();
    // In Meeting context, not NoteBlock — topic should fail
    dispatch(&mut state, Command::Topic("oops".to_string())).unwrap();
    assert_eq!(state.status, "Not in a note block");
    assert_eq!(state.doc.to_text(), before);
}
```

Also update these **existing** tests that check for old bare-field format. Find and update:

- `start_in_meeting_inserts_started_line`: change `text.contains("Started: ")` → `text.contains("meta:Started: ")`
- `end_in_meeting_inserts_ended_line`: change `text.contains("Ended: ")` → `text.contains("meta:Ended: ")`
- `scheduled_in_meeting_inserts_scheduled_line`: change `text.contains("Scheduled: 09:00\n")` → `text.contains("meta:Scheduled: 09:00\n")`
- `start_twice_overwrites_started_line` (if present after line 1364): update same pattern

- [ ] **Step 3.2: Run the tests to confirm they fail**

```bash
cargo test -p buff --lib app::actions 2>&1 | tail -30
```

Expected: compile errors (`Command::Purpose`, `Command::Topic` unhandled in `dispatch`) plus failures on the updated existing tests.

- [ ] **Step 3.3: Add dispatch arms**

In `dispatch()` in `src/app/actions.rs`, add two new arms after the `Command::Scheduled` arm (line 105):

```rust
Command::Purpose(text)     => handle_purpose(state, &text)?,
Command::Topic(text)       => handle_topic(state, &text)?,
```

- [ ] **Step 3.4: Update existing handlers to use `set_metadata_field`**

Replace the three bodies in `handle_start`, `handle_end`, `handle_scheduled` that call `set_meeting_time_field` with `set_metadata_field`:

In `handle_start` (around line 235):
```rust
crate::model::writer::set_metadata_field(
    &mut state.doc.lines, heading, "Started", &time,
);
```

In `handle_end` (around line 250):
```rust
crate::model::writer::set_metadata_field(
    &mut state.doc.lines, heading, "Ended", &time,
);
```

In `handle_scheduled` (around line 264):
```rust
crate::model::writer::set_metadata_field(
    &mut state.doc.lines, heading, "Scheduled", time,
);
```

- [ ] **Step 3.5: Add `handle_purpose` and `handle_topic` functions**

Add after `handle_scheduled` in `src/app/actions.rs`:

```rust
fn handle_purpose(state: &mut AppState, text: &str) -> anyhow::Result<()> {
    let ord = match &state.context {
        Context::Meeting(ord) => *ord,
        _ => {
            state.status = "Not in a meeting".to_string();
            return Ok(());
        }
    };
    if let Some(heading) = state.doc.meetings().get(ord).map(|m| m.heading_line) {
        crate::model::writer::set_metadata_field(
            &mut state.doc.lines, heading, "Purpose", text,
        );
        after_doc_mutation(state)?;
    }
    Ok(())
}

fn handle_topic(state: &mut AppState, text: &str) -> anyhow::Result<()> {
    let ord = match &state.context {
        Context::NoteBlock(ord) => *ord,
        _ => {
            state.status = "Not in a note block".to_string();
            return Ok(());
        }
    };
    if let Some(heading) = state.doc.note_headings().get(ord).map(|n| n.heading_line) {
        crate::model::writer::set_metadata_field(
            &mut state.doc.lines, heading, "Topic", text,
        );
        after_doc_mutation(state)?;
    }
    Ok(())
}
```

- [ ] **Step 3.6: Run the tests to verify they pass**

```bash
cargo test -p buff --lib app::actions 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 3.7: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat(actions): add handle_purpose and handle_topic; update handlers to set_metadata_field"
```

---

## Task 4: Add `metadata` color to the theme

**Files:**
- Modify: `src/ui/theme.rs`
- Modify: `src/config.rs`
- Test: `src/ui/theme.rs` (existing `mod tests` block)

- [ ] **Step 4.1: Write the failing tests**

Add inside `mod tests` in `src/ui/theme.rs`:

```rust
#[test]
fn light_theme_has_metadata_color() {
    let theme = light();
    // metadata should be a dim/dark color distinguishable from normal text
    assert_ne!(theme.metadata, Color::Reset);
}

#[test]
fn dark_theme_has_metadata_color() {
    let theme = dark();
    assert_ne!(theme.metadata, Color::Reset);
}

#[test]
fn resolve_applies_metadata_override() {
    let mut overrides = ThemeOverrides::default();
    overrides.metadata = Some("cyan".to_string());
    let theme = resolve_theme("light", &overrides);
    assert_eq!(theme.metadata, Color::Cyan);
}
```

- [ ] **Step 4.2: Run the tests to confirm they fail**

```bash
cargo test -p buff --lib ui::theme 2>&1 | tail -20
```

Expected: compile error — `metadata` field doesn't exist.

- [ ] **Step 4.3: Add `metadata` field to `Theme` struct**

In `src/ui/theme.rs`, add `metadata: Color` to the `Theme` struct after `capture_bg`:

```rust
pub capture_bg: Color,
pub metadata: Color,
```

- [ ] **Step 4.4: Add `metadata` color values to both theme constructors**

In `light()`:
```rust
metadata: Color::DarkGray,
```

In `dark()`:
```rust
metadata: Color::Gray,
```

- [ ] **Step 4.5: Add `metadata` to `ThemeOverrides` in `src/config.rs`**

In the `ThemeOverrides` struct (around line 132), add after `capture_bg`:

```rust
pub capture_bg: Option<String>,
pub metadata: Option<String>,
```

- [ ] **Step 4.6: Add `apply!(metadata)` to `resolve_theme`**

In `resolve_theme()` in `src/ui/theme.rs`, add after `apply!(capture_bg);`:

```rust
apply!(metadata);
```

- [ ] **Step 4.7: Run the tests to verify they pass**

```bash
cargo test -p buff --lib ui::theme 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 4.8: Commit**

```bash
git add src/ui/theme.rs src/config.rs
git commit -m "feat(theme): add metadata color field"
```

---

## Task 5: Classify and render `meta:` lines in the display layer

**Files:**
- Modify: `src/ui/document.rs`
- Test: `src/ui/document.rs` (existing `mod tests` block)

- [ ] **Step 5.1: Write the failing tests**

Add inside `mod tests` in `src/ui/document.rs`:

```rust
#[test]
fn classify_meta_field_strips_prefix() {
    let mut in_code = false;
    let result = classify_line("meta:Purpose: kick off Q3", &mut in_code, false);
    assert_eq!(result, LineKind::MetaField("Purpose: kick off Q3"));
}

#[test]
fn classify_meta_field_scheduled() {
    let mut in_code = false;
    let result = classify_line("meta:Scheduled: 09:00", &mut in_code, false);
    assert_eq!(result, LineKind::MetaField("Scheduled: 09:00"));
}

#[test]
fn classify_meta_field_on_cursor_line_shows_raw() {
    // vim cursor always shows raw text, even for meta: lines
    let mut in_code = false;
    let result = classify_line("meta:Purpose: kick off Q3", &mut in_code, true);
    assert_eq!(result, LineKind::VimCursor("meta:Purpose: kick off Q3"));
}

#[test]
fn render_meta_field_italic_metadata_color() {
    let t = th();
    let line = render_line_kind(LineKind::MetaField("Purpose: kick off Q3"), &t);
    assert_eq!(
        line,
        Line::from(Span::styled(
            "Purpose: kick off Q3",
            Style::default()
                .fg(t.metadata)
                .add_modifier(Modifier::ITALIC),
        ))
    );
}

#[test]
fn style_line_meta_field_round_trip() {
    // style_line is the public-facing function — verify end-to-end
    let t = th();
    let mut in_code = false;
    let result = style_line("meta:Started: 09:05", &mut in_code, false, &t);
    assert_eq!(
        result,
        Line::from(Span::styled(
            "Started: 09:05",
            Style::default()
                .fg(t.metadata)
                .add_modifier(Modifier::ITALIC),
        ))
    );
}
```

- [ ] **Step 5.2: Run the tests to confirm they fail**

```bash
cargo test -p buff --lib ui::document 2>&1 | tail -20
```

Expected: compile error — `LineKind::MetaField` doesn't exist.

- [ ] **Step 5.3: Add `MetaField` variant to `LineKind`**

In `src/ui/document.rs`, add to the `LineKind` enum (after `Plain`, before the closing `}`):

```rust
/// Metadata line stored with `meta:` prefix. Contains the text after stripping `meta:`.
/// Example: `meta:Purpose: kick off Q3` → `MetaField("Purpose: kick off Q3")`.
MetaField(&'a str),
```

- [ ] **Step 5.4: Add classification arm in `classify_line`**

In `classify_line()` in `src/ui/document.rs`, add before the final `LineKind::Plain(line)` return:

```rust
if let Some(rest) = line.strip_prefix("meta:") {
    return LineKind::MetaField(rest);
}
```

- [ ] **Step 5.5: Add render arm in `render_line_kind`**

In `render_line_kind()` in `src/ui/document.rs`, add before the `LineKind::Ordered` arm (or anywhere before the wildcard — the match is exhaustive):

```rust
LineKind::MetaField(rest) => Line::from(Span::styled(
    rest,
    Style::default()
        .fg(theme.metadata)
        .add_modifier(Modifier::ITALIC),
)),
```

- [ ] **Step 5.6: Run the tests to verify they pass**

```bash
cargo test -p buff --lib ui::document 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 5.7: Run the full test suite**

```bash
cargo test -p buff 2>&1 | tail -30
```

Expected: all tests pass, zero failures.

- [ ] **Step 5.8: Commit**

```bash
git add src/ui/document.rs
git commit -m "feat(document): classify and render meta: metadata lines as MetaField"
```

---

## Task 6: Final build and smoke test

- [ ] **Step 6.1: Build the release binary**

```bash
cargo build --release 2>&1 | tail -10
```

Expected: compiles cleanly with no warnings about unused `set_meeting_time_field` (the function was removed).

- [ ] **Step 6.2: Smoke-test in the terminal**

```bash
cargo run 2>&1 &
```

Manually verify:
1. Create a meeting with `/meeting Standup`
2. Run `/purpose Align on sprint blockers` — verify `meta:Purpose: Align on sprint blockers` is in the raw file
3. Verify the display shows `Purpose: Align on sprint blockers` in italic/muted color
4. Run `/start` — verify `meta:Started: HH:MM` appears, canonical order is `Purpose → Scheduled → Started → Ended`
5. Create a note block with `/note Design`
6. Run `/topic API v2 design` — verify `meta:Topic: API v2 design` in raw file and displays without prefix
7. Run `/purpose …` while in note context — verify status `"Not in a meeting"`
8. Run `/topic …` while in meeting context — verify status `"Not in a note block"`

- [ ] **Step 6.3: Commit completion marker**

```bash
git add -A
git commit -m "chore: metadata slash commands implementation complete"
```
