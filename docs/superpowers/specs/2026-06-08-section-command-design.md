# Design: `/section` Command

**Date:** 2026-06-08
**Status:** Approved

---

## Overview

Add a `/section <name>` command that inserts a Markdown heading one level deeper than the current context and routes subsequent entries under that heading. Works inside meetings, note-blocks, and existing sections. Max depth is H6.

### Example

```
/meeting Daily Standup        → ### Daily Standup  (H3, context = Meeting)
/section Tanner's Update      → #### Tanner's Update  (H4, context = Section)
  - point one                 → appended under #### Tanner's Update
/section Action Items         → ##### Action Items  (H5, context = Section)
  - follow up                 → appended under ##### Action Items
```

To exit a section: press `Esc`, navigate to the desired parent heading, and press `Enter` to resume it.

---

## Decisions

| Question | Answer |
|---|---|
| Does `/section` change entry context? | Yes — entries route under the new heading |
| Nested `/section` calls? | Go one level deeper each time (H4 → H5 → H6) |
| How to exit a section? | `Esc` + navigate to a heading + `Enter` |
| Where is `/section` valid? | `Meeting`, `NoteBlock`, or `Section` context only |
| At H6 already? | Error: "already at maximum depth" |

---

## Approach

`Context::Section { heading_line: usize, level: u8 }` — a new variant on the existing `Context` enum. `heading_line` is the index in `doc.lines` of the inserted heading. Since all insertions go *after* this line, the index never shifts once placed.

No new collection state on `Document`. No new ordinal tracking. Entry insertion for a section scans forward from `heading_line + 1` to find the section end (the first line whose heading level is ≤ `level`).

---

## Files Changed

Six files, no new files.

### `src/model/day.rs`

Add `Section` variant to `EntryTarget`:

```rust
pub enum EntryTarget {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
    Section { heading_line: usize, level: u8 },  // NEW
}
```

### `src/app/state.rs`

Add `Section` variant to `Context`:

```rust
pub enum Context {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
    Section { heading_line: usize, level: u8 },  // NEW
}
```

Update `update_context_display()`:

```rust
Context::Section { heading_line, .. } => {
    let name = self.doc.lines.get(*heading_line)
        .map(|l| l.trim_start_matches('#').trim_start())
        .unwrap_or("section");
    format!("context: {}", name)
}
```

### `src/app/command.rs`

Add `Section(String)` variant to `Command` enum and a parse arm:

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

### `src/model/writer.rs`

Extract a private helper `insertion_index_for_target(&self, target: &EntryTarget) -> usize` that contains the match logic currently inlined in `add_block`. Both `add_block` and the new `add_section_heading` call this helper, eliminating duplication.

**New method `add_section_heading()`** — calls the shared helper to find the insertion point, inserts the heading line, and returns the line index so the caller can store it in `Context::Section`:

```rust
pub fn add_section_heading(&mut self, target: &EntryTarget, level: u8, name: &str) -> usize {
    let insert_idx = self.insertion_index_for_target(target);
    let hashes = "#".repeat(level as usize);
    self.lines.insert(insert_idx, format!("{} {}", hashes, name));
    insert_idx
}
```

**`add_block()` — new `EntryTarget::Section` arm** — section ends at the first line whose heading level is ≤ `level`:

```rust
EntryTarget::Section { heading_line, level } => {
    let start = *heading_line;
    let end = self.lines.iter().enumerate().skip(start + 1)
        .position(|(_, line)| {
            heading_level(line).map_or(false, |lv| lv <= *level as usize)
        })
        .map(|i| start + 1 + i)
        .unwrap_or(self.lines.len());
    block_insert_index(&self.lines, start, end)
}
```

### `src/app/actions.rs`

**`Command::Entry` routing** — add Section arm to the context→target match:

```rust
Context::Section { heading_line, level } =>
    EntryTarget::Section { heading_line: *heading_line, level: *level },
```

**New `Command::Section(name)` dispatch arm:**

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
        Context::Section { heading_line, level } =>
            EntryTarget::Section { heading_line: *heading_line, level: *level },
        Context::Notes => unreachable!(),
    };
    let next_level = current_level + 1;
    let heading_line = state.doc.add_section_heading(&target, next_level, &name);
    state.context = Context::Section { heading_line, level: next_level };
    state.update_context_display();
    after_doc_mutation(state)?;
}
```

**`resume_selected_heading()`** — add `MarkdownHeading` branch so `Esc`+navigate+`Enter` works for user-created sections:

```rust
SelectableKind::MarkdownHeading => {
    let level = crate::model::parser::heading_level(&sel.text)
        .unwrap_or(4) as u8;
    let heading_line = sel.lines.start;
    state.context = Context::Section { heading_line, level };
    state.update_context_display();
    state.focus = Focus::Capture;
    state.status.clear();
    return;
}
```

Error message update: `"not a meeting, note, or section"`.

### `src/ui/help.rs`

Add one line in the Commands block:

```
  /section "Name"  add sub-section (one heading deeper, max ######)
```

---

## Error Cases

| Condition | Behavior |
|---|---|
| `/section` with no name | `InvalidArgs`: `/section needs a name` |
| `/section` outside Meeting/NoteBlock/Section | Status: `Not in a meeting or note` |
| `/section` when already at H6 | Status: `/section: already at maximum depth (######)` |

---

## Not in Scope (v1)

- `/todo` tagging with section name (falls through to untagged todo — acceptable)
- `/leave` does not pop to parent section (use navigate mode instead)
- No explicit `/endsection` or `/up` command
