# Notes Pane Italic Markdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render inline italic markdown (`*text*` and `_text_`) in the notes pane using `Modifier::ITALIC`, matching the existing bold markdown support.

**Architecture:** Extend the existing single-pass inline formatter in `src/ui/document.rs` to recognize both bold and italic markers, then reuse it across all line kinds that already support inline bold.

**Tech Stack:** Rust, ratatui

---

## File Structure

- `src/ui/document.rs` — only file that changes.
  - `parse_inline_bold` becomes `parse_inline_formatting`.
  - All production call sites switch to the new name.
  - Existing `parse_inline_bold_*` tests are renamed and updated.
  - New unit tests cover italic parsing.
  - New render tests cover italic in bullets, todos, quotes, ordered lists, meta fields, and plain lines.

## Task 1: Rename `parse_inline_bold` to `parse_inline_formatting`

**Files:**
- Modify: `src/ui/document.rs`

- [ ] **Step 1: Rename the function and update its doc comment**

Change `src/ui/document.rs` lines 120–158 from:

```rust
/// Parse inline bold markers (`**text**` and `__text__`) in `text`, applying
/// `base_style` to non-bold spans and `base_style` + `Modifier::BOLD` to bold spans.
fn parse_inline_bold<'a>(text: &'a str, base_style: Style) -> Vec<Span<'a>> {
```

to:

```rust
/// Parse inline formatting markers (`**text**`, `__text__`, `*text*`, `_text_`) in
/// `text`, applying `base_style` to plain spans and the relevant modifier to
/// formatted spans (`Modifier::BOLD` or `Modifier::ITALIC`).
fn parse_inline_formatting<'a>(text: &'a str, base_style: Style) -> Vec<Span<'a>> {
```

Leave the function body unchanged for now.

- [ ] **Step 2: Update production call sites**

Replace every call to `parse_inline_bold(` with `parse_inline_formatting(` in `src/ui/document.rs`:

- Line 182: inside `TodoUnchecked`
- Line 194: inside `TodoDone`
- Line 204: inside `Quote`
- Line 213: inside `Bullet`
- Line 216: inside `Ordered`
- Line 217: inside `MetaField`
- Line 223: inside `Plain`

- [ ] **Step 3: Update existing test names and call sites**

In the test module (around lines 831–991), rename the test group comment and all `parse_inline_bold_*` functions/test bodies to use `parse_inline_formatting`:

- `// --- parse_inline_bold tests ---` → `// --- parse_inline_formatting tests ---`
- `parse_inline_bold_no_markers_returns_plain`
- `parse_inline_bold_double_stars`
- `parse_inline_bold_double_underscores`
- `parse_inline_bold_multiple_markers`
- `parse_inline_bold_unmatched_marker_is_plain`
- `parse_inline_bold_preserves_base_style`

For example, change:

```rust
fn parse_inline_bold_no_markers_returns_plain() {
    let spans = parse_inline_bold("just text", Style::default());
```

to:

```rust
fn parse_inline_formatting_no_markers_returns_plain() {
    let spans = parse_inline_formatting("just text", Style::default());
```

Do this for each test in that group.

- [ ] **Step 4: Run tests to confirm no regressions**

```bash
cargo test parse_inline_formatting
```

Expected: all existing tests pass under the new name.

- [ ] **Step 5: Commit**

```bash
git add src/ui/document.rs
git commit -m "refactor(document): rename parse_inline_bold to parse_inline_formatting"
```

## Task 2: Add failing unit tests for italic formatting

**Files:**
- Modify: `src/ui/document.rs`

- [ ] **Step 1: Add italic unit tests after the existing formatting tests**

Append the following tests to the `parse_inline_formatting` test group in `src/ui/document.rs`:

```rust
    #[test]
    fn parse_inline_formatting_italic_stars() {
        let spans = parse_inline_formatting("hello *world*", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_italic_underscores() {
        let spans = parse_inline_formatting("hello _world_", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_mixed_bold_and_italic() {
        let spans = parse_inline_formatting("**bold** and *italic*", Style::default());
        assert_eq!(
            spans,
            vec![
                Span::styled("bold", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" and "),
                Span::styled("italic", Style::default().add_modifier(Modifier::ITALIC)),
            ]
        );
    }

    #[test]
    fn parse_inline_formatting_unmatched_italic_is_plain() {
        let spans = parse_inline_formatting("hello *world", Style::default());
        assert_eq!(spans, vec![Span::raw("hello *world")]);
    }

    #[test]
    fn parse_inline_formatting_italic_preserves_base_style() {
        let base = Style::default()
            .fg(ratatui::style::Color::Red)
            .add_modifier(Modifier::BOLD);
        let spans = parse_inline_formatting("plain *italic*", base);
        assert_eq!(
            spans,
            vec![
                Span::styled("plain ", base),
                Span::styled("italic", base.add_modifier(Modifier::ITALIC)),
            ]
        );
    }
```

- [ ] **Step 2: Run the new tests to verify they fail**

```bash
cargo test parse_inline_formatting_italic
```

Expected: failures because italic markers are not yet parsed.

- [ ] **Step 3: Commit the failing tests**

```bash
git add src/ui/document.rs
git commit -m "test(document): add italic formatting tests"
```

## Task 3: Implement italic parsing in `parse_inline_formatting`

**Files:**
- Modify: `src/ui/document.rs`

- [ ] **Step 1: Replace the function body**

Change the body of `parse_inline_formatting` in `src/ui/document.rs` to:

```rust
fn parse_inline_formatting<'a>(text: &'a str, base_style: Style) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut rest = text;

    while !rest.is_empty() {
        let star_pos = rest.find("**");
        let under_pos = rest.find("__");
        // Ignore a single `*` or `_` that is the first character of a double marker.
        let single_star_pos = rest.find('*').filter(|&p| star_pos != Some(p));
        let single_under_pos = rest.find('_').filter(|&p| under_pos != Some(p));

        let candidates = [
            (star_pos, "**", Modifier::BOLD),
            (under_pos, "__", Modifier::BOLD),
            (single_star_pos, "*", Modifier::ITALIC),
            (single_under_pos, "_", Modifier::ITALIC),
        ];
        let Some((start, marker, modifier)) = candidates
            .into_iter()
            .filter_map(|(pos, marker, modifier)| pos.map(|p| (p, marker, modifier)))
            .min_by_key(|(p, _, _)| *p)
        else {
            break;
        };

        let after_open = &rest[start + marker.len()..];
        if let Some(end) = after_open.find(marker) {
            if start > 0 {
                spans.push(Span::styled(&rest[..start], base_style));
            }
            let styled_text = &after_open[..end];
            spans.push(Span::styled(styled_text, base_style.add_modifier(modifier)));
            rest = &after_open[end + marker.len()..];
        } else {
            // Unmatched opening marker — treat remaining text as plain.
            break;
        }
    }

    if !rest.is_empty() {
        spans.push(Span::styled(rest, base_style));
    }

    spans
}
```

- [ ] **Step 2: Run all formatting tests**

```bash
cargo test parse_inline_formatting
```

Expected: all tests pass, including the new italic tests.

- [ ] **Step 3: Commit**

```bash
git add src/ui/document.rs
git commit -m "feat(document): parse italic markdown in notes pane"
```

## Task 4: Add render tests for italic in line kinds

**Files:**
- Modify: `src/ui/document.rs`

- [ ] **Step 1: Append render tests for italic**

Add the following tests after the existing `render_meta_field_with_inline_bold` test in `src/ui/document.rs`:

```rust
    #[test]
    fn render_plain_with_inline_italic() {
        let t = th();
        let line = render_line_kind(LineKind::Plain("hello *world*"), &t);
        assert_eq!(
            line,
            Line::from(vec![
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
            ])
        );
    }

    #[test]
    fn render_bullet_with_inline_italic() {
        let line = render_line_kind(LineKind::Bullet("", "hello *world*"), &th());
        assert_eq!(
            line,
            Line::from(vec![
                Span::raw("• "),
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
            ])
        );
    }

    #[test]
    fn render_todo_unchecked_with_inline_italic() {
        let line = render_line_kind(LineKind::TodoUnchecked("", "hello *world*"), &th());
        assert_eq!(
            line,
            Line::from(vec![
                Span::raw("☐ "),
                Span::raw("hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
            ])
        );
    }

    #[test]
    fn render_quote_with_inline_italic() {
        let t = th();
        let line = render_line_kind(LineKind::Quote("hello *world*"), &t);
        let base = Style::default().add_modifier(Modifier::ITALIC);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled(
                    "│ ",
                    Style::default().fg(t.quote_marker).add_modifier(Modifier::ITALIC),
                ),
                Span::styled("hello ", base),
                Span::styled("world", base.add_modifier(Modifier::ITALIC)),
            ])
        );
    }

    #[test]
    fn render_ordered_with_inline_italic() {
        let line = render_line_kind(LineKind::Ordered("1. hello *world*"), &th());
        assert_eq!(
            line,
            Line::from(vec![
                Span::raw("1. hello "),
                Span::styled("world", Style::default().add_modifier(Modifier::ITALIC)),
            ])
        );
    }

    #[test]
    fn render_meta_field_with_inline_italic() {
        let t = th();
        let line = render_line_kind(LineKind::MetaField("hello *world*"), &t);
        let base = Style::default().fg(t.metadata).add_modifier(Modifier::ITALIC);
        assert_eq!(
            line,
            Line::from(vec![
                Span::styled("hello ", base),
                Span::styled("world", base.add_modifier(Modifier::ITALIC)),
            ])
        );
    }
```

- [ ] **Step 2: Run the new render tests**

```bash
cargo test inline_italic
```

Expected: all new render tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/ui/document.rs
git commit -m "test(document): add italic render tests for line kinds"
```

## Task 5: Run the full test suite

**Files:**
- None (verification only)

- [ ] **Step 1: Run all tests**

```bash
cargo test
```

Expected: full suite passes.

- [ ] **Step 2: Fix any regressions**

If any test fails, update the relevant code or test and rerun `cargo test`.

- [ ] **Step 3: Final commit (if fixes were needed)**

```bash
git add src/ui/document.rs
git commit -m "fix(document): address regressions from italic formatting"
```

If no fixes were needed, this task has no additional commit.

## Self-Review Checklist

1. **Spec coverage:**
   - Italic rendered with `Modifier::ITALIC` — Task 3.
   - Works in all line kinds that support bold — Task 4 tests.
   - No new theme colors — no task needed.
   - Bullet marker conflict avoided — handled by existing `classify_line` logic, noted in Task 3.

2. **Placeholder scan:**
   - No TBD/TODO/fill-in-later statements.
   - Every code step contains the actual code or exact command.

3. **Type consistency:**
   - Function name `parse_inline_formatting` is used consistently across production and test code.
   - `Modifier::ITALIC` and `Modifier::BOLD` are used as specified.

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-06-12-notes-pane-italic-markdown.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using `executing-plans`, batch execution with checkpoints.

Which approach would you like?
