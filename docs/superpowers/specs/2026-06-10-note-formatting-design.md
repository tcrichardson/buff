# Note Formatting Design

**Date:** 2026-06-10  
**Scope:** Two rendering changes to `src/ui/document.rs`

---

## Overview

Two changes to how notes are rendered in the TUI document view:

1. **Hide `#` heading markers** — heading lines display only their text content (styled with color + bold), not the raw `#` prefix characters. The raw characters are shown only when the cursor is on that line (edit mode), which is already handled by the existing cursor-line raw-display logic.

2. **Indented bullets render like unindented bullets** — bullet and todo-checkbox lines with leading whitespace substitute their marker character the same way as unindented lines, preserving the indentation.

---

## Change 1: Hide `#` in Heading Rendering

### Current behavior

`src/ui/document.rs` lines 39–68 match each heading level via `strip_prefix`, then re-add the prefix via `format!`:

```rust
if let Some(rest) = line.strip_prefix("# ") {
    Line::from(Span::styled(
        format!("# {}", rest),  // prefix stripped then immediately re-added
        Style::default().fg(theme.heading1).add_modifier(Modifier::BOLD),
    ))
}
```

The `#` characters are visible in the rendered note.

### Target behavior

Only `rest` is rendered — the `# ` prefix is dropped from display:

```rust
if let Some(rest) = line.strip_prefix("# ") {
    Line::from(Span::styled(
        rest,
        Style::default().fg(theme.heading1).add_modifier(Modifier::BOLD),
    ))
}
```

Applied to all six heading levels (`#` through `######`).

### Edit mode

No change needed. The cursor-line raw-display path (lines 24–29) already renders the full raw line — including `#` characters — whenever the vim cursor is on a heading line.

### Visual differentiation

Heading levels remain distinguishable by color + bold:

| Level | Usage in app | Color |
|-------|-------------|-------|
| H1 | Note name | `theme.heading1` |
| H2 | Section headers (Meetings, Notes, To-dos) | `theme.heading2` |
| H3–H6 | User headings | `theme.heading3`–`heading6` |

Color + bold is sufficient; no additional prefix or indentation indicator is needed.

---

## Change 2: Indented Bullet/Todo Rendering

### Current behavior

The bullet and todo-checkbox checks use `strip_prefix`, which only matches lines starting at column 0:

```rust
} else if let Some(rest) = line.strip_prefix("- [ ] ") {
    // only matches "- [ ] item", not "  - [ ] item"
```

Indented lines like `  - item` fall through to the plain-text branch and display raw.

### Target behavior

Bullet and todo-checkbox branches are refactored to handle indented lines by splitting each line into `(leading_whitespace, trimmed_content)` before pattern matching.

Rendering for an indented bullet:
```
  - sub-item    →   [2 spaces]• sub-item
    - deeper    →   [4 spaces]• deeper
  - [ ] task    →   [2 spaces]☐ task
  - [x] task    →   [2 spaces]☑ task  (strikethrough + done color)
```

### Implementation approach

For the bullet and todo-checkbox branches only:

1. Compute `indent`: the leading whitespace of the line (`&line[..line.len() - line.trim_start().len()]`)
2. Get `trimmed`: `line.trim_start()`
3. Try todo/bullet patterns against `trimmed`
4. If matched, render as `Span::raw(indent) + Span::raw(symbol) + Span::raw(content)`

Headings and blockquotes are not indented in this application, so their branches remain unchanged.

### Pattern matching order (unchanged)

Todo-checkbox branches must precede plain-bullet branches (since `- [ ] ` starts with `- `):

1. Cursor line → raw (unchanged)
2. Code fences (unchanged)
3. Headings `######` … `#` (prefix hidden per Change 1)
4. `[indent]- [ ] ` → `[indent]☐ content`
5. `[indent]- [x/X] ` → `[indent]☑ content` (strikethrough + `theme.todo_done`)
6. `> ` blockquote (unchanged — not indented in this app)
7. `[indent]- ` / `[indent]* ` / `[indent]+ ` → `[indent]• content`
8. Ordered lists (unchanged)
9. Plain text (unchanged)

---

## Files Changed

| File | Change |
|------|--------|
| `src/ui/document.rs` | Heading rendering: render `rest` instead of `format!("# {}", rest)` for all six heading levels. Bullet/todo rendering: split indent from trimmed before matching. |

No changes to `src/model/parser.rs`, `src/ui/theme.rs`, or any other file.

---

## Out of Scope

- Blockquote indentation handling
- Ordered list indentation handling
- Any changes to edit-mode behavior (cursor-line raw display already works correctly)
- Any changes to theme colors or heading styles
