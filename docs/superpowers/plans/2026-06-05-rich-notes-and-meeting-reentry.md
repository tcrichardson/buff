# Rich Notes, Multi-line Capture & Meeting Re-entry — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three capture/navigation features to Kua-Tin — re-entering an existing meeting, raw-Markdown notes (headings/quotes/lists/code), and multi-line entries — built on a block-aware selectable model.

**Architecture:** The `Document` stays a `Vec<String>` of lines (verbatim preservation). A new block classifier groups lines into selectable **blocks** spanning line ranges. A single passthrough formatter turns composed input into Markdown lines (plain text → bullet; Markdown → verbatim) and is shared by both "add new entry" and "edit existing block". Navigate mode gains whole-block edit/delete and meeting re-entry via Enter. The capture bar becomes multi-line via `Alt+Enter`.

**Tech Stack:** Rust (edition 2024), Ratatui 0.30 + crossterm, chrono, anyhow; tests with `tempfile` and Ratatui `TestBackend`.

**Spec:** `docs/superpowers/specs/2026-06-05-rich-notes-and-meeting-reentry-design.md`

---

## File Structure

- `src/model/day.rs` — **modify**: expand `SelectableKind`; change `Selectable.line: usize` → `lines: Range<usize>`.
- `src/model/parser.rs` — **modify**: add pure line-classification helpers (`heading_level`, `is_section_heading`, `is_bullet`, `todo_state`, `is_ordered`, `is_quote`, `is_fence`, `continuation_end`).
- `src/model/writer.rs` — **modify**: add `looks_like_markdown`, `format_entry`, `add_block`, `replace_selectable`; rewrite `selectables`; make `toggle_todo`/`delete_selectable` range-based; reimplement `add_entry` via `add_block`; remove `edit_selectable`.
- `src/app/actions.rs` — **modify**: `dispatch` uses `format_entry`+`add_block`; `commit_edit` uses `format_entry`+`replace_selectable`; add `resume_selected_meeting`.
- `src/ui/document.rs` — **modify**: range-based whole-block highlight + scroll; styling for quote/code/numbered/`*`/`+` bullets.
- `src/ui/capture.rs` — **modify**: render multi-line input with correct cursor.
- `src/ui/layout.rs` — **modify**: dynamic capture-box height.
- `src/main.rs` — **modify**: `Alt+Enter` inserts newline; `Enter` commits; `Enter` resumes meeting in navigate mode.
- `README.md` — **modify**: document the three features and the Option-as-Meta requirement.

---

## Task 1: Passthrough formatter (`format_entry` + `looks_like_markdown`)

Pure functions, additive — nothing else changes yet. These convert composed input into the Markdown lines to store.

**Files:**
- Modify: `src/model/writer.rs`
- Test: `src/model/writer.rs` (its `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write failing tests**

Add these tests inside the existing `mod tests` in `src/model/writer.rs`:

```rust
#[test]
fn format_plain_single_line_becomes_bullet() {
    assert_eq!(format_entry("hello world", None), vec!["- hello world"]);
}

#[test]
fn format_plain_single_line_with_timestamp() {
    assert_eq!(format_entry("hello", Some("09:20")), vec!["- 09:20 hello"]);
}

#[test]
fn format_plain_multiline_indents_continuation() {
    assert_eq!(
        format_entry("first\nsecond\nthird", None),
        vec!["- first", "  second", "  third"]
    );
}

#[test]
fn format_plain_multiline_timestamp_first_line_only() {
    assert_eq!(
        format_entry("first\nsecond", Some("09:20")),
        vec!["- 09:20 first", "  second"]
    );
}

#[test]
fn format_heading_passthrough_verbatim() {
    assert_eq!(format_entry("## Section", None), vec!["## Section"]);
}

#[test]
fn format_quote_passthrough_verbatim() {
    assert_eq!(format_entry("> a quote", Some("09:20")), vec!["> a quote"]);
}

#[test]
fn format_ordered_list_passthrough_verbatim() {
    assert_eq!(format_entry("1. first", None), vec!["1. first"]);
}

#[test]
fn format_explicit_bullet_passthrough_verbatim() {
    assert_eq!(format_entry("- already", None), vec!["- already"]);
}

#[test]
fn format_code_fence_multiline_verbatim() {
    assert_eq!(
        format_entry("```rust\nfn main() {}\n```", None),
        vec!["```rust", "fn main() {}", "```"]
    );
}

#[test]
fn format_strips_trailing_blank_lines() {
    assert_eq!(format_entry("hello\n", None), vec!["- hello"]);
}

#[test]
fn looks_like_markdown_detects_signals() {
    assert!(looks_like_markdown("# h"));
    assert!(looks_like_markdown("###### h"));
    assert!(looks_like_markdown("> q"));
    assert!(looks_like_markdown("```"));
    assert!(looks_like_markdown("- b"));
    assert!(looks_like_markdown("* b"));
    assert!(looks_like_markdown("+ b"));
    assert!(looks_like_markdown("1. x"));
    assert!(looks_like_markdown("2) x"));
    assert!(!looks_like_markdown("plain text"));
    assert!(!looks_like_markdown("12.5 dollars"));
    assert!(!looks_like_markdown("#nospace"));
}
```

- [ ] **Step 2: Run tests, verify they fail to compile**

Run: `cargo test --lib format_ 2>&1 | head -20`
Expected: compile error — `cannot find function format_entry` / `looks_like_markdown`.

- [ ] **Step 3: Implement the functions**

Add to `src/model/writer.rs` (top of file, after the `use` lines, as free functions — not inside `impl Document`):

```rust
/// True if the first line of an entry already looks like Markdown and should be
/// stored verbatim rather than wrapped in a bullet.
pub fn looks_like_markdown(first_line: &str) -> bool {
    let t = first_line.trim_start();
    if t.starts_with("```") {
        return true;
    }
    if t == ">" || t.starts_with("> ") {
        return true;
    }
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    if crate::model::parser::heading_level(t).is_some() {
        return true;
    }
    crate::model::parser::is_ordered(t)
}

/// Convert composed (possibly multi-line) input into the Markdown lines to store.
/// Plain text becomes a bullet (with optional `HH:MM` timestamp on the first
/// line); anything that looks like Markdown is stored verbatim with no timestamp.
pub fn format_entry(input: &str, timestamp: Option<&str>) -> Vec<String> {
    let mut raw: Vec<&str> = input.split('\n').collect();
    while raw.len() > 1 && raw.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        raw.pop();
    }

    if looks_like_markdown(raw[0]) {
        return raw.iter().map(|s| s.to_string()).collect();
    }

    let mut out = Vec::with_capacity(raw.len());
    let first = match timestamp {
        Some(ts) => format!("- {} {}", ts, raw[0]),
        None => format!("- {}", raw[0]),
    };
    out.push(first);
    for line in &raw[1..] {
        out.push(format!("  {}", line));
    }
    out
}
```

This uses `crate::model::parser::heading_level` and `::is_ordered` (fully qualified, so no module-level `use` is needed and Task 2's function-local import won't clash). Add these two helpers now to `src/model/parser.rs` (they are pure; Task 2 adds the rest):

```rust
/// Number of leading `#` (1..=6) if the line is an ATX heading (`#` then a space).
pub fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line.chars().nth(hashes) == Some(' ') {
        Some(hashes)
    } else {
        None
    }
}

/// True if the line starts an ordered-list item: digits then `. ` or `) `.
pub fn is_ordered(line: &str) -> bool {
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return false;
    }
    let rest = &line[digits..];
    rest.starts_with(". ") || rest.starts_with(") ")
}
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test --lib format_ && cargo test --lib looks_like_markdown`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/model/writer.rs src/model/parser.rs
git commit -m "feat: add markdown passthrough formatter (format_entry)"
```

---

## Task 2: Block-aware selectable model

Change `Selectable` to span line ranges with richer kinds, rewrite `selectables()` as a block classifier, make toggle/delete/replace range-based, and update all call sites and tests so the build is green.

**Files:**
- Modify: `src/model/day.rs`, `src/model/parser.rs`, `src/model/writer.rs`, `src/app/actions.rs`, `src/ui/document.rs`
- Test: the `mod tests` in `src/model/parser.rs`, `src/model/writer.rs`, `src/app/actions.rs`

- [ ] **Step 1: Write failing classifier tests**

Add to `mod tests` in `src/model/writer.rs`:

```rust
#[test]
fn classify_blocks_full_example() {
    let text = "# 2026-06-04 (Thu)\n\n## Meetings\n\n### 09:15 Standup\n\n- point A\n  more A\n\n## Notes\n\n- idea\n> a quote\n1. one\n\n```rust\nfn x() {}\n```\n\n## To-dos\n\n- [ ] todo1\n- [x] todo2\n";
    let doc = Document::from_text(text);
    let sel = doc.selectables();

    let kinds: Vec<_> = sel.iter().map(|s| (s.lines.clone(), s.kind.clone())).collect();
    assert_eq!(
        kinds,
        vec![
            (4..5, SelectableKind::MeetingHeading { ordinal: 0 }),
            (6..8, SelectableKind::Bullet),                 // "- point A" + "  more A"
            (10..11, SelectableKind::Bullet),               // "- idea"
            (11..12, SelectableKind::Quote),
            (12..13, SelectableKind::Numbered),
            (14..17, SelectableKind::CodeBlock),            // fence + body + fence
            (19..20, SelectableKind::Todo { done: false }),
            (20..21, SelectableKind::Todo { done: true }),
        ]
    );
    assert_eq!(sel[1].text, "- point A\n  more A");
    assert_eq!(sel[5].text, "```rust\nfn x() {}\n```");
}

#[test]
fn classify_markdown_heading_in_notes_is_selectable() {
    let doc = Document::from_text("# Day\n\n## Notes\n\n## Subsection\n\n## To-dos\n");
    let sel = doc.selectables();
    assert_eq!(sel.len(), 1);
    assert_eq!(sel[0].kind, SelectableKind::MarkdownHeading);
    assert_eq!(sel[0].text, "## Subsection");
}

#[test]
fn classify_unterminated_fence_runs_to_section_end() {
    let doc = Document::from_text("# Day\n\n## Notes\n\n```\nstuff\n\n## To-dos\n");
    let sel = doc.selectables();
    assert_eq!(sel.len(), 1);
    assert_eq!(sel[0].kind, SelectableKind::CodeBlock);
    assert_eq!(sel[0].lines, 4..7);
}

#[test]
fn classify_raw_external_line_is_selectable() {
    let doc = Document::from_text("# Day\n\n## Notes\n\nplain external line\n\n## To-dos\n");
    let sel = doc.selectables();
    assert_eq!(sel.len(), 1);
    assert_eq!(sel[0].kind, SelectableKind::Raw);
    assert_eq!(sel[0].text, "plain external line");
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test --lib classify_ 2>&1 | head -20`
Expected: compile errors — `SelectableKind::MeetingHeading` etc. not found, `lines` field not found.

- [ ] **Step 3: Update the data model in `src/model/day.rs`**

Add `use std::ops::Range;` at the top. Replace the `SelectableKind` and `Selectable` definitions (lines 16–27) with:

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Selectable {
    pub lines: Range<usize>,
    pub kind: SelectableKind,
    pub text: String,
}
```

- [ ] **Step 4: Add classification helpers to `src/model/parser.rs`**

Append these (alongside `heading_level`/`is_ordered` from Task 1):

```rust
pub fn is_section_heading(line: &str) -> bool {
    matches!(line, "## Meetings" | "## Notes" | "## To-dos")
}

pub fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

pub fn is_quote(line: &str) -> bool {
    let t = line.trim_start();
    t == ">" || t.starts_with("> ")
}

pub fn is_bullet(line: &str) -> bool {
    line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ")
}

/// `Some(false)` for `- [ ]`, `Some(true)` for `- [x]`/`- [X]`, else `None`.
pub fn todo_state(line: &str) -> Option<bool> {
    if line.starts_with("- [ ] ") {
        Some(false)
    } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
        Some(true)
    } else {
        None
    }
}

/// Index after the last continuation line starting at `from`. A continuation
/// line is non-blank and indented by at least two spaces (or a tab).
pub fn continuation_end(lines: &[String], from: usize) -> usize {
    let mut j = from;
    while j < lines.len() {
        let l = &lines[j];
        if l.trim().is_empty() {
            break;
        }
        let indent = l.len() - l.trim_start().len();
        if indent >= 2 || l.starts_with('\t') {
            j += 1;
        } else {
            break;
        }
    }
    j
}
```

- [ ] **Step 5: Rewrite `selectables()` in `src/model/writer.rs`**

Replace the entire existing `selectables` method (lines ~101–131) with:

```rust
pub fn selectables(&self) -> Vec<Selectable> {
    use crate::model::parser::{
        continuation_end, heading_level, is_bullet, is_fence, is_ordered, is_quote,
        is_section_heading, todo_state,
    };

    let lines = &self.lines;
    let meetings_start = heading_line(lines, SectionKind::Meetings);
    let meetings_end = meetings_start.map(|s| section_end(lines, s));

    let mut result = Vec::new();
    let mut i = 0;
    let mut meeting_ord = 0usize;

    let join = |range: std::ops::Range<usize>| lines[range].join("\n");

    while i < lines.len() {
        let line = &lines[i];

        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        // Structural headings (day title at line 0, fixed section headings) are not selectable.
        if (i == 0 && line.starts_with("# ")) || is_section_heading(line) {
            i += 1;
            continue;
        }

        // Code fence: run to the closing fence (or section end / EOF).
        if is_fence(line) {
            let start = i;
            let mut j = i + 1;
            while j < lines.len() && !is_fence(&lines[j]) && !is_section_heading(&lines[j]) {
                j += 1;
            }
            let end = if j < lines.len() && is_fence(&lines[j]) {
                j + 1
            } else {
                j
            };
            result.push(Selectable {
                lines: start..end,
                kind: SelectableKind::CodeBlock,
                text: join(start..end),
            });
            i = end;
            continue;
        }

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

        // Markdown heading typed as a note.
        if heading_level(line).is_some() {
            result.push(Selectable {
                lines: i..i + 1,
                kind: SelectableKind::MarkdownHeading,
                text: line.clone(),
            });
            i += 1;
            continue;
        }

        // Blockquote (consecutive quote lines).
        if is_quote(line) {
            let start = i;
            let mut j = i + 1;
            while j < lines.len() && is_quote(&lines[j]) {
                j += 1;
            }
            result.push(Selectable {
                lines: start..j,
                kind: SelectableKind::Quote,
                text: join(start..j),
            });
            i = j;
            continue;
        }

        // Todo (check before bullet, since "- [ ]" also matches "- ").
        if let Some(done) = todo_state(line) {
            let end = continuation_end(lines, i + 1);
            result.push(Selectable {
                lines: i..end,
                kind: SelectableKind::Todo { done },
                text: join(i..end),
            });
            i = end;
            continue;
        }

        if is_bullet(line) {
            let end = continuation_end(lines, i + 1);
            result.push(Selectable {
                lines: i..end,
                kind: SelectableKind::Bullet,
                text: join(i..end),
            });
            i = end;
            continue;
        }

        if is_ordered(line) {
            let end = continuation_end(lines, i + 1);
            result.push(Selectable {
                lines: i..end,
                kind: SelectableKind::Numbered,
                text: join(i..end),
            });
            i = end;
            continue;
        }

        // Anything else: a single-line Raw block (e.g. external edits).
        result.push(Selectable {
            lines: i..i + 1,
            kind: SelectableKind::Raw,
            text: line.clone(),
        });
        i += 1;
    }

    result
}
```

- [ ] **Step 6: Make `toggle_todo` / `delete_selectable` range-based and replace `edit_selectable`**

In `src/model/writer.rs`, replace `toggle_todo`, `edit_selectable`, and `delete_selectable` with:

```rust
pub fn toggle_todo(&mut self, sel_index: usize) -> anyhow::Result<()> {
    let selectables = self.selectables();
    let sel = selectables
        .get(sel_index)
        .ok_or_else(|| anyhow::anyhow!("index out of bounds"))?;
    match sel.kind {
        SelectableKind::Todo { done } => {
            let li = sel.lines.start;
            let content = &self.lines[li][6..];
            self.lines[li] = if done {
                format!("- [ ] {}", content)
            } else {
                format!("- [x] {}", content)
            };
            Ok(())
        }
        _ => Err(anyhow::anyhow!("not a to-do")),
    }
}

/// Replace the selected block's line range with `new_lines`.
pub fn replace_selectable(&mut self, sel_index: usize, new_lines: &[String]) -> anyhow::Result<()> {
    let selectables = self.selectables();
    let sel = selectables
        .get(sel_index)
        .ok_or_else(|| anyhow::anyhow!("index out of bounds"))?;
    let range = sel.lines.clone();
    self.lines.splice(range, new_lines.iter().cloned());
    Ok(())
}

pub fn delete_selectable(&mut self, sel_index: usize) -> anyhow::Result<()> {
    let selectables = self.selectables();
    let sel = selectables
        .get(sel_index)
        .ok_or_else(|| anyhow::anyhow!("index out of bounds"))?;
    let range = sel.lines.clone();
    self.lines.drain(range);
    Ok(())
}
```

- [ ] **Step 7: Update `src/app/actions.rs` `commit_edit` to use the passthrough + replace**

Replace the body of `commit_edit` (lines ~184–197) with:

```rust
pub fn commit_edit(state: &mut AppState) -> anyhow::Result<()> {
    if let Some(idx) = state.editing {
        let new_lines = crate::model::writer::format_entry(&state.input, None);
        state.doc.replace_selectable(idx, &new_lines)?;
        state.selectables = state.doc.selectables();
        state.editing = None;
        state.input.clear();
        state.focus = crate::app::state::Focus::Navigate;
        state.save()?;
        state.dates_with_notes =
            crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
        state.status.clear();
    }
    Ok(())
}
```

- [ ] **Step 8: Fix `src/ui/document.rs` to use ranges (compile fix + whole-block highlight)**

Replace lines 8–12 (the `selected_line` computation) with:

```rust
    let selected_range = if app.focus == Focus::Navigate {
        app.selectables.get(app.selected).map(|s| s.lines.clone())
    } else {
        None
    };
```

Replace the `is_selected` line (line ~20) with:

```rust
            let is_selected = selected_range
                .as_ref()
                .map(|r| r.contains(&i))
                .unwrap_or(false);
```

Replace the `scroll_offset` block (lines ~99–104) with:

```rust
    let scroll_offset = if let Some(r) = &selected_range {
        let last = r.end.saturating_sub(1);
        let visible_height = area.height as usize;
        (last + 1).saturating_sub(visible_height)
    } else {
        0
    };
```

- [ ] **Step 9: Update existing model/action tests for the new shape**

In `src/model/writer.rs` `mod tests`, **delete** the now-obsolete tests `selectables_over_spec_example`, `edit_entry_keeps_marker_and_swaps_text`, `edit_checked_todo_keeps_marker_and_swaps_text`, and `edit_out_of_bounds_returns_err`. Replace them with:

```rust
#[test]
fn replace_selectable_swaps_block() {
    let mut doc = Document::from_text("# Day\n\n## Notes\n\n- idea\n");
    doc.replace_selectable(0, &["- new idea".to_string()]).unwrap();
    let text = doc.to_text();
    assert!(text.contains("- new idea\n"), "got: {}", text);
    assert!(!text.contains("- idea\n"), "old text should be gone");
}

#[test]
fn replace_selectable_multiline() {
    let mut doc = Document::from_text("# Day\n\n## Notes\n\n- one\n");
    doc.replace_selectable(0, &["- one".to_string(), "  two".to_string()])
        .unwrap();
    assert!(doc.to_text().contains("- one\n  two\n"), "got: {}", doc.to_text());
}

#[test]
fn replace_out_of_bounds_returns_err() {
    let mut doc = Document::from_text("# Day\n\n## Notes\n\n- idea\n");
    assert!(doc.replace_selectable(99, &["x".to_string()]).is_err());
}
```

In `src/model/writer.rs`, fix `delete_updates_selectable_indices` (it references `.line`): change `sel[0].line` → `sel[0].lines.start`, `sel[1].line` → `sel[1].lines.start`, `sel[0].text` from `"first"` → `"- first"`, and `sel[1].text` from `"third"` → `"- third"`.

In `src/app/actions.rs` `mod tests`, fix `edit_flow`: change the assertion `assert_eq!(state.input, "idea");` to `assert_eq!(state.input, "- idea");` (begin-edit now loads raw block text).

- [ ] **Step 10: Build and run the full suite**

Run: `cargo test --lib`
Expected: PASS (all model/action/ui tests green).

- [ ] **Step 11: Commit**

```bash
git add src/model/day.rs src/model/parser.rs src/model/writer.rs src/app/actions.rs src/ui/document.rs
git commit -m "feat: block-aware selectables spanning line ranges"
```

---

## Task 3: Commit path — markdown + multi-line entries via `add_block`

Route new entries through `format_entry` + a new `add_block`, so Notes/Meeting captures support Markdown passthrough and (later) multi-line input.

**Files:**
- Modify: `src/model/writer.rs`, `src/app/actions.rs`
- Test: `mod tests` in `src/model/writer.rs`, `src/app/actions.rs`

- [ ] **Step 1: Write failing tests**

Add to `src/app/actions.rs` `mod tests`:

```rust
#[test]
fn entry_markdown_heading_stored_verbatim() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Entry("## Section".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(text.contains("## Section\n"), "got: {}", text);
    assert!(!text.contains("- ## Section"), "should not be wrapped: {}", text);
}

#[test]
fn entry_multiline_plain_becomes_indented_bullet() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Entry("first\nsecond".to_string())).unwrap();
    assert!(
        state.doc.to_text().contains("- first\n  second\n"),
        "got: {}",
        state.doc.to_text()
    );
}
```

Add to `src/model/writer.rs` `mod tests`:

```rust
#[test]
fn add_block_inserts_multiple_lines_into_notes() {
    let mut doc = Document::from_text("# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
    doc.add_block(&EntryTarget::Notes, &["- one".to_string(), "  two".to_string()]);
    assert!(doc.to_text().contains("## Notes\n- one\n  two\n"), "got: {}", doc.to_text());
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test --lib add_block entry_markdown entry_multiline 2>&1 | head -20`
Expected: compile error — `no method named add_block`.

- [ ] **Step 3: Add `add_block` and reimplement `add_entry`**

In `src/model/writer.rs`, replace the existing `add_entry` method with:

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
    };
    for (k, line) in block.iter().enumerate() {
        self.lines.insert(insert_idx + k, line.clone());
    }
}

pub fn add_entry(&mut self, target: &EntryTarget, text: &str, time: Option<&str>) {
    let bullet = match time {
        Some(t) => format!("- {} {}", t, text),
        None => format!("- {}", text),
    };
    self.add_block(target, &[bullet]);
}
```

- [ ] **Step 4: Switch `dispatch` Entry handler to use `format_entry` + `add_block`**

In `src/app/actions.rs`, replace the `Command::Entry(text)` arm (lines ~30–54) with:

```rust
        Command::Entry(text) => {
            let text = text.trim();
            if text.is_empty() {
                return Ok(());
            }
            let time_str = state.current_time_hhmm();
            let time = if state.config.timestamp_entries {
                Some(time_str.as_str())
            } else {
                None
            };
            let block = crate::model::writer::format_entry(text, time);
            let target = match &state.context {
                Context::Notes => EntryTarget::Notes,
                Context::Meeting(ord) => EntryTarget::Meeting(*ord),
            };
            state.doc.add_block(&target, &block);
            state.selectables = state.doc.selectables();
            state.save()?;
            state.dates_with_notes =
                crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
            state.status.clear();
        }
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: PASS (including the existing `two_plain_lines_append_notes`, `meeting_then_entry_nests_bullet`, etc., which still hold).

- [ ] **Step 6: Commit**

```bash
git add src/model/writer.rs src/app/actions.rs
git commit -m "feat: route entries through passthrough formatter and add_block"
```

---

## Task 4: Resume meeting + key routing (Alt+Enter, Enter)

Add meeting re-entry in navigate mode and the multi-line newline / commit key handling.

**Files:**
- Modify: `src/app/actions.rs`, `src/main.rs`
- Test: `mod tests` in `src/app/actions.rs`

- [ ] **Step 1: Write failing tests for resume**

Add to `src/app/actions.rs` `mod tests`:

```rust
#[test]
fn resume_meeting_sets_context_and_focus() {
    use crate::model::day::SelectableKind;
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Meeting("Standup".to_string())).unwrap();
    dispatch(&mut state, Command::Note).unwrap(); // leave the meeting context
    assert_eq!(state.context, Context::Notes);

    let idx = state
        .selectables
        .iter()
        .position(|s| matches!(s.kind, SelectableKind::MeetingHeading { .. }))
        .expect("meeting heading should be selectable");
    state.selected = idx;
    state.focus = crate::app::state::Focus::Navigate;

    resume_selected_meeting(&mut state);
    assert_eq!(state.context, Context::Meeting(0));
    assert_eq!(state.focus, crate::app::state::Focus::Capture);

    dispatch(&mut state, Command::Entry("under meeting".to_string())).unwrap();
    let text = state.doc.to_text();
    let heading = text.find("### ").unwrap();
    let entry = text.find("- under meeting").unwrap();
    assert!(entry > heading, "entry should be under the meeting heading");
}

#[test]
fn resume_on_non_meeting_sets_status() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Entry("idea".to_string())).unwrap();
    state.selected = 0;
    state.focus = crate::app::state::Focus::Navigate;
    resume_selected_meeting(&mut state);
    assert_eq!(state.status, "not a meeting");
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test --lib resume_ 2>&1 | head -20`
Expected: compile error — `cannot find function resume_selected_meeting`.

- [ ] **Step 3: Implement `resume_selected_meeting`**

Add to `src/app/actions.rs` (after `begin_edit_selected`). Also add `use crate::model::day::SelectableKind;` if not already imported (it is not — add it to the top `use` block):

```rust
pub fn resume_selected_meeting(state: &mut AppState) {
    if let Some(sel) = state.selectables.get(state.selected) {
        if let SelectableKind::MeetingHeading { ordinal } = sel.kind {
            state.context = Context::Meeting(ordinal);
            state.update_context_display();
            state.focus = crate::app::state::Focus::Capture;
            state.status.clear();
            return;
        }
    }
    state.status = "not a meeting".to_string();
}
```

Update the top-of-file import line `use crate::model::day::EntryTarget;` to:

```rust
use crate::model::day::{EntryTarget, SelectableKind};
```

- [ ] **Step 4: Run resume tests**

Run: `cargo test --lib resume_`
Expected: PASS.

- [ ] **Step 5: Wire keys in `src/main.rs`**

In the `Focus::Capture` block, replace the `KeyCode::Enter =>` arm (lines ~195–206) with:

```rust
                        KeyCode::Enter => {
                            if key.modifiers.contains(KeyModifiers::ALT) {
                                app.input.push('\n');
                            } else if app.editing.is_some() {
                                kua_tin::app::actions::commit_edit(&mut app)?;
                            } else {
                                let cmd = kua_tin::app::command::parse(&app.input);
                                kua_tin::app::actions::dispatch(&mut app, cmd)?;
                                if app.overlay != Overlay::None {
                                    app.pending_delete = false;
                                }
                                app.input.clear();
                            }
                        }
```

In the `Focus::Navigate` `match key.code` block, add an `Enter` arm (e.g. right before `KeyCode::Char('?')`):

```rust
                        KeyCode::Enter => {
                            kua_tin::app::actions::resume_selected_meeting(&mut app);
                        }
```

- [ ] **Step 6: Build (main.rs has no unit tests)**

Run: `cargo build`
Expected: compiles with no errors.

- [ ] **Step 7: Commit**

```bash
git add src/app/actions.rs src/main.rs
git commit -m "feat: resume meeting on Enter; Alt+Enter inserts newline"
```

---

## Task 5: Multi-line capture bar

Render the (possibly multi-line) input and grow the capture box.

**Files:**
- Modify: `src/ui/capture.rs`, `src/ui/layout.rs`
- Test: `mod tests` in `src/ui/layout.rs`

- [ ] **Step 1: Write a failing smoke test**

Add to `src/ui/layout.rs` `mod tests`:

```rust
#[test]
fn render_multiline_input() {
    let doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    let mut app = test_app(doc, Focus::Capture, 0);
    app.input = "line one\nline two".to_string();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("line one"), "first line missing: {}", content);
    assert!(content.contains("line two"), "second line missing: {}", content);
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test --lib render_multiline_input 2>&1 | head -30`
Expected: FAIL — only `line one` (with the rest of the single-line input) appears; `line two` is not rendered on its own row (assertion fails or the second line is absent).

- [ ] **Step 3: Make `render_input` multi-line in `src/ui/capture.rs`**

Replace `render_input` with:

```rust
pub fn render_input(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    use ratatui::text::Text;

    let (title, prefix) = if app.editing.is_some() {
        ("Edit", "Edit: › ")
    } else {
        ("Capture", "› ")
    };
    let block = Block::default().title(title).borders(Borders::ALL);

    let input_lines: Vec<&str> = app.input.split('\n').collect();
    let rendered: Vec<Line> = input_lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                Line::from(format!("{}{}", prefix, l))
            } else {
                Line::from((*l).to_string())
            }
        })
        .collect();

    let paragraph = Paragraph::new(Text::from(rendered)).block(block);
    frame.render_widget(paragraph, area);

    let last = input_lines.len() - 1;
    let last_len = input_lines[last].chars().count();
    let col = if last == 0 {
        prefix.chars().count() + last_len
    } else {
        last_len
    };
    let inner_x = area.x + 1;
    let inner_y = area.y + 1;
    frame.set_cursor_position(ratatui::layout::Position::new(
        inner_x + col as u16,
        inner_y + last as u16,
    ));
}
```

- [ ] **Step 4: Make the capture box height dynamic in `src/ui/layout.rs`**

Replace the `Layout` construction (lines 6–14) with:

```rust
    let input_line_count = app.input.split('\n').count().max(1) as u16;
    let input_height = (input_line_count + 2).clamp(3, 12);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(input_height),
        ])
        .split(frame.area());
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: PASS (new test plus existing `render_*`).

- [ ] **Step 6: Commit**

```bash
git add src/ui/capture.rs src/ui/layout.rs
git commit -m "feat: multi-line capture bar with dynamic height"
```

---

## Task 6: Render new block types

Style quotes, code blocks, numbered items, and `*`/`+` bullets in the document pane.

**Files:**
- Modify: `src/ui/document.rs`
- Test: `mod tests` in `src/ui/layout.rs`

- [ ] **Step 1: Write a failing smoke test**

Add to `src/ui/layout.rs` `mod tests`:

```rust
#[test]
fn render_quote_and_code_and_numbered() {
    let doc = Document::from_text(
        "# Day\n\n## Notes\n\n> a quote\n1. first item\n```\ncode line\n```\n\n## To-dos\n",
    );
    let app = test_app(doc, Focus::Capture, 0);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();

    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(content.contains("a quote"), "quote text missing: {}", content);
    assert!(content.contains("first item"), "numbered text missing: {}", content);
    assert!(content.contains("code line"), "code text missing: {}", content);
}
```

- [ ] **Step 2: Run, verify it passes-but-unstyled / fails as written**

Run: `cargo test --lib render_quote_and_code_and_numbered`
Expected: this content assertion PASSES even before styling (text falls through to the raw `else` branch). Treat Step 1 as a regression guard; the real change is visual styling verified in Step 3–4. (If you prefer a failing assertion first, temporarily assert a styled marker like `│`, then implement.)

- [ ] **Step 3: Add styling branches in `src/ui/document.rs`**

Inside the `.map(|(i, line)| { ... })` closure, track code-block state and add branches. Change the closure to take `mut` access to an `in_code` flag declared just before the `.map`. Replace the iterator section (lines ~14–97) with:

```rust
    let mut in_code = false;
    let text_lines: Vec<Line> = app
        .doc
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let is_selected = selected_range
                .as_ref()
                .map(|r| r.contains(&i))
                .unwrap_or(false);
            let highlight = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            let fence = line.trim_start().starts_with("```");
            if in_code || fence {
                if fence {
                    in_code = !in_code;
                }
                return Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(Color::DarkGray),
                ))
                .style(highlight);
            }

            if let Some(rest) = line.strip_prefix("# ") {
                Line::from(vec![Span::styled(
                    format!("# {}", rest),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("## ") {
                Line::from(vec![Span::styled(
                    format!("## {}", rest),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("### ") {
                Line::from(vec![Span::styled(
                    format!("### {}", rest),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("- [ ] ") {
                Line::from(vec![Span::raw("☐ "), Span::raw(rest)]).style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("- [x] ")
                .or_else(|| line.strip_prefix("- [X] "))
            {
                Line::from(vec![
                    Span::styled("☑ ", Style::default().fg(Color::Green)),
                    Span::styled(
                        rest,
                        Style::default().fg(Color::Green).add_modifier(Modifier::CROSSED_OUT),
                    ),
                ])
                .style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("> ")
                .or_else(|| if line == ">" { Some("") } else { None })
            {
                Line::from(vec![
                    Span::styled("│ ", Style::default().fg(Color::Magenta)),
                    Span::styled(rest, Style::default().add_modifier(Modifier::ITALIC)),
                ])
                .style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .or_else(|| line.strip_prefix("+ "))
            {
                Line::from(vec![Span::raw("• "), Span::raw(rest)]).style(highlight)
            } else if crate::model::parser::is_ordered(line) {
                Line::from(Span::raw(line.as_str())).style(highlight)
            } else {
                Line::from(line.as_str()).style(highlight)
            }
        })
        .collect();
```

Note: the headings now keep their prefix in the span text (`format!("# {}", rest)`) so the rendered output still contains `#`/`##`/`###`. This preserves the existing `render_help_overlay`/`render_empty_day` expectations and keeps headings visually intact.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib`
Expected: PASS (new test plus all existing UI tests, including `render_navigate_mode` REVERSED check and `render_populated_day` checkbox glyphs).

- [ ] **Step 5: Commit**

```bash
git add src/ui/document.rs
git commit -m "feat: render quotes, code blocks, and numbered lists"
```

---

## Task 7: Documentation

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the README**

In `README.md`, under "Capture mode (default)", after the slash-command table, add:

```markdown
### Markdown notes

Plain text becomes a bullet. If what you type already looks like Markdown, it is
stored verbatim:

- `# Heading`, `## Subheading` — headings
- `> quoted text` — blockquote
- `1. first`, `2. second` — numbered list
- `- item`, `* item`, `+ item` — bullet
- ```` ``` ```` fenced code blocks

### Multi-line notes

Press **Alt+Enter** (Option+Enter on macOS) to add a line break while composing;
**Enter** commits the whole entry. Plain multi-line text is stored as a single
bullet with the following lines indented; code fences and quotes are stored
verbatim.

> macOS note: enable "Use Option as Meta key" in Terminal.app/iTerm2 settings so
> Option+Enter reaches the app.
```

Under "Navigate mode", update the key table to add:

```markdown
| `Enter` | Re-enter the selected meeting (when a `### HH:MM Name` heading is selected) |
```

And add a sentence after the table: "All entry types — bullets (including multi-line), to-dos, meeting headings, and Markdown blocks — are selectable, editable (`e`), and deletable (`dd`)."

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: document markdown notes, multi-line capture, meeting re-entry"
```

---

## Task 8: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt`
Then: `git diff --quiet || (git add -A && git commit -m "style: cargo fmt")`

- [ ] **Step 2: Lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings. Fix any that appear and commit with `fix: clippy`.

- [ ] **Step 3: Full test suite**

Run: `cargo test`
Expected: all tests PASS.

- [ ] **Step 4: Manual smoke (optional but recommended)**

Run: `cargo run -- --notes-dir /tmp/kuatin-smoke`
Verify by hand: type a plain note (becomes a bullet); type `## Heading` (verbatim heading); compose a two-line note with Alt+Enter (single indented bullet); `/meeting "Standup"`, `/note`, then Esc → select the meeting heading → Enter (context returns to the meeting); `e` on a block edits it; `dd` deletes it.

---

## Self-Review Notes (for the implementer)

- `format_entry` and the classifier must agree on signals: editing a block loads its raw `text`, and re-committing re-runs `format_entry`, so every kind must round-trip. Tests `replace_selectable_*` and `classify_blocks_full_example` guard this.
- Method/field names used consistently across tasks: `Selectable.lines` (Range), `SelectableKind::{Bullet,Todo{done},MeetingHeading{ordinal},MarkdownHeading,Quote,Numbered,CodeBlock,Raw}`, `Document::{selectables,add_block,add_entry,replace_selectable,delete_selectable,toggle_todo}`, `actions::{dispatch,commit_edit,resume_selected_meeting,begin_edit_selected}`, free fns `writer::{format_entry,looks_like_markdown}`, `parser::{heading_level,is_ordered,is_section_heading,is_fence,is_quote,is_bullet,todo_state,continuation_end}`.
- `Selectable.text` is **raw** block text (lines joined with `\n`); `begin_edit_selected` already assigns `state.input = sel.text.clone()`, which is now multi-line aware with no change needed.
