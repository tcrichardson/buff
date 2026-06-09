# VimNormal Enter Key — Edit Selected Line Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pressing `Enter` in VimNormal mode loads the raw markdown text of the line under the vim cursor into the capture box for editing.

**Architecture:** A new `UiAction::VimBeginEditLine` variant is mapped to `Enter` in `vim_normal::key_to_action`. Its handler finds the `Selectable` whose `lines` range contains `vim.cursor_line`, populates `state.input` with the raw markdown text, sets `state.editing`, and switches focus to `Focus::Capture`. The existing `CommitEdit` action handles the commit path unchanged.

**Tech Stack:** Rust, ratatui, crossterm. No new dependencies.

---

### Task 1: Add `VimBeginEditLine` to `UiAction` and wire the key binding

**Files:**
- Modify: `src/app/input/mod.rs` (enum, key dispatch)
- Modify: `src/app/input/vim_normal.rs` (key binding)

- [ ] **Step 1: Write the failing test**

  Add to the `#[cfg(test)]` block in `src/app/input/mod.rs` (after the last test, before the closing `}`):

  ```rust
  #[test]
  fn vimnormal_enter_emits_begin_edit_line() {
      let tmp = tempfile::tempdir().unwrap();
      let mut state = test_state(&tmp);
      state.focus = Focus::VimNormal;
      assert_eq!(
          key_to_action(&state, make_key(KeyCode::Enter)),
          Some(UiAction::VimBeginEditLine)
      );
  }
  ```

- [ ] **Step 2: Run the test to confirm it fails**

  ```
  cargo test vimnormal_enter_emits_begin_edit_line
  ```

  Expected: FAIL — `VimBeginEditLine` does not exist yet.

- [ ] **Step 3: Add the `VimBeginEditLine` variant to `UiAction`**

  In `src/app/input/mod.rs`, add the variant after `VimToggleTodo` (line 88):

  ```rust
      VimToggleTodo,
      VimBeginEditLine,
  ```

- [ ] **Step 4: Map `Enter` to `VimBeginEditLine` in vim_normal**

  In `src/app/input/vim_normal.rs`, add one line before `KeyCode::Esc` (line 44):

  ```rust
          KeyCode::Enter     => Some(UiAction::VimBeginEditLine),
          KeyCode::Esc       => None,
  ```

- [ ] **Step 5: Add a stub dispatch arm to silence the compiler**

  In `src/app/input/mod.rs`, `execute_action`, add `VimBeginEditLine` to the existing VimNormal arm. Find this block (around line 339) and add the new variant:

  ```rust
          UiAction::VimMoveLeft
          | UiAction::VimMoveRight
          | UiAction::VimMoveUp
          | UiAction::VimMoveDown
          | UiAction::VimMoveLineStart
          | UiAction::VimMoveLineEnd
          | UiAction::VimMoveFileStart
          | UiAction::VimMoveFileEnd
          | UiAction::VimMoveWordForward
          | UiAction::VimMoveWordBackward
          | UiAction::VimMoveWordEnd
          | UiAction::VimSetPendingOp(_)
          | UiAction::VimClearPendingOp
          | UiAction::VimEnterInsert
          | UiAction::VimEnterInsertAfter
          | UiAction::VimEnterInsertEOL
          | UiAction::VimInsertLineBelow
          | UiAction::VimInsertLineAbove
          | UiAction::VimDeleteChar
          | UiAction::VimDeleteLine
          | UiAction::VimYankLine
          | UiAction::VimPasteBelow
          | UiAction::VimPasteAbove
          | UiAction::VimUndo
          | UiAction::VimToggleTodo
          | UiAction::VimBeginEditLine => return vim_normal::execute_action(state, action),
  ```

  Also add a stub arm in `vim_normal::execute_action` in `src/app/input/vim_normal.rs` to handle the new variant without panicking. Change the `_ => unreachable!(...)` pattern — first add the arm before it:

  ```rust
          UiAction::VimToggleTodo       => toggle_todo(state),
          UiAction::VimBeginEditLine    => { /* TODO: implement in Task 2 */ }
          _ => unreachable!("vim_normal::execute_action called with non-vim-normal action: {:?}", action),
  ```

- [ ] **Step 6: Run the test to confirm it passes**

  ```
  cargo test vimnormal_enter_emits_begin_edit_line
  ```

  Expected: PASS.

- [ ] **Step 7: Run the full test suite**

  ```
  cargo test
  ```

  Expected: all tests pass.

- [ ] **Step 8: Commit**

  ```bash
  git add src/app/input/mod.rs src/app/input/vim_normal.rs
  git commit -m "feat: add VimBeginEditLine action and Enter key binding in VimNormal"
  ```

---

### Task 2: Implement the `vim_begin_edit_line` handler

**Files:**
- Modify: `src/app/actions.rs` (new function)
- Modify: `src/app/input/vim_normal.rs` (replace stub with real call)

- [ ] **Step 1: Write failing tests**

  In `src/app/actions.rs`, in the existing `#[cfg(test)]` block (find the `mod tests {` near the bottom), add:

  ```rust
  #[test]
  fn vim_begin_edit_line_on_selectable_populates_input_and_editing() {
      let tmp = tempfile::tempdir().unwrap();
      let mut state = test_state(&tmp);
      state.focus = Focus::VimNormal;
      state.doc.lines = vec![
          "# Day".to_string(),
          String::new(),
          "## Notes".to_string(),
          "- a bullet".to_string(),
      ];
      state.selectables = state.doc.selectables();
      state.vim.cursor_line = 3; // on "- a bullet"

      vim_begin_edit_line(&mut state).unwrap();

      assert_eq!(state.input, "- a bullet");
      assert!(state.editing.is_some());
      assert_eq!(state.cursor_pos, "- a bullet".len());
      assert_eq!(state.focus, Focus::Capture);
  }

  #[test]
  fn vim_begin_edit_line_on_empty_line_is_noop() {
      let tmp = tempfile::tempdir().unwrap();
      let mut state = test_state(&tmp);
      state.focus = Focus::VimNormal;
      state.doc.lines = vec![
          "# Day".to_string(),
          String::new(),
      ];
      state.selectables = state.doc.selectables();
      state.vim.cursor_line = 1; // empty line — not a selectable

      vim_begin_edit_line(&mut state).unwrap();

      assert!(state.input.is_empty());
      assert!(state.editing.is_none());
      assert_eq!(state.focus, Focus::VimNormal);
  }
  ```

  Note: the `test_state` helper and `use super::*;` are already in that module. You also need `Focus` in scope — check if it's already imported. If not, add `use crate::app::state::Focus;` at the top of the `mod tests` block. (It's likely already present via `use super::*`.)

- [ ] **Step 2: Run the tests to confirm they fail**

  ```
  cargo test vim_begin_edit_line
  ```

  Expected: FAIL — `vim_begin_edit_line` is not defined.

- [ ] **Step 3: Implement `vim_begin_edit_line` in `src/app/actions.rs`**

  Add the following function. A good place is right before `commit_edit` (around line 445):

  ```rust
  pub fn vim_begin_edit_line(state: &mut AppState) -> anyhow::Result<()> {
      let cursor_line = state.vim.cursor_line;
      let found = state
          .selectables
          .iter()
          .enumerate()
          .find(|(_, sel)| sel.lines.contains(&cursor_line));
      if let Some((idx, sel)) = found {
          let text = state.doc.lines[sel.lines.clone()].join("\n");
          state.input = text;
          state.editing = Some(idx);
          state.cursor_pos = state.input.len();
          state.focus = crate::app::state::Focus::Capture;
      }
      Ok(())
  }
  ```

- [ ] **Step 4: Replace the stub in `vim_normal::execute_action`**

  In `src/app/input/vim_normal.rs`, replace:

  ```rust
          UiAction::VimBeginEditLine    => { /* TODO: implement in Task 2 */ }
  ```

  With:

  ```rust
          UiAction::VimBeginEditLine    => { crate::app::actions::vim_begin_edit_line(state)?; }
  ```

  Note: `execute_action` currently returns `Ok(EventOutcome::Continue)` after the match — the `?` propagates any error correctly.

  However, `execute_action` in `vim_normal.rs` currently returns `Ok(EventOutcome::Continue)` at the end, not from within each arm. The match arms that call functions like `delete_line(state)` don't return values. To handle `vim_begin_edit_line` which returns `Result`, you need to call it with `?`. Change the function signature handling: add the `?` as shown above — the outer `Ok(EventOutcome::Continue)` at line 79 still runs after the match.

- [ ] **Step 5: Run the failing tests to confirm they now pass**

  ```
  cargo test vim_begin_edit_line
  ```

  Expected: PASS.

- [ ] **Step 6: Run the full test suite**

  ```
  cargo test
  ```

  Expected: all tests pass.

- [ ] **Step 7: Commit**

  ```bash
  git add src/app/actions.rs src/app/input/vim_normal.rs
  git commit -m "feat: implement vim_begin_edit_line — Enter in VimNormal opens line for editing"
  ```

---

## Verification

After both tasks are complete, do a quick manual smoke test:

1. Run `cargo run`
2. Add a note (e.g., `- a test bullet`) in the capture box
3. Press `Esc` to enter VimNormal
4. Navigate to the bullet line with `j`/`k`
5. Press `Enter` — the capture box should show `Edit: › - a test bullet` with cursor at end
6. Edit the text and press `Enter` — the bullet should update in the document
7. Press `Esc` then `Enter` on the same line — changes should be discarded
