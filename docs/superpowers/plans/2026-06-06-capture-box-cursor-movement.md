# Capture Box Cursor Movement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add free cursor movement within the capture box so the user can edit text mid-line, with left/right arrow movement, Backspace at cursor, insert-at-cursor typing, and Home/End (Ctrl+A / Ctrl+E) line jumps.

**Architecture:** Add a `cursor_pos: usize` (byte offset into `app.input`) to `AppState`. All mutation actions (`TypeChar`, `DeleteChar`, `TypeNewline`) operate at `cursor_pos` rather than always at the string end. Four new `UiAction` variants handle movement. `render_input` in `capture.rs` maps `cursor_pos` to a terminal (row, col) for cursor placement.

**Tech Stack:** Rust, Ratatui TUI, Crossterm event handling.

---

## Files Changed

| File | Change |
|---|---|
| `src/app/state.rs` | Add `cursor_pos: usize` field, initialize to `0` |
| `src/app/input.rs` | Add 4 `UiAction` variants; key mappings; UTF-8 helpers; update all 3 mutation arms + 4 movement arms; reset `cursor_pos` on clear |
| `src/app/actions.rs` | `begin_edit_selected` sets `cursor_pos = input.len()`; `commit_edit` resets `cursor_pos = 0` |
| `src/ui/capture.rs` | Replace end-of-last-line cursor calc with `cursor_pos`-aware row/col logic |

---

## Task 1: Add `cursor_pos` field to `AppState`

**Files:**
- Modify: `src/app/state.rs`
- Test: `src/app/input.rs` (existing test module)

- [ ] **Step 1: Write a failing test**

At the bottom of the `#[cfg(test)] mod tests` block in `src/app/input.rs`, add:

```rust
#[test]
fn cursor_pos_initializes_to_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let state = test_state(&tmp);
    assert_eq!(state.cursor_pos, 0);
}
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cargo test cursor_pos_initializes_to_zero
```

Expected: compile error — `no field 'cursor_pos' on type 'AppState'`

- [ ] **Step 3: Add the field to `AppState`**

In `src/app/state.rs`, add `cursor_pos` to the struct after `input`:

```rust
pub input: String,
pub cursor_pos: usize,  // byte offset into `input`; always <= input.len(), always on a char boundary
pub overlay: Overlay,
```

Then in `AppState::open_day`, add `cursor_pos: 0` to the struct literal after `input: String::new()`:

```rust
input: String::new(),
cursor_pos: 0,
overlay: Overlay::None,
```

- [ ] **Step 4: Run the test to confirm it passes**

```bash
cargo test cursor_pos_initializes_to_zero
```

Expected: `test app::input::tests::cursor_pos_initializes_to_zero ... ok`

- [ ] **Step 5: Run the full test suite to confirm no regressions**

```bash
cargo test
```

Expected: all 216 tests pass (215 + the new one).

- [ ] **Step 6: Commit**

```bash
git add src/app/state.rs src/app/input.rs
git commit -m "feat: add cursor_pos field to AppState"
```

---

## Task 2: Add UTF-8 boundary helper functions

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write failing tests for the helpers**

In the `#[cfg(test)] mod tests` block in `src/app/input.rs`, add:

```rust
#[test]
fn prev_char_boundary_steps_back_one_ascii() {
    assert_eq!(super::prev_char_boundary("hello", 3), 2);
}

#[test]
fn prev_char_boundary_at_zero_stays_zero() {
    assert_eq!(super::prev_char_boundary("hello", 0), 0);
}

#[test]
fn prev_char_boundary_steps_back_multibyte() {
    // "é" is U+00E9, encoded as 2 bytes: 0xC3 0xA9
    let s = "aé"; // bytes: [0x61, 0xC3, 0xA9]
    assert_eq!(super::prev_char_boundary(s, 3), 1); // from end back to start of 'é'
    assert_eq!(super::prev_char_boundary(s, 1), 0); // from 'é' back to 'a'
}

#[test]
fn next_char_boundary_steps_forward_one_ascii() {
    assert_eq!(super::next_char_boundary("hello", 1), 2);
}

#[test]
fn next_char_boundary_at_end_stays_end() {
    assert_eq!(super::next_char_boundary("hello", 5), 5);
}

#[test]
fn next_char_boundary_steps_forward_multibyte() {
    // "aé" bytes: [0x61, 0xC3, 0xA9]
    let s = "aé";
    assert_eq!(super::next_char_boundary(s, 0), 1); // 'a' → start of 'é'
    assert_eq!(super::next_char_boundary(s, 1), 3); // start of 'é' → past 'é' = end
}
```

- [ ] **Step 2: Confirm tests fail**

```bash
cargo test prev_char_boundary next_char_boundary
```

Expected: compile errors — `super::prev_char_boundary` and `super::next_char_boundary` not found.

- [ ] **Step 3: Implement the helpers in `src/app/input.rs`**

Add these two functions at module scope (outside any `impl` block, before `pub fn key_to_action`):

```rust
/// Step back one Unicode scalar from `pos`. Returns 0 if already at start.
fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos - 1;
    while !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Step forward one Unicode scalar from `pos`. Returns `s.len()` if already at end.
fn next_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos + 1;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    p
}
```

- [ ] **Step 4: Run the new tests**

```bash
cargo test prev_char_boundary next_char_boundary
```

Expected: all 6 new tests pass.

- [ ] **Step 5: Run the full suite**

```bash
cargo test
```

Expected: all 222 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: add UTF-8 char boundary helper functions"
```

---

## Task 3: Add movement `UiAction` variants, key mappings, and stub handlers

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write failing tests for the key mappings**

Add these tests to the `#[cfg(test)] mod tests` block in `src/app/input.rs`:

```rust
#[test]
fn capture_left_arrow_moves_cursor_left() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Left)),
        Some(UiAction::MoveCursorLeft)
    );
}

#[test]
fn capture_right_arrow_moves_cursor_right() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Right)),
        Some(UiAction::MoveCursorRight)
    );
}

#[test]
fn capture_home_moves_to_line_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::Home)),
        Some(UiAction::MoveCursorLineStart)
    );
}

#[test]
fn capture_end_moves_to_line_end() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    assert_eq!(
        key_to_action(&state, make_key(KeyCode::End)),
        Some(UiAction::MoveCursorLineEnd)
    );
}

#[test]
fn capture_ctrl_a_moves_to_line_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    assert_eq!(
        key_to_action(&state, ctrl(KeyCode::Char('a'))),
        Some(UiAction::MoveCursorLineStart)
    );
}

#[test]
fn capture_ctrl_e_moves_to_line_end() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    assert_eq!(
        key_to_action(&state, ctrl(KeyCode::Char('e'))),
        Some(UiAction::MoveCursorLineEnd)
    );
}
```

- [ ] **Step 2: Confirm tests fail**

```bash
cargo test capture_left_arrow capture_right_arrow capture_home capture_end capture_ctrl_a capture_ctrl_e
```

Expected: compile errors — `UiAction::MoveCursorLeft` etc. don't exist yet.

- [ ] **Step 3: Add the new `UiAction` variants**

In `src/app/input.rs`, in the `UiAction` enum, add after the existing `CommitEdit` line:

```rust
// Capture mode — cursor movement
MoveCursorLeft,
MoveCursorRight,
MoveCursorLineStart,
MoveCursorLineEnd,
```

- [ ] **Step 4: Add key mappings to `key_to_action`**

In `src/app/input.rs`, in the `Focus::Capture` match arm of `key_to_action`, replace:

```rust
        KeyCode::Up | KeyCode::Down => None, // ignored in capture mode
        _ => None,
```

with:

```rust
        KeyCode::Left => Some(UiAction::MoveCursorLeft),
        KeyCode::Right => Some(UiAction::MoveCursorRight),
        KeyCode::Home => Some(UiAction::MoveCursorLineStart),
        KeyCode::End => Some(UiAction::MoveCursorLineEnd),
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::MoveCursorLineStart)
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UiAction::MoveCursorLineEnd)
        }
        KeyCode::Up | KeyCode::Down => None, // ignored in capture mode
        _ => None,
```

- [ ] **Step 5: Add no-op stub arms to `execute_action`**

Rust `match` must be exhaustive. In `execute_action`, find the comment `// Navigate mode` section. Add these stubs before that section (still inside the outer `match action` — after the last Capture mode arm `UiAction::CommitEdit`):

```rust
        // Capture mode — cursor movement (implemented in a later task)
        UiAction::MoveCursorLeft => {}
        UiAction::MoveCursorRight => {}
        UiAction::MoveCursorLineStart => {}
        UiAction::MoveCursorLineEnd => {}
```

- [ ] **Step 6: Run the key mapping tests**

```bash
cargo test capture_left_arrow capture_right_arrow capture_home capture_end capture_ctrl_a capture_ctrl_e
```

Expected: all 6 pass.

- [ ] **Step 7: Run the full suite**

```bash
cargo test
```

Expected: all 228 tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: add cursor movement UiAction variants and key mappings"
```

---

## Task 4: Make `TypeChar` insert at `cursor_pos`

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write a failing test**

Add to the test module in `src/app/input.rs`:

```rust
#[test]
fn type_char_inserts_at_cursor_pos() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "ac".to_string();
    state.cursor_pos = 1; // between 'a' and 'c'
    execute_action(&mut state, UiAction::TypeChar('b')).unwrap();
    assert_eq!(state.input, "abc");
    assert_eq!(state.cursor_pos, 2);
}
```

- [ ] **Step 2: Confirm test fails**

```bash
cargo test type_char_inserts_at_cursor_pos
```

Expected: FAIL — inserts at end → `"acb"` instead of `"abc"`.

- [ ] **Step 3: Update `TypeChar` in `execute_action`**

Replace:

```rust
        UiAction::TypeChar(c) => {
            state.input.push(c);
        }
```

with:

```rust
        UiAction::TypeChar(c) => {
            state.input.insert(state.cursor_pos, c);
            state.cursor_pos += c.len_utf8();
        }
```

- [ ] **Step 4: Run the new test**

```bash
cargo test type_char_inserts_at_cursor_pos
```

Expected: `ok`

- [ ] **Step 5: Run the full suite (existing `TypeChar` tests must still pass)**

```bash
cargo test
```

Expected: all 229 tests pass.

Note: existing tests like `type_char_appends_to_input` still pass because `cursor_pos` starts at `0` and advances with each character — the end result is the same as before.

- [ ] **Step 6: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: TypeChar inserts at cursor_pos"
```

---

## Task 5: Make `DeleteChar` remove the character before `cursor_pos`

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Update the existing test that directly sets `state.input`**

Find the test `delete_char_pops_last_char` in `src/app/input.rs`. It sets `state.input = "ab"` but leaves `cursor_pos` at 0. After this change, Backspace at position 0 is a no-op. Update the test to also position the cursor at the end:

Replace the existing test body:

```rust
fn delete_char_pops_last_char() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "ab".to_string();
    execute_action(&mut state, UiAction::DeleteChar).unwrap();
    assert_eq!(state.input, "a");
}
```

with:

```rust
fn delete_char_pops_last_char() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "ab".to_string();
    state.cursor_pos = 2; // cursor at end
    execute_action(&mut state, UiAction::DeleteChar).unwrap();
    assert_eq!(state.input, "a");
    assert_eq!(state.cursor_pos, 1);
}
```

- [ ] **Step 2: Write a new failing test — delete mid-string**

Add to the test module:

```rust
#[test]
fn delete_char_removes_char_before_cursor() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc".to_string();
    state.cursor_pos = 2; // between 'b' and 'c'
    execute_action(&mut state, UiAction::DeleteChar).unwrap();
    assert_eq!(state.input, "ac");
    assert_eq!(state.cursor_pos, 1);
}

#[test]
fn delete_char_at_start_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc".to_string();
    state.cursor_pos = 0;
    execute_action(&mut state, UiAction::DeleteChar).unwrap();
    assert_eq!(state.input, "abc");
    assert_eq!(state.cursor_pos, 0);
}
```

- [ ] **Step 3: Confirm the updated test and new tests fail**

```bash
cargo test delete_char
```

Expected: `delete_char_removes_char_before_cursor` fails (still uses old `pop()` logic), `delete_char_at_start_is_noop` fails.

- [ ] **Step 4: Update `DeleteChar` in `execute_action`**

Replace:

```rust
        UiAction::DeleteChar => {
            state.input.pop();
        }
```

with:

```rust
        UiAction::DeleteChar => {
            if state.cursor_pos > 0 {
                let prev = prev_char_boundary(&state.input, state.cursor_pos);
                state.input.remove(prev);
                state.cursor_pos = prev;
            }
        }
```

- [ ] **Step 5: Run all `delete_char` tests**

```bash
cargo test delete_char
```

Expected: all 4 `delete_char` tests pass.

- [ ] **Step 6: Run the full suite**

```bash
cargo test
```

Expected: all 231 tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: DeleteChar removes char before cursor_pos"
```

---

## Task 6: Make `TypeNewline` insert at `cursor_pos`

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write a failing test**

Add to the test module:

```rust
#[test]
fn type_newline_inserts_at_cursor_pos() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "ab".to_string();
    state.cursor_pos = 1; // between 'a' and 'b'
    execute_action(&mut state, UiAction::TypeNewline).unwrap();
    assert_eq!(state.input, "a\nb");
    assert_eq!(state.cursor_pos, 2);
}
```

- [ ] **Step 2: Confirm test fails**

```bash
cargo test type_newline_inserts_at_cursor_pos
```

Expected: FAIL — currently appends `\n` at end → `"ab\n"`.

- [ ] **Step 3: Update `TypeNewline` in `execute_action`**

Replace:

```rust
        UiAction::TypeNewline => {
            state.input.push('\n');
        }
```

with:

```rust
        UiAction::TypeNewline => {
            state.input.insert(state.cursor_pos, '\n');
            state.cursor_pos += 1;
        }
```

- [ ] **Step 4: Run new and existing newline tests**

```bash
cargo test newline
```

Expected: both `type_newline_pushes_newline_char` and `type_newline_inserts_at_cursor_pos` pass.

Note: `type_newline_pushes_newline_char` still passes because when input is empty, `cursor_pos` is 0, and inserting `\n` at 0 in `""` gives `"\n"`.

- [ ] **Step 5: Run the full suite**

```bash
cargo test
```

Expected: all 232 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: TypeNewline inserts at cursor_pos"
```

---

## Task 7: Implement `MoveCursorLeft` and `MoveCursorRight`

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write failing tests**

Add to the test module:

```rust
#[test]
fn move_cursor_left_decrements() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc".to_string();
    state.cursor_pos = 2;
    execute_action(&mut state, UiAction::MoveCursorLeft).unwrap();
    assert_eq!(state.cursor_pos, 1);
}

#[test]
fn move_cursor_left_clamps_at_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc".to_string();
    state.cursor_pos = 0;
    execute_action(&mut state, UiAction::MoveCursorLeft).unwrap();
    assert_eq!(state.cursor_pos, 0);
}

#[test]
fn move_cursor_right_increments() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc".to_string();
    state.cursor_pos = 1;
    execute_action(&mut state, UiAction::MoveCursorRight).unwrap();
    assert_eq!(state.cursor_pos, 2);
}

#[test]
fn move_cursor_right_clamps_at_end() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc".to_string();
    state.cursor_pos = 3;
    execute_action(&mut state, UiAction::MoveCursorRight).unwrap();
    assert_eq!(state.cursor_pos, 3);
}

#[test]
fn move_cursor_left_steps_over_multibyte_char() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "aé".to_string(); // bytes: [0x61, 0xC3, 0xA9] — len = 3
    state.cursor_pos = 3; // past 'é'
    execute_action(&mut state, UiAction::MoveCursorLeft).unwrap();
    assert_eq!(state.cursor_pos, 1); // back to start of 'é'
}

#[test]
fn move_cursor_right_steps_over_multibyte_char() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "aé".to_string();
    state.cursor_pos = 1; // at start of 'é'
    execute_action(&mut state, UiAction::MoveCursorRight).unwrap();
    assert_eq!(state.cursor_pos, 3); // past 'é'
}
```

- [ ] **Step 2: Confirm tests fail**

```bash
cargo test move_cursor_left move_cursor_right
```

Expected: FAIL — stubs are no-ops, `cursor_pos` never changes.

- [ ] **Step 3: Replace stubs for `MoveCursorLeft` and `MoveCursorRight`**

In `execute_action`, find the stub block added in Task 3. Replace the left/right stubs:

```rust
        UiAction::MoveCursorLeft => {
            state.cursor_pos = prev_char_boundary(&state.input, state.cursor_pos);
        }
        UiAction::MoveCursorRight => {
            state.cursor_pos = next_char_boundary(&state.input, state.cursor_pos);
        }
        UiAction::MoveCursorLineStart => {}
        UiAction::MoveCursorLineEnd => {}
```

- [ ] **Step 4: Run the new tests**

```bash
cargo test move_cursor_left move_cursor_right
```

Expected: all 6 tests pass.

- [ ] **Step 5: Run the full suite**

```bash
cargo test
```

Expected: all 238 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: implement MoveCursorLeft and MoveCursorRight"
```

---

## Task 8: Implement `MoveCursorLineStart` and `MoveCursorLineEnd`

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write failing tests**

Add to the test module:

```rust
#[test]
fn move_cursor_line_start_jumps_to_zero_when_no_newline() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "hello".to_string();
    state.cursor_pos = 3;
    execute_action(&mut state, UiAction::MoveCursorLineStart).unwrap();
    assert_eq!(state.cursor_pos, 0);
}

#[test]
fn move_cursor_line_start_jumps_past_newline() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc\ndefg".to_string();
    state.cursor_pos = 6; // at 'f' on second line
    execute_action(&mut state, UiAction::MoveCursorLineStart).unwrap();
    assert_eq!(state.cursor_pos, 4); // first char of second line ('d')
}

#[test]
fn move_cursor_line_start_at_bol_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc\ndefg".to_string();
    state.cursor_pos = 4; // already at start of second line
    execute_action(&mut state, UiAction::MoveCursorLineStart).unwrap();
    assert_eq!(state.cursor_pos, 4);
}

#[test]
fn move_cursor_line_end_jumps_to_end_when_no_newline() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "hello".to_string();
    state.cursor_pos = 2;
    execute_action(&mut state, UiAction::MoveCursorLineEnd).unwrap();
    assert_eq!(state.cursor_pos, 5);
}

#[test]
fn move_cursor_line_end_jumps_to_newline_position() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc\ndefg".to_string();
    state.cursor_pos = 1; // at 'b' on first line
    execute_action(&mut state, UiAction::MoveCursorLineEnd).unwrap();
    assert_eq!(state.cursor_pos, 3); // position of '\n' (= right after 'c')
}

#[test]
fn move_cursor_line_end_at_eol_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "abc\ndefg".to_string();
    state.cursor_pos = 3; // already at position of '\n' on first line
    execute_action(&mut state, UiAction::MoveCursorLineEnd).unwrap();
    assert_eq!(state.cursor_pos, 3);
}
```

- [ ] **Step 2: Confirm tests fail**

```bash
cargo test move_cursor_line_start move_cursor_line_end
```

Expected: FAIL — stubs are no-ops.

- [ ] **Step 3: Replace stubs for `MoveCursorLineStart` and `MoveCursorLineEnd`**

In `execute_action`, replace the remaining two stubs:

```rust
        UiAction::MoveCursorLineStart => {
            let before = &state.input[..state.cursor_pos];
            state.cursor_pos = match before.rfind('\n') {
                Some(nl_pos) => nl_pos + 1,
                None => 0,
            };
        }
        UiAction::MoveCursorLineEnd => {
            let after = &state.input[state.cursor_pos..];
            state.cursor_pos = match after.find('\n') {
                Some(nl_offset) => state.cursor_pos + nl_offset,
                None => state.input.len(),
            };
        }
```

- [ ] **Step 4: Run the new tests**

```bash
cargo test move_cursor_line_start move_cursor_line_end
```

Expected: all 6 tests pass.

- [ ] **Step 5: Run the full suite**

```bash
cargo test
```

Expected: all 244 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: implement MoveCursorLineStart and MoveCursorLineEnd"
```

---

## Task 9: Reset `cursor_pos` on input clear; set on begin-edit

**Files:**
- Modify: `src/app/input.rs`, `src/app/actions.rs`

- [ ] **Step 1: Write failing tests**

Add to the test module in `src/app/input.rs`:

```rust
#[test]
fn cursor_pos_reset_to_zero_on_submit() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "hello".to_string();
    state.cursor_pos = 3;
    execute_action(&mut state, UiAction::SubmitInput).unwrap();
    assert_eq!(state.cursor_pos, 0);
}

#[test]
fn cursor_pos_reset_to_zero_on_cancel_edit() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.editing = Some(0);
    state.input = "hello".to_string();
    state.cursor_pos = 3;
    execute_action(&mut state, UiAction::CancelEdit).unwrap();
    assert_eq!(state.cursor_pos, 0);
}
```

Add to the test module in `src/app/actions.rs` (inside the existing `#[cfg(test)] mod tests` block):

```rust
#[test]
fn cursor_pos_set_to_end_on_begin_edit() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Entry("idea".to_string())).unwrap();
    state.selected = 0;
    begin_edit_selected(&mut state);
    // "- idea" is 6 bytes
    assert_eq!(state.cursor_pos, state.input.len());
    assert_eq!(state.cursor_pos, 6);
}

#[test]
fn cursor_pos_reset_to_zero_on_commit_edit() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Entry("idea".to_string())).unwrap();
    state.selected = 0;
    begin_edit_selected(&mut state);
    state.cursor_pos = 3;
    commit_edit(&mut state).unwrap();
    assert_eq!(state.cursor_pos, 0);
}
```

- [ ] **Step 2: Confirm tests fail**

```bash
cargo test cursor_pos_reset cursor_pos_set_to_end
```

Expected: FAIL — `cursor_pos` is not yet reset/set in those code paths.

- [ ] **Step 3: Reset `cursor_pos` in `execute_action` for `SubmitInput` and `CancelEdit`**

In `src/app/input.rs`, update the `SubmitInput` arm:

```rust
        UiAction::SubmitInput => {
            let cmd = crate::app::command::parse(&state.input);
            crate::app::actions::dispatch(state, cmd)?;
            if state.overlay != Overlay::None {
                state.pending_delete = false;
            }
            state.input.clear();
            state.cursor_pos = 0;
        }
```

Update the `CancelEdit` arm:

```rust
        UiAction::CancelEdit => {
            state.editing = None;
            state.input.clear();
            state.cursor_pos = 0;
        }
```

- [ ] **Step 4: Set `cursor_pos` in `begin_edit_selected` and reset in `commit_edit`**

In `src/app/actions.rs`, update `begin_edit_selected`:

```rust
pub fn begin_edit_selected(state: &mut AppState) {
    if let Some(sel) = state.selectables.get(state.selected) {
        state.editing = Some(state.selected);
        state.input = sel.text.clone();
        state.cursor_pos = state.input.len();
        state.focus = crate::app::state::Focus::Capture;
    } else {
        state.status = "nothing selected".to_string();
    }
}
```

In `src/app/actions.rs`, update `commit_edit`:

```rust
pub fn commit_edit(state: &mut AppState) -> anyhow::Result<()> {
    if let Some(idx) = state.editing {
        let new_lines = crate::model::writer::format_entry(&state.input, None);
        state.doc.replace_selectable(idx, &new_lines)?;
        state.selectables = state.doc.selectables();
        state.editing = None;
        state.input.clear();
        state.cursor_pos = 0;
        state.focus = crate::app::state::Focus::Navigate;
        state.save()?;
        state.dates_with_notes =
            crate::storage::dates_with_notes(&state.notes_dir, &state.config.date_format);
        state.status.clear();
    }
    Ok(())
}
```

- [ ] **Step 5: Run the new tests**

```bash
cargo test cursor_pos_reset cursor_pos_set_to_end
```

Expected: all 4 tests pass.

- [ ] **Step 6: Run the full suite**

```bash
cargo test
```

Expected: all 248 tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs src/app/actions.rs
git commit -m "feat: reset cursor_pos on clear; set to end on begin_edit"
```

---

## Task 10: Update cursor rendering to use `cursor_pos`

**Files:**
- Modify: `src/ui/capture.rs`

No unit test for TUI rendering — verified via `cargo build` and manual visual inspection.

- [ ] **Step 1: Replace the cursor positioning logic in `render_input`**

In `src/ui/capture.rs`, the current cursor placement code is the last block of `render_input` (lines 55–67). Replace everything from `let last = ...` to the end of the function with:

```rust
    // Compute cursor (row, col) from cursor_pos byte offset
    let mut remaining = app.cursor_pos;
    let mut cursor_row = 0;
    let mut cursor_col = 0usize; // character count within the line
    for (i, line) in input_lines.iter().enumerate() {
        let line_bytes = line.len();
        if remaining <= line_bytes {
            cursor_col = line[..remaining].chars().count();
            cursor_row = i;
            break;
        }
        remaining -= line_bytes + 1; // +1 for the '\n' separator
        // Fallback if cursor_pos == input.len() and input ends with '\n'
        cursor_row = i + 1;
        cursor_col = 0;
    }

    let col = if cursor_row == 0 {
        prefix.chars().count() + cursor_col
    } else {
        cursor_col
    };

    let inner_x = area.x + 1;
    let inner_y = area.y + 1;
    frame.set_cursor_position(ratatui::layout::Position::new(
        inner_x + col as u16,
        inner_y + (cursor_row.saturating_sub(overflow)) as u16,
    ));
```

- [ ] **Step 2: Build to verify it compiles**

```bash
cargo build
```

Expected: no errors.

- [ ] **Step 3: Run the full test suite**

```bash
cargo test
```

Expected: all 248 tests pass.

- [ ] **Step 4: Manual visual test**

```bash
cargo run
```

Verify:
- Typing characters places the cursor after each typed character
- `←` moves cursor left through the text, character by character
- `→` moves cursor right through the text
- `Ctrl+A` or `Home` jumps cursor to start of the first line
- `Ctrl+E` or `End` jumps cursor to end of the line
- Typing when cursor is mid-string inserts at cursor (not at end)
- Backspace when cursor is mid-string deletes the character to the left
- Entering edit mode (`e` in Navigate mode): cursor starts at end of loaded text, can then be moved and edited

- [ ] **Step 5: Commit**

```bash
git add src/ui/capture.rs
git commit -m "feat: render cursor at cursor_pos in capture box"
```

---

## Plan complete

All 10 tasks implement the cursor movement feature described in the spec at `docs/superpowers/specs/2026-06-06-capture-box-cursor-movement-design.md`.
