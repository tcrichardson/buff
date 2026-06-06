# actions::dispatch DRY Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the four-way repeated post-mutation sequence in `dispatch` by extracting a private `after_doc_mutation` helper function.

**Architecture:** Add a private `fn after_doc_mutation(state: &mut AppState) -> anyhow::Result<()>` in `src/app/actions.rs` that encapsulates the four steps always performed after writing to the document (refresh selectables, save, refresh dates_with_notes, clear status). Replace the four repeated inline sequences in the `Entry`, `Meeting`, `Note(Some)`, and `Todo` arms of `dispatch` with a single call to this helper.

**Tech Stack:** Rust, existing `anyhow`, `crate::storage`, `crate::app::state::AppState`

**Spec:** `docs/superpowers/specs/2026-06-05-dispatch-dry-refactor-design.md`

---

## File Map

| Action | File | What changes |
|--------|------|-------------|
| Modify | `src/app/actions.rs` | Add private `after_doc_mutation`; update 4 arms of `dispatch` |

---

## Task 1: Extract `after_doc_mutation` and refactor `dispatch`

**Files:**
- Modify: `src/app/actions.rs:28-129`

This is a pure structural refactor — behavior is unchanged. TDD here means verifying all existing tests continue to pass (they already cover the refactored arms). No new tests are needed.

- [ ] **Step 1: Verify the current test suite passes before touching anything**

```bash
cargo test
```

Expected output includes:
```
test result: ok. 182 passed; 0 failed
```

- [ ] **Step 2: Add the `after_doc_mutation` private function**

Insert the following immediately before `pub fn dispatch` (i.e., between line 26 and line 28 in the current file):

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

- [ ] **Step 3: Refactor the `Command::Entry` arm**

Replace the current `Command::Entry` arm (lines 30–53 of the original) with:

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
                Context::NoteBlock(ord) => EntryTarget::NoteBlock(*ord),
            };
            state.doc.add_block(&target, &block);
            after_doc_mutation(state)?;
        }
```

- [ ] **Step 4: Refactor the `Command::Meeting` arm**

Replace the current `Command::Meeting` arm (lines 54–63 of the original) with:

```rust
        Command::Meeting(name) => {
            let ord = state.doc.add_meeting(&state.current_time_hhmm(), &name);
            state.context = Context::Meeting(ord);
            state.update_context_display();
            after_doc_mutation(state)?;
        }
```

- [ ] **Step 5: Refactor the `Command::Note` arm**

Replace the current `Command::Note` arm (lines 64–79 of the original) with:

```rust
        Command::Note(name) => {
            if let Some(n) = name {
                let ord = state.doc.add_note_heading(&n);
                state.context = Context::NoteBlock(ord);
                state.update_context_display();
                after_doc_mutation(state)?;
            } else {
                state.context = Context::Notes;
                state.update_context_display();
                state.status.clear();
            }
        }
```

- [ ] **Step 6: Refactor the `Command::Todo` arm**

Replace the current `Command::Todo` arm (lines 80–94 of the original) with:

```rust
        Command::Todo(text) => {
            let meeting_name = match &state.context {
                Context::Meeting(ord) => state.doc.meetings().get(*ord).map(|m| m.name.clone()),
                Context::NoteBlock(ord) => {
                    state.doc.note_headings().get(*ord).map(|n| n.name.clone())
                }
                _ => None,
            };
            state.doc.add_todo(&text, meeting_name.as_deref());
            after_doc_mutation(state)?;
        }
```

- [ ] **Step 7: Run the full test suite**

```bash
cargo test
```

Expected: all 182 tests pass, zero failures. If any test fails, the refactor introduced a behavior change — do not proceed; compare the failed arm against the original and fix.

- [ ] **Step 8: Verify it compiles without warnings**

```bash
cargo build
```

Expected: compiles cleanly with no warnings.

- [ ] **Step 9: Commit**

```bash
git add src/app/actions.rs
git commit -m "refactor: extract after_doc_mutation helper to eliminate dispatch DRY violation"
```
