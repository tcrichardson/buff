# style_line Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `style_line` in `src/ui/document.rs` into a pure classification function and a pure rendering function, eliminating its cyclomatic complexity of 19 without changing any public behavior.

**Architecture:** Introduce a private `LineKind<'a>` enum carrying all data needed for rendering. `classify_line` (pure logic, no Theme) maps a raw line to a `LineKind`. `render_line_kind` (pure rendering, no branching logic) maps a `LineKind` to a ratatui `Line`. `style_line` becomes a two-line adapter. All existing tests continue to pass unchanged because `style_line`'s signature is not altered.

**Tech Stack:** Rust, Cargo, ratatui. All tests are inline `#[cfg(test)]` modules. Test runner: `cargo test`.

---

### Task 1: Establish baseline

**Files:**
- Read: `src/ui/document.rs`

- [ ] **Step 1: Run the full test suite**

```bash
cargo test
```

Expected output ends with something like:
```
test result: ok. 446 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

Note the exact passing count. Every subsequent task must end with at least this many tests passing.

- [ ] **Step 2: Confirm the target function**

```bash
cargo test ui::document
```

Expected: 18 tests pass (heading1..6_hides_hash, indented/unindented bullet/todo variants, heading_on_cursor_line_shows_raw). These are your regression guard — they must all continue passing after every task.

---

### Task 2: Add the `LineKind` enum

**Files:**
- Modify: `src/ui/document.rs` (add enum after existing imports, before `style_line`)

This enum carries every piece of data that the renderer needs so that `render_line_kind` requires no extra arguments beyond the `LineKind` and `Theme`.

- [ ] **Step 1: Add the enum to `src/ui/document.rs` after the `use` block**

Insert this block immediately before the `fn style_line` definition (line 14):

```rust
/// Classifies a single document line into a rendering variant.
/// Each variant carries exactly the string slices needed for rendering,
/// so `render_line_kind` needs no other context.
#[derive(Debug, PartialEq)]
enum LineKind<'a> {
    /// Vim cursor sits on this line — show raw text with cursor background.
    VimCursor(&'a str),
    /// Inside (or entering/leaving) a code fence — show with code colour.
    Code(&'a str),
    /// Markdown heading. `u8` is the level (1–6); `&str` is the text after the hashes.
    Heading(u8, &'a str),
    /// Unchecked todo checkbox. `(indent, rest)`.
    TodoUnchecked(&'a str, &'a str),
    /// Checked todo checkbox (lowercase or uppercase x). `(indent, rest)`.
    TodoDone(&'a str, &'a str),
    /// Blockquote line. Contains the text after `"> "` (may be empty).
    Quote(&'a str),
    /// Unordered bullet (`-`, `*`, `+`). `(indent, rest)`.
    Bullet(&'a str, &'a str),
    /// Ordered list item — shown verbatim.
    Ordered(&'a str),
    /// Plain text — shown verbatim.
    Plain(&'a str),
}
```

- [ ] **Step 2: Run the test suite to confirm no regressions**

```bash
cargo test
```

Expected: same passing count as Task 1. (The enum is dead code until the next task; the compiler may warn about unused variants — that is fine for now.)

- [ ] **Step 3: Commit**

```bash
git add src/ui/document.rs
git commit -m "refactor(document): add LineKind enum for classify/render split"
```

---

### Task 3: Add `classify_line` with tests

**Files:**
- Modify: `src/ui/document.rs` (add `classify_line` function + tests in `#[cfg(test)]` block)

`classify_line` contains all the detection logic currently inside `style_line`. It mutates `in_code` (same as today) and returns a `LineKind`.

- [ ] **Step 1: Add the failing tests for `classify_line`**

Add inside the existing `mod tests` block at the bottom of `src/ui/document.rs`, after the last existing test:

```rust
    // --- classify_line tests ---

    #[test]
    fn classify_vim_cursor_returns_vim_cursor_variant() {
        let mut in_code = false;
        let result = classify_line("# My Note", &mut in_code, true);
        assert_eq!(result, LineKind::VimCursor("# My Note"));
        assert!(!in_code, "vim_cursor should reset in_code to false");
    }

    #[test]
    fn classify_vim_cursor_resets_in_code() {
        let mut in_code = true;
        let result = classify_line("anything", &mut in_code, true);
        assert_eq!(result, LineKind::VimCursor("anything"));
        assert!(!in_code);
    }

    #[test]
    fn classify_code_fence_opens_block() {
        let mut in_code = false;
        let result = classify_line("```", &mut in_code, false);
        assert_eq!(result, LineKind::Code("```"));
        assert!(in_code, "fence should open the code block");
    }

    #[test]
    fn classify_code_fence_closes_block() {
        let mut in_code = true;
        let result = classify_line("```", &mut in_code, false);
        assert_eq!(result, LineKind::Code("```"));
        assert!(!in_code, "second fence should close the code block");
    }

    #[test]
    fn classify_line_inside_code_block() {
        let mut in_code = true;
        let result = classify_line("let x = 1;", &mut in_code, false);
        assert_eq!(result, LineKind::Code("let x = 1;"));
        assert!(in_code, "in_code should remain true for non-fence lines");
    }

    #[test]
    fn classify_heading1() {
        let mut in_code = false;
        assert_eq!(classify_line("# Title", &mut in_code, false), LineKind::Heading(1, "Title"));
    }

    #[test]
    fn classify_heading2() {
        let mut in_code = false;
        assert_eq!(classify_line("## Notes", &mut in_code, false), LineKind::Heading(2, "Notes"));
    }

    #[test]
    fn classify_heading3() {
        let mut in_code = false;
        assert_eq!(classify_line("### Sub", &mut in_code, false), LineKind::Heading(3, "Sub"));
    }

    #[test]
    fn classify_heading4() {
        let mut in_code = false;
        assert_eq!(classify_line("#### Deep", &mut in_code, false), LineKind::Heading(4, "Deep"));
    }

    #[test]
    fn classify_heading5() {
        let mut in_code = false;
        assert_eq!(classify_line("##### Five", &mut in_code, false), LineKind::Heading(5, "Five"));
    }

    #[test]
    fn classify_heading6() {
        let mut in_code = false;
        assert_eq!(classify_line("###### Six", &mut in_code, false), LineKind::Heading(6, "Six"));
    }

    #[test]
    fn classify_todo_unchecked() {
        let mut in_code = false;
        assert_eq!(classify_line("- [ ] task", &mut in_code, false), LineKind::TodoUnchecked("", "task"));
    }

    #[test]
    fn classify_todo_unchecked_indented() {
        let mut in_code = false;
        assert_eq!(classify_line("  - [ ] task", &mut in_code, false), LineKind::TodoUnchecked("  ", "task"));
    }

    #[test]
    fn classify_todo_done_lowercase_x() {
        let mut in_code = false;
        assert_eq!(classify_line("- [x] done", &mut in_code, false), LineKind::TodoDone("", "done"));
    }

    #[test]
    fn classify_todo_done_uppercase_x() {
        let mut in_code = false;
        assert_eq!(classify_line("  - [X] done", &mut in_code, false), LineKind::TodoDone("  ", "done"));
    }

    #[test]
    fn classify_quote_with_text() {
        let mut in_code = false;
        assert_eq!(classify_line("> hello", &mut in_code, false), LineKind::Quote("hello"));
    }

    #[test]
    fn classify_quote_bare() {
        let mut in_code = false;
        assert_eq!(classify_line(">", &mut in_code, false), LineKind::Quote(""));
    }

    #[test]
    fn classify_bullet_dash() {
        let mut in_code = false;
        assert_eq!(classify_line("- item", &mut in_code, false), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_star() {
        let mut in_code = false;
        assert_eq!(classify_line("* item", &mut in_code, false), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_plus() {
        let mut in_code = false;
        assert_eq!(classify_line("+ item", &mut in_code, false), LineKind::Bullet("", "item"));
    }

    #[test]
    fn classify_bullet_indented() {
        let mut in_code = false;
        assert_eq!(classify_line("  - sub", &mut in_code, false), LineKind::Bullet("  ", "sub"));
    }

    #[test]
    fn classify_ordered() {
        let mut in_code = false;
        assert_eq!(classify_line("1. first", &mut in_code, false), LineKind::Ordered("1. first"));
    }

    #[test]
    fn classify_plain() {
        let mut in_code = false;
        assert_eq!(classify_line("just text", &mut in_code, false), LineKind::Plain("just text"));
    }
```

- [ ] **Step 2: Run tests to confirm they fail with "not found"**

```bash
cargo test ui::document::tests::classify 2>&1 | head -30
```

Expected: compile error — `classify_line` is not defined yet.

- [ ] **Step 3: Add the `classify_line` function**

Add this function immediately before `style_line` in `src/ui/document.rs`:

```rust
/// Classifies `line` into a `LineKind`, mutating `in_code` when a code fence
/// is encountered.  Called once per line during rendering.
fn classify_line<'a>(line: &'a str, in_code: &mut bool, vim_cursor: bool) -> LineKind<'a> {
    if vim_cursor {
        *in_code = false;
        return LineKind::VimCursor(line);
    }

    let fence = line.trim_start().starts_with("```");
    if *in_code || fence {
        if fence {
            *in_code = !*in_code;
        }
        return LineKind::Code(line);
    }

    // Try headings longest-prefix-first to avoid `#` matching `##`.
    for (prefix, level) in [
        ("###### ", 6u8),
        ("##### ",  5),
        ("#### ",   4),
        ("### ",    3),
        ("## ",     2),
        ("# ",      1),
    ] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return LineKind::Heading(level, rest);
        }
    }

    // Blockquote matches the full line (not trimmed — they're not indented here).
    if let Some(rest) = line.strip_prefix("> ") {
        return LineKind::Quote(rest);
    }
    if line == ">" {
        return LineKind::Quote("");
    }

    // For bullets and todos: extract leading whitespace so indented variants
    // render identically to unindented ones.
    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let trimmed = &line[indent_len..];

    if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        return LineKind::TodoUnchecked(indent, rest);
    }
    if let Some(rest) = trimmed
        .strip_prefix("- [x] ")
        .or_else(|| trimmed.strip_prefix("- [X] "))
    {
        return LineKind::TodoDone(indent, rest);
    }
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        return LineKind::Bullet(indent, rest);
    }

    if crate::model::parser::is_ordered(line) {
        return LineKind::Ordered(line);
    }

    LineKind::Plain(line)
}
```

- [ ] **Step 4: Run classifier tests**

```bash
cargo test ui::document::tests::classify
```

Expected: all `classify_*` tests pass.

- [ ] **Step 5: Run full suite to confirm no regressions**

```bash
cargo test
```

Expected: same total count as baseline, plus the new `classify_*` tests.

- [ ] **Step 6: Commit**

```bash
git add src/ui/document.rs
git commit -m "refactor(document): add classify_line extracting detection logic from style_line"
```

---

### Task 4: Add `heading_color` helper and `render_line_kind` with tests

**Files:**
- Modify: `src/ui/document.rs` (add `heading_color` + `render_line_kind` before `style_line`)

`render_line_kind` is a pure `match` with no nested conditionals. Each arm builds a `Line` directly from the data in the variant.

- [ ] **Step 1: Add tests for `render_line_kind`**

Append inside the existing `mod tests` block in `src/ui/document.rs`:

```rust
    // --- render_line_kind tests ---

    #[test]
    fn render_vim_cursor_uses_cursor_bg() {
        let t = th();
        let line = render_line_kind(LineKind::VimCursor("# raw"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled("# raw", Style::default().bg(t.vim_cursor_line)))
        );
    }

    #[test]
    fn render_code_uses_code_fg() {
        let t = th();
        let line = render_line_kind(LineKind::Code("let x = 1;"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled("let x = 1;", Style::default().fg(t.code)))
        );
    }

    #[test]
    fn render_heading1_bold_heading1_color() {
        let t = th();
        let line = render_line_kind(LineKind::Heading(1, "Title"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled(
                "Title",
                Style::default().fg(t.heading1).add_modifier(Modifier::BOLD)
            ))
        );
    }

    #[test]
    fn render_heading6_bold_heading6_color() {
        let t = th();
        let line = render_line_kind(LineKind::Heading(6, "Six"), &t);
        assert_eq!(
            line,
            Line::from(Span::styled(
                "Six",
                Style::default().fg(t.heading6).add_modifier(Modifier::BOLD)
            ))
        );
    }

    #[test]
    fn render_todo_unchecked_no_indent() {
        let line = render_line_kind(LineKind::TodoUnchecked("", "task"), &th());
        assert_eq!(line, Line::from(vec![Span::raw("☐ "), Span::raw("task")]));
    }

    #[test]
    fn render_todo_unchecked_with_indent() {
        let line = render_line_kind(LineKind::TodoUnchecked("  ", "task"), &th());
        assert_eq!(
            line,
            Line::from(vec![Span::raw("  "), Span::raw("☐ "), Span::raw("task")])
        );
    }

    #[test]
    fn render_todo_done_strikethrough() {
        let t = th();
        let line = render_line_kind(LineKind::TodoDone("", "done"), &t);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled("☑ ", Style::default().fg(t.todo_done)),
                Span::styled(
                    "done",
                    Style::default().fg(t.todo_done).add_modifier(Modifier::CROSSED_OUT)
                ),
            ])
        );
    }

    #[test]
    fn render_quote() {
        let t = th();
        let line = render_line_kind(LineKind::Quote("hello"), &t);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled(
                    "│ ",
                    Style::default().fg(t.quote_marker).add_modifier(Modifier::ITALIC)
                ),
                Span::styled("hello", Style::default().add_modifier(Modifier::ITALIC)),
            ])
        );
    }

    #[test]
    fn render_bullet_no_indent() {
        let line = render_line_kind(LineKind::Bullet("", "item"), &th());
        assert_eq!(line, Line::from(vec![Span::raw("• "), Span::raw("item")]));
    }

    #[test]
    fn render_bullet_with_indent() {
        let line = render_line_kind(LineKind::Bullet("  ", "sub"), &th());
        assert_eq!(
            line,
            Line::from(vec![Span::raw("  "), Span::raw("• "), Span::raw("sub")])
        );
    }

    #[test]
    fn render_ordered_verbatim() {
        let line = render_line_kind(LineKind::Ordered("1. first"), &th());
        assert_eq!(line, Line::from(Span::raw("1. first")));
    }

    #[test]
    fn render_plain_verbatim() {
        let line = render_line_kind(LineKind::Plain("just text"), &th());
        assert_eq!(line, Line::from("just text"));
    }
```

- [ ] **Step 2: Run tests to confirm they fail with compile error**

```bash
cargo test ui::document::tests::render 2>&1 | head -20
```

Expected: compile error — `render_line_kind` is not defined yet.

- [ ] **Step 3: Add `heading_color` and `render_line_kind`**

Add these two functions immediately before `style_line` in `src/ui/document.rs`:

```rust
/// Maps a heading level (1–6) to its theme colour.
fn heading_color(level: u8, theme: &Theme) -> ratatui::style::Color {
    match level {
        1 => theme.heading1,
        2 => theme.heading2,
        3 => theme.heading3,
        4 => theme.heading4,
        5 => theme.heading5,
        _ => theme.heading6,
    }
}

/// Converts a classified `LineKind` into a styled ratatui `Line`.
/// No branching logic lives here — only span construction.
fn render_line_kind<'a>(kind: LineKind<'a>, theme: &Theme) -> Line<'a> {
    match kind {
        LineKind::VimCursor(line) => {
            Line::from(Span::styled(line, Style::default().bg(theme.vim_cursor_line)))
        }
        LineKind::Code(line) => {
            Line::from(Span::styled(line, Style::default().fg(theme.code)))
        }
        LineKind::Heading(level, text) => Line::from(Span::styled(
            text,
            Style::default()
                .fg(heading_color(level, theme))
                .add_modifier(Modifier::BOLD),
        )),
        LineKind::TodoUnchecked(indent, rest) => {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::raw("☐ "));
            spans.push(Span::raw(rest));
            Line::from(spans)
        }
        LineKind::TodoDone(indent, rest) => {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::styled("☑ ", Style::default().fg(theme.todo_done)));
            spans.push(Span::styled(
                rest,
                Style::default()
                    .fg(theme.todo_done)
                    .add_modifier(Modifier::CROSSED_OUT),
            ));
            Line::from(spans)
        }
        LineKind::Quote(rest) => Line::from(vec![
            Span::styled(
                "│ ",
                Style::default()
                    .fg(theme.quote_marker)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(rest, Style::default().add_modifier(Modifier::ITALIC)),
        ]),
        LineKind::Bullet(indent, rest) => {
            let mut spans = Vec::with_capacity(3);
            if !indent.is_empty() {
                spans.push(Span::raw(indent));
            }
            spans.push(Span::raw("• "));
            spans.push(Span::raw(rest));
            Line::from(spans)
        }
        LineKind::Ordered(line) => Line::from(Span::raw(line)),
        LineKind::Plain(line) => Line::from(line),
    }
}
```

- [ ] **Step 4: Run the new render tests**

```bash
cargo test ui::document::tests::render
```

Expected: all `render_*` tests pass.

- [ ] **Step 5: Run full suite**

```bash
cargo test
```

Expected: same baseline count plus the new tests.

- [ ] **Step 6: Commit**

```bash
git add src/ui/document.rs
git commit -m "refactor(document): add render_line_kind and heading_color helpers"
```

---

### Task 5: Replace `style_line` body with classify + render

**Files:**
- Modify: `src/ui/document.rs` (replace `style_line` body — all existing callers and tests are unaffected)

This is the payoff task. The function signature stays identical; only the body changes.

- [ ] **Step 1: Replace the body of `style_line`**

The current function body (lines 15–122) becomes:

```rust
fn style_line<'a>(line: &'a str, in_code: &mut bool, vim_cursor: bool, theme: &Theme) -> Line<'a> {
    render_line_kind(classify_line(line, in_code, vim_cursor), theme)
}
```

The old body from `if vim_cursor {` through the closing `}` on line 122 should be fully deleted and replaced with the single `render_line_kind(...)` call.

- [ ] **Step 2: Run the existing style_line tests to confirm they all still pass**

```bash
cargo test ui::document
```

Expected: all tests in `ui::document` pass (the 18 original tests plus the new classify/render tests added in Tasks 3 and 4).

- [ ] **Step 3: Run the full suite**

```bash
cargo test
```

Expected: same total count as end of Task 4. Zero regressions.

- [ ] **Step 4: Commit**

```bash
git add src/ui/document.rs
git commit -m "refactor(document): replace style_line body with classify_line + render_line_kind"
```

---

### Task 6: Final verification and cleanup

**Files:**
- Read: `src/ui/document.rs`

- [ ] **Step 1: Review the completed file**

Read through `src/ui/document.rs` and verify:
- `LineKind` enum is present with `#[derive(Debug, PartialEq)]`
- `classify_line` is present and contains all detection logic
- `heading_color` is present
- `render_line_kind` is present and contains only a `match` with span construction
- `style_line` body is exactly one line: `render_line_kind(classify_line(line, in_code, vim_cursor), theme)`
- No dead code warnings

- [ ] **Step 2: Check for compiler warnings**

```bash
cargo build 2>&1 | grep warning
```

Expected: no warnings about unused code in `document.rs`. If any `LineKind` variants appear unused, the `render_line_kind` arms have not been written for all cases — fix accordingly.

- [ ] **Step 3: Run the full test suite one final time**

```bash
cargo test
```

Expected: all tests pass, count ≥ baseline from Task 1.

- [ ] **Step 4: Final commit (if any cleanup was needed)**

If Step 1 or 2 required changes:
```bash
git add src/ui/document.rs
git commit -m "refactor(document): cleanup after style_line classify/render split"
```

If nothing changed, no commit is needed.

---

## Summary

After this plan executes, `src/ui/document.rs` will contain:

| Function | Complexity (before → after) | Responsibility |
|---|---|---|
| `classify_line` | new, ~12 | Line type detection, `in_code` mutation |
| `heading_color` | new, 1 | Level → theme colour lookup |
| `render_line_kind` | new, ~9 | Span construction from `LineKind` |
| `style_line` | 19 → 1 | Thin adapter (two functions chained) |

New line types can be added by: (1) adding a `LineKind` variant, (2) adding a detection branch in `classify_line`, (3) adding a `match` arm in `render_line_kind`. No other function needs touching.
