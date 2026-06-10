# Note Panel Scroll Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the document panel so it keeps the current context section or last-inserted line visible in Capture mode instead of always showing the top of the document.

**Architecture:** Add a single `doc_anchor_line: usize` field to `AppState`. Input handlers update this field to track either the vim cursor (in vim modes) or the relevant content line (in Capture mode). The render formula in `document.rs` uses the anchor for both modes, eliminating the hard-coded `0` that causes the snap-to-top bug.

**Tech Stack:** Rust, Ratatui 0.30, crossterm 0.29

---

## File Map

| File | Change |
|------|--------|
| `src/app/state.rs` | Add `doc_anchor_line: usize` field; initialize to `0` in `open_day` |
| `src/app/context.rs` | Add `context_heading_line()`, `find_heading()`, `find_nth_l3_heading_in_section()` |
| `src/app/actions.rs` | Add `state.doc_anchor_line = state.vim.cursor_line` to `vim_update_context`; update `vim_jump_to_new_content` to set anchor in Capture mode |
| `src/app/input/mod.rs` | Set anchor to context heading on `SwitchToCapture` action |
| `src/ui/document.rs` | Replace hard-coded `0` scroll with `doc_anchor_line.saturating_sub(3)` in else branch |

---

## Task 1: Add `doc_anchor_line` field to `AppState`

**Files:**
- Modify: `src/app/state.rs`

- [ ] **Step 1: Write the failing test**

Add this test to the `#[cfg(test)]` module at the bottom of `src/app/state.rs`:

```rust
#[test]
fn doc_anchor_line_initializes_to_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::open_day(
        tmp.path().to_path_buf(),
        Config::default(),
        NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
    )
    .unwrap();
    assert_eq!(state.doc_anchor_line, 0);
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p buff doc_anchor_line_initializes_to_zero
```

Expected: compile error — field `doc_anchor_line` does not exist

- [ ] **Step 3: Add the field and initialize it**

In `src/app/state.rs`, add after `right_panel_scroll` (line 91):

```rust
pub doc_anchor_line: usize,
```

In the `open_day` constructor, after `right_panel_scroll: 0,` (around line 132):

```rust
doc_anchor_line: 0,
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test -p buff doc_anchor_line_initializes_to_zero
```

Expected: `test doc_anchor_line_initializes_to_zero ... ok`

- [ ] **Step 5: Run the full test suite to catch regressions**

```bash
cargo test
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/app/state.rs
git commit -m "feat: add doc_anchor_line field to AppState"
```

---

## Task 2: Add `context_heading_line` utility to `context.rs`

**Files:**
- Modify: `src/app/context.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)]` module at the bottom of `src/app/context.rs`:

```rust
#[test]
fn context_heading_line_notes_returns_notes_heading() {
    let doc = lines("## Meetings\n\n## Notes\nstuff\n\n## To-dos\n");
    assert_eq!(context_heading_line(&doc, &Context::Notes), 2);
}

#[test]
fn context_heading_line_todos_returns_todos_heading() {
    let doc = lines("## Meetings\n\n## Notes\n\n## To-dos\n");
    assert_eq!(context_heading_line(&doc, &Context::Todos), 4);
}

#[test]
fn context_heading_line_meeting_zero_returns_first_meeting() {
    let doc = lines("## Meetings\n### Standup\ncontent\n### Planning\ncontent\n");
    assert_eq!(context_heading_line(&doc, &Context::Meeting(0)), 1);
}

#[test]
fn context_heading_line_meeting_one_returns_second_meeting() {
    let doc = lines("## Meetings\n### Standup\ncontent\n### Planning\ncontent\n");
    assert_eq!(context_heading_line(&doc, &Context::Meeting(1)), 3);
}

#[test]
fn context_heading_line_note_block_zero_returns_first_note() {
    let doc = lines("## Notes\n### My Note\ncontent\n");
    assert_eq!(context_heading_line(&doc, &Context::NoteBlock(0)), 1);
}

#[test]
fn context_heading_line_section_returns_heading_line_directly() {
    let doc = lines("## Meetings\n### Standup\n#### Phase 1\ncontent\n");
    assert_eq!(
        context_heading_line(&doc, &Context::Section { heading_line: 2, level: 4 }),
        2,
    );
}

#[test]
fn context_heading_line_missing_heading_returns_zero() {
    let doc = lines("## Meetings\n");
    assert_eq!(context_heading_line(&doc, &Context::Notes), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p buff context_heading_line
```

Expected: compile errors — `context_heading_line` not defined

- [ ] **Step 3: Add the implementation**

Add these functions to `src/app/context.rs` before the `#[cfg(test)]` section:

```rust
/// Find the line index of the first line in `lines` that exactly equals `heading`.
/// Returns `None` if not found.
fn find_heading(lines: &[String], heading: &str) -> Option<usize> {
    lines.iter().position(|l| l == heading)
}

/// Find the line index of the Nth (0-based) `### ` heading that appears after
/// the given `##` section heading. Stops scanning at the next `## ` heading.
/// Returns `None` if the section or Nth heading is not found.
fn find_nth_l3_heading_in_section(
    lines: &[String],
    section: &str,
    n: usize,
) -> Option<usize> {
    let start = find_heading(lines, section)?;
    let mut count = 0usize;
    for (offset, line) in lines[start + 1..].iter().enumerate() {
        if line.starts_with("## ") {
            break; // entered a different section
        }
        if line.starts_with("### ") {
            if count == n {
                return Some(start + 1 + offset);
            }
            count += 1;
        }
    }
    None
}

/// Returns the line index in `lines` of the heading that corresponds to `context`.
/// Used to compute the Capture-mode scroll anchor when entering Capture from VimNormal.
/// Falls back to `0` (top of document) if the heading cannot be located.
pub fn context_heading_line(lines: &[String], context: &Context) -> usize {
    match context {
        Context::Section { heading_line, .. } => *heading_line,
        Context::Notes => find_heading(lines, "## Notes").unwrap_or(0),
        Context::Todos => find_heading(lines, "## To-dos").unwrap_or(0),
        Context::Meeting(n) => {
            find_nth_l3_heading_in_section(lines, "## Meetings", *n).unwrap_or(0)
        }
        Context::NoteBlock(n) => {
            find_nth_l3_heading_in_section(lines, "## Notes", *n).unwrap_or(0)
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p buff context_heading_line
```

Expected: all 7 `context_heading_line` tests pass

- [ ] **Step 5: Run full test suite**

```bash
cargo test
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/app/context.rs
git commit -m "feat: add context_heading_line utility"
```

---

## Task 3: Sync anchor with vim cursor via `vim_update_context`

**Files:**
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` module in `src/app/actions.rs`:

```rust
#[test]
fn vim_update_context_sets_doc_anchor_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.doc.lines = vec![
        "# Day".to_string(),
        String::new(),
        "## Notes".to_string(),
        "line one".to_string(),
        "line two".to_string(),
    ];
    state.vim.cursor_line = 4;
    vim_update_context(&mut state);
    assert_eq!(state.doc_anchor_line, 4);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p buff vim_update_context_sets_doc_anchor_line
```

Expected: FAIL — `assert_eq!(0, 4)`

- [ ] **Step 3: Add the anchor sync**

In `src/app/actions.rs`, update `vim_update_context` (currently at line 32–36):

```rust
pub fn vim_update_context(state: &mut AppState) {
    use crate::app::context::context_at_line;
    state.doc_anchor_line = state.vim.cursor_line;  // keep anchor in sync
    state.context = context_at_line(&state.doc.lines, state.vim.cursor_line);
    state.update_context_display();
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p buff vim_update_context_sets_doc_anchor_line
```

Expected: `test vim_update_context_sets_doc_anchor_line ... ok`

- [ ] **Step 5: Run full test suite**

```bash
cargo test
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat: sync doc_anchor_line with vim cursor in vim_update_context"
```

---

## Task 4: Set anchor to context heading on VimNormal→Capture transition

**Files:**
- Modify: `src/app/input/mod.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` module in `src/app/input/mod.rs`:

```rust
#[test]
fn switch_to_capture_sets_anchor_to_context_heading() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    // Set up document with a Notes section not at line 0
    state.doc.lines = vec![
        "# Day".to_string(),
        String::new(),
        "## Meetings".to_string(),
        String::new(),
        "## Notes".to_string(),
        "a note".to_string(),
        String::new(),
        "## To-dos".to_string(),
    ];
    state.context = crate::app::state::Context::Notes;
    state.focus = Focus::VimNormal;
    state.vim.cursor_line = 5; // inside ## Notes
    state.doc_anchor_line = 5; // synced by vim_update_context

    execute_action(&mut state, UiAction::SwitchToCapture).unwrap();

    assert_eq!(state.focus, Focus::Capture);
    // Anchor should jump to "## Notes" heading at line 4, not stay at 5
    assert_eq!(state.doc_anchor_line, 4);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p buff switch_to_capture_sets_anchor_to_context_heading
```

Expected: FAIL — `assert_eq!(5, 4)`

- [ ] **Step 3: Update `SwitchToCapture` handler**

In `src/app/input/mod.rs`, find the `SwitchToCapture` arm of `execute_action` (near line 333). Replace it:

```rust
UiAction::SwitchToCapture => {
    state.focus = Focus::Capture;
    state.doc_anchor_line =
        crate::app::context::context_heading_line(&state.doc.lines, &state.context);
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p buff switch_to_capture_sets_anchor_to_context_heading
```

Expected: `test switch_to_capture_sets_anchor_to_context_heading ... ok`

- [ ] **Step 5: Run full test suite**

```bash
cargo test
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/app/input/mod.rs
git commit -m "feat: set doc_anchor_line to context heading on vim->capture transition"
```

---

## Task 5: Set anchor to inserted line after Capture submission

**Files:**
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` module in `src/app/actions.rs`:

```rust
#[test]
fn vim_jump_to_new_content_sets_anchor_in_capture_mode() {
    use crate::app::state::Focus;
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    // Start in Capture mode (the default)
    assert_eq!(state.focus, Focus::Capture);
    // Add some content so last non-empty line is not 0
    dispatch(&mut state, Command::Entry("first note".to_string())).unwrap();
    dispatch(&mut state, Command::Entry("second note".to_string())).unwrap();
    // doc_anchor_line should now point at the last non-empty line
    let last_nonempty = state
        .doc
        .lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .unwrap();
    assert_eq!(
        state.doc_anchor_line,
        last_nonempty,
        "anchor should point to last inserted line; lines: {:?}",
        state.doc.lines
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p buff vim_jump_to_new_content_sets_anchor_in_capture_mode
```

Expected: FAIL — anchor is 0, not the last non-empty line

- [ ] **Step 3: Update `vim_jump_to_new_content`**

In `src/app/actions.rs`, replace the entire `vim_jump_to_new_content` function (lines 61–71):

```rust
pub fn vim_jump_to_new_content(state: &mut AppState) {
    if let Some(idx) = state.doc.lines.iter().rposition(|l| !l.trim().is_empty()) {
        if matches!(
            state.focus,
            crate::app::state::Focus::VimNormal | crate::app::state::Focus::VimInsert
        ) {
            state.vim.cursor_line = idx;
            state.vim.cursor_col = 0;
            vim_update_context(state); // also sets doc_anchor_line = idx
        } else {
            // Capture mode: update anchor so document scrolls to new content
            state.doc_anchor_line = idx;
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p buff vim_jump_to_new_content_sets_anchor_in_capture_mode
```

Expected: `test vim_jump_to_new_content_sets_anchor_in_capture_mode ... ok`

- [ ] **Step 5: Run full test suite**

```bash
cargo test
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat: set doc_anchor_line after capture insertion"
```

---

## Task 6: Update document render to use anchor instead of hard-coded `0`

**Files:**
- Modify: `src/ui/document.rs`

This task has no unit test — the render function takes `&AppState` and writes directly to a terminal Frame. Correctness is verified visually by running the app and checking that:
1. Typing notes in Capture mode keeps the last-inserted note visible
2. Pressing Esc from a scrolled position in VimNormal doesn't snap back to the top

- [ ] **Step 1: Replace the scroll computation in `document.rs`**

In `src/ui/document.rs`, find lines 142–148:

```rust
    // Scroll: follow cursor in vim mode, else 0
    let scroll_offset: usize = if vim_active {
        let visible_height = area.height as usize;
        cursor_line.saturating_sub(visible_height.saturating_sub(1))
    } else {
        0
    };
```

Replace with:

```rust
    // Scroll: use doc_anchor_line for both vim and capture modes.
    // Vim mode: anchor follows cursor; keeps cursor near bottom of viewport.
    // Capture mode: anchor is context heading or last inserted line; shown near top.
    let doc_anchor = app.doc_anchor_line;
    let visible_height = area.height as usize;
    let scroll_offset: usize = if vim_active {
        doc_anchor.saturating_sub(visible_height.saturating_sub(1))
    } else {
        doc_anchor.saturating_sub(3)
    };
```

Note: `cursor_line` is already computed at the top of `render()` (line 128 in the original). The variable name change from `cursor_line` → `doc_anchor` in this formula is intentional — in vim mode they are equal (both track `vim.cursor_line` via `vim_update_context`), so the scroll behavior is unchanged. In Capture mode `doc_anchor` is now the section heading or last insertion line instead of `0`.

- [ ] **Step 2: Build to verify it compiles**

```bash
cargo build
```

Expected: compiles without errors or warnings

- [ ] **Step 3: Run the full test suite**

```bash
cargo test
```

Expected: all tests pass

- [ ] **Step 4: Smoke-test manually**

Run `cargo run` and verify:
- Type a note in the capture bar → the document panel scrolls to show the new line
- Navigate in VimNormal to a section near the bottom, then press Esc → document stays in place (anchor at context heading) rather than snapping to top
- j/k navigation in VimNormal still scrolls the cursor into view

- [ ] **Step 5: Commit**

```bash
git add src/ui/document.rs
git commit -m "fix: use doc_anchor_line in document render; capture mode no longer snaps to top"
```

---

## Self-Review Checklist

- [x] **Spec coverage:** All three spec goals are addressed: (1) context heading visible on Capture entry — Task 4; (2) last insertion visible in Capture — Task 5; (3) vim cursor follow unchanged — Tasks 3 + 6
- [x] **No placeholders:** All code blocks are complete and compilable
- [x] **Type consistency:** `doc_anchor_line: usize` used consistently across all tasks; `context_heading_line(&[String], &Context) -> usize` signature matches all call sites
- [x] **Scope:** Single coherent feature; no decomposition needed
