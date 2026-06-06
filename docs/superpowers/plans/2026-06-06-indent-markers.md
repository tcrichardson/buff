# Indent Markers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace broken Tab-inserts-spaces indentation with a `->` prefix marker that converts to Markdown spaces on submit, enabling nested bullets and indented content in the capture box.

**Architecture:** `expand_indent_markers` is a pure per-line helper added to `format_entry` in `writer.rs` — it strips leading `->` tokens and replaces them with 2-spaces each before any other processing. `input.rs` gets a new `PrependIndent` action (`Ctrl+.`) and the existing `TypeIndent` action is changed from inserting `"  "` to inserting `"->"`. No changes to the trim pipeline.

**Tech Stack:** Rust, ratatui/crossterm, cargo test

---

## File Map

| File | Change |
|---|---|
| `src/model/writer.rs` | Add `expand_indent_markers(line: &str) -> String`; call it on every line at the top of `format_entry` |
| `src/app/input.rs` | Add `UiAction::PrependIndent`; bind `Ctrl+.` to it; change `TypeIndent` to insert `"->"` |
| `src/ui/help.rs` | Update capture-mode key listing |
| `README.md` | Update capture box key table and Markdown notes section |

---

## Task 1: `expand_indent_markers` helper and `format_entry` integration

**Files:**
- Modify: `src/model/writer.rs`

- [ ] **Step 1: Write failing tests**

Add these tests inside the `#[cfg(test)] mod tests` block in `src/model/writer.rs`:

```rust
#[test]
fn expand_indent_markers_zero_markers() {
    assert_eq!(expand_indent_markers("- item"), "- item");
}

#[test]
fn expand_indent_markers_one_marker() {
    assert_eq!(expand_indent_markers("->- item"), "  - item");
}

#[test]
fn expand_indent_markers_two_markers() {
    assert_eq!(expand_indent_markers("->->- item"), "    - item");
}

#[test]
fn expand_indent_markers_three_markers() {
    assert_eq!(expand_indent_markers("->->->- item"), "      - item");
}

#[test]
fn expand_indent_markers_plain_text() {
    assert_eq!(expand_indent_markers("->plain"), "  plain");
}

#[test]
fn expand_indent_markers_mid_line_preserved() {
    assert_eq!(expand_indent_markers("hello -> world"), "hello -> world");
}

#[test]
fn format_entry_single_indent_marker_becomes_bullet() {
    assert_eq!(format_entry("->- item", None), vec!["  - item"]);
}

#[test]
fn format_entry_double_indent_marker() {
    assert_eq!(format_entry("->->- item", None), vec!["    - item"]);
}

#[test]
fn format_entry_indent_marker_plain_text() {
    assert_eq!(format_entry("->plain", None), vec!["  plain"]);
}

#[test]
fn format_entry_multiline_indent_markers() {
    assert_eq!(
        format_entry("->- parent\n->->- child", None),
        vec!["  - parent", "    - child"]
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test expand_indent_markers format_entry_single_indent format_entry_double format_entry_indent format_entry_multiline
```

Expected: compile error (`expand_indent_markers` not found) or test failures.

- [ ] **Step 3: Add `expand_indent_markers` function**

Add this function near the top of `src/model/writer.rs`, before `looks_like_markdown`:

```rust
/// Strip leading `->` markers and replace each with two spaces.
/// `->` appearing anywhere other than the very start of the line is preserved.
pub fn expand_indent_markers(line: &str) -> String {
    let mut rest = line;
    let mut indent = String::new();
    while let Some(after) = rest.strip_prefix("->") {
        indent.push_str("  ");
        rest = after;
    }
    if indent.is_empty() {
        line.to_string()
    } else {
        format!("{}{}", indent, rest)
    }
}
```

- [ ] **Step 4: Apply `expand_indent_markers` in `format_entry`**

In `src/model/writer.rs`, change `format_entry` so that the first thing it does after splitting lines is expand markers on every line. The current function starts like this:

```rust
pub fn format_entry(input: &str, timestamp: Option<&str>) -> Vec<String> {
    let mut raw: Vec<&str> = input.split('\n').collect();
    while raw.len() > 1 && raw.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        raw.pop();
    }

    if looks_like_markdown(raw[0]) {
        return raw.iter().map(|s| s.to_string()).collect();
    }
    ...
```

Replace it with:

```rust
pub fn format_entry(input: &str, timestamp: Option<&str>) -> Vec<String> {
    let mut raw: Vec<&str> = input.split('\n').collect();
    while raw.len() > 1 && raw.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        raw.pop();
    }

    // Expand leading `->` markers to spaces on every line before any other processing.
    let expanded: Vec<String> = raw.iter().map(|l| expand_indent_markers(l)).collect();
    let raw: Vec<&str> = expanded.iter().map(|s| s.as_str()).collect();

    if looks_like_markdown(raw[0]) {
        return raw.iter().map(|s| s.to_string()).collect();
    }

    let mut out = Vec::with_capacity(raw.len());
    let first = match timestamp {
        Some(ts) => format!("{} {}", ts, raw[0]),
        None => raw[0].to_string(),
    };
    out.push(first);
    for line in &raw[1..] {
        out.push(line.to_string());
    }
    out
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/model/writer.rs
git commit -m "feat: add expand_indent_markers and apply in format_entry"
```

---

## Task 2: `Ctrl+.` shortcut (`PrependIndent` action)

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Write failing tests**

Add these tests inside `#[cfg(test)] mod tests` in `src/app/input.rs`:

```rust
#[test]
fn ctrl_dot_in_capture_emits_prepend_indent() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.focus = Focus::Capture;
    let key = KeyEvent {
        code: KeyCode::Char('.'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(key_to_action(&state, key), Some(UiAction::PrependIndent));
}

#[test]
fn prepend_indent_on_first_line_inserts_at_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "- item".to_string();
    state.cursor_pos = 3; // mid-line
    execute_action(&mut state, UiAction::PrependIndent).unwrap();
    assert_eq!(state.input, "->- item");
    assert_eq!(state.cursor_pos, 5); // 3 + 2
}

#[test]
fn prepend_indent_on_second_line_inserts_at_line_start() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "- parent\n- child".to_string();
    state.cursor_pos = 12; // somewhere on second line ("- ch|ild")
    execute_action(&mut state, UiAction::PrependIndent).unwrap();
    assert_eq!(state.input, "- parent\n->- child");
    assert_eq!(state.cursor_pos, 14); // 12 + 2
}

#[test]
fn prepend_indent_twice_stacks_markers() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "- item".to_string();
    state.cursor_pos = 0;
    execute_action(&mut state, UiAction::PrependIndent).unwrap();
    execute_action(&mut state, UiAction::PrependIndent).unwrap();
    assert_eq!(state.input, "->->- item");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test ctrl_dot prepend_indent
```

Expected: compile error (`PrependIndent` variant not found).

- [ ] **Step 3: Add `PrependIndent` to `UiAction`**

In `src/app/input.rs`, add `PrependIndent` to the enum alongside the other capture-mode actions:

```rust
// Capture mode
TypeChar(char),
DeleteChar,
TypeNewline,
TypeIndent,
PrependIndent,
SubmitInput,
CommitEdit,
```

- [ ] **Step 4: Bind `Ctrl+.` in `key_to_action`**

In `src/app/input.rs`, in the `Focus::Capture` match arm, add a binding for `Ctrl+.` just before the catch-all `_ => None`:

```rust
KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    Some(UiAction::PrependIndent)
}
```

- [ ] **Step 5: Implement `PrependIndent` in `execute_action`**

In `src/app/input.rs`, add the execution case alongside the other `TypeX` variants:

```rust
UiAction::PrependIndent => {
    let line_start = match state.input[..state.cursor_pos].rfind('\n') {
        Some(nl) => nl + 1,
        None => 0,
    };
    state.input.insert_str(line_start, "->");
    state.cursor_pos += 2;
}
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs
git commit -m "feat: add PrependIndent action bound to Ctrl+."
```

---

## Task 3: Change `TypeIndent` to insert `"->"` instead of spaces

**Files:**
- Modify: `src/app/input.rs`

- [ ] **Step 1: Update the existing `TypeIndent` tests**

In `src/app/input.rs`, find and update these two existing tests:

```rust
#[test]
fn type_indent_inserts_two_spaces() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    execute_action(&mut state, UiAction::TypeIndent).unwrap();
    assert_eq!(state.input, "->");
    assert_eq!(state.cursor_pos, 2);
}

#[test]
fn type_indent_inserts_at_cursor_pos() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    state.input = "ab".to_string();
    state.cursor_pos = 1; // between 'a' and 'b'
    execute_action(&mut state, UiAction::TypeIndent).unwrap();
    assert_eq!(state.input, "a->b");
    assert_eq!(state.cursor_pos, 3);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test type_indent
```

Expected: FAIL — current impl still inserts `"  "`.

- [ ] **Step 3: Change `TypeIndent` execution to insert `"->"`**

In `src/app/input.rs`, find the `UiAction::TypeIndent` arm in `execute_action` and change it:

```rust
UiAction::TypeIndent => {
    state.input.insert_str(state.cursor_pos, "->");
    state.cursor_pos += 2;
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/app/input.rs
git commit -m "fix: TypeIndent inserts '->' instead of spaces"
```

---

## Task 4: Integration test — dispatch with `->` markers

**Files:**
- Modify: `src/app/actions.rs`

- [ ] **Step 1: Write failing integration tests**

Add these tests inside `#[cfg(test)] mod tests` in `src/app/actions.rs`:

```rust
#[test]
fn dispatch_single_indent_marker_stores_indented_bullet() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Entry("->- sub".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(text.contains("  - sub\n"), "got: {}", text);
}

#[test]
fn dispatch_double_indent_marker_stores_deeply_indented_bullet() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Entry("->->- deep".to_string())).unwrap();
    let text = state.doc.to_text();
    assert!(text.contains("    - deep\n"), "got: {}", text);
}

#[test]
fn dispatch_parent_then_sub_bullet_are_independent_selectables() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = test_state(&tmp);
    dispatch(&mut state, Command::Entry("- parent".to_string())).unwrap();
    dispatch(&mut state, Command::Entry("->- child".to_string())).unwrap();
    assert_eq!(
        state.selectables.len(),
        2,
        "expected 2 selectables, got: {:?}",
        state.selectables
    );
    assert_eq!(state.selectables[0].text, "- parent");
    assert_eq!(state.selectables[1].text, "  - child");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test dispatch_single_indent dispatch_double_indent dispatch_parent_then_sub
```

Expected: FAIL — markers not yet expanded (Task 1 must be complete first).

- [ ] **Step 3: Run all tests to confirm Task 1 already makes these pass**

```bash
cargo test
```

Expected: all tests pass (Tasks 1–3 already implement the behaviour these test).
If any fail, recheck Task 1 Step 4.

- [ ] **Step 4: Commit**

```bash
git add src/app/actions.rs
git commit -m "test: integration tests for -> indent marker dispatch"
```

---

## Task 5: Update help overlay and README

**Files:**
- Modify: `src/ui/help.rs`
- Modify: `README.md`

- [ ] **Step 1: Update `src/ui/help.rs`**

Find the help text string in `src/ui/help.rs`. Replace the capture-mode section:

```rust
let help_text = r#"Capture mode:
  type to enter notes, Enter to submit, Esc to navigate
  Tab        insert indent (->)
  Ctrl+.     prepend indent at line start

Commands:
```

(Leave everything after `Commands:` unchanged.)

- [ ] **Step 2: Update `README.md` capture box key table**

Find the capture box key table in `README.md` and replace the `Tab` row:

```markdown
| `Tab` | Insert indent marker (`->`) at cursor |
| `Ctrl+.` | Prepend indent marker (`->`) at start of current line |
```

- [ ] **Step 3: Update `README.md` Markdown notes section**

Find the Markdown notes section. After the existing sentence "Plain text is stored as-is. If you want a bullet, type it explicitly:", add:

```markdown
To create nested bullets or indented content, prefix the line with `->` (one level = 2 spaces). Stack for deeper nesting: `->->- item`. Use `Ctrl+.` to prepend `->` at the line start, or `Tab` to insert it at the cursor.

```markdown
->- Sub bullet          stored as:   - Sub bullet
->->- Deep bullet       stored as:     - Deep bullet
\```
```

- [ ] **Step 4: Run all tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/ui/help.rs README.md
git commit -m "docs: update help and README for -> indent marker"
```
