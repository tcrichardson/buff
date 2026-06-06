# Design: Eliminate DRY Violation in `actions::dispatch`

**Date:** 2026-06-05  
**Status:** Approved  
**Context:** `src/app/actions.rs::dispatch` (complexity 29) repeats an identical four-line post-mutation sequence across four match arms. This spec covers extracting that sequence into a private helper.

---

## Problem

Four arms of `dispatch` — `Entry`, `Meeting`, `Note(Some(_))`, and `Todo` — each end with the exact same four lines:

```rust
state.selectables = state.doc.selectables();
state.save()?;
state.dates_with_notes =
    crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
state.status.clear();
```

This is a DRY violation: the invariant "these four things always happen together after writing to the document" is implied by repetition rather than enforced by code. If a new doc-writing command is added, all four lines must be remembered and copied correctly.

---

## Solution

Extract a private function `after_doc_mutation` in `src/app/actions.rs` that performs the four-step sync. Each of the four arms calls it instead of repeating the sequence inline.

---

## Implementation

### New private function

Add immediately before `dispatch` in `src/app/actions.rs`:

```rust
fn after_doc_mutation(state: &mut AppState) -> anyhow::Result<()> {
    state.selectables = state.doc.selectables();
    state.save()?;
    state.dates_with_notes =
        crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
    state.status.clear();
    Ok(())
}
```

### Refactored dispatch arms

**`Command::Entry`** — the arm becomes (full body shown):
```rust
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
    Context::NoteBlock(ord) => EntryTarget::NoteBlock(*ord),
};
state.doc.add_block(&target, &block);
after_doc_mutation(state)?;
```

**`Command::Meeting`** — replace final 4 lines with:
```rust
let ord = state.doc.add_meeting(&state.current_time_hhmm(), &name);
state.context = Context::Meeting(ord);
state.update_context_display();
after_doc_mutation(state)?;
```

**`Command::Note(Some(n))`** — replace final 4 lines with:
```rust
let ord = state.doc.add_note_heading(&n);
state.context = Context::NoteBlock(ord);
state.update_context_display();
after_doc_mutation(state)?;
```

Note: `Command::Note(None)` is unchanged — it does not write to the document and does not call `after_doc_mutation`.

**`Command::Todo`** — replace final 4 lines with:
```rust
state.doc.add_todo(&text, meeting_name.as_deref());
after_doc_mutation(state)?;
```

---

## What Does Not Change

- All other match arms in `dispatch` (`Leave`, `Help`, `Quit`, `Summarize`, `Unknown`, `InvalidArgs`, `Today`, `Goto`)
- All other public functions in `actions.rs`
- The public API of `dispatch`
- All behavior — this is a pure structural refactor
- No new tests required — all 43 existing tests in `actions.rs` cover the refactored arms

---

## Expected Impact

- **Lines removed:** ~16 (four repetitions of the 4-line sequence replaced by four 1-line calls)
- **Complexity reduction:** Measurable but moderate — the 13 match arms remain, but the per-arm branch count drops
- **Maintainability:** New doc-writing commands now have a single obvious call to make; missing it becomes an obvious omission rather than a silent copy-paste failure

---

## Out of Scope

- Further structural decomposition of `dispatch` (separating doc-writers from context-changers from navigation/status)
- Any changes to `state.rs`, `command.rs`, or the model layer
- Changes to any other function in `actions.rs`
