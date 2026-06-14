# Chat Panel Markdown Rendering — Design Spec

**Date:** 2026-06-14  
**Status:** Approved

## Problem

The notes panel renders markdown with styled headings, bullets, blockquotes, code blocks, and inline bold/italic. The chat panel renders all text verbatim — asterisks and pound signs appear as literal characters. This makes AI responses that include markdown formatting hard to read.

## Goal

Render markdown in the chat panel with full parity to the notes panel, using a shared rendering module to avoid code duplication.

---

## Architecture

### New file: `src/ui/markdown.rs`

A shared markdown rendering module extracted from `document.rs`. Owns the core pipeline:

1. **`classify_line`** — classifies a `&str` into a `LineKind`, maintaining code-fence state via `&mut bool`
2. **`parse_inline_formatting`** — parses `**bold**` / `*italic*` / `__bold__` / `_italic_` into `Vec<Span>`
3. **`render_line_kind`** — maps a `LineKind` to a ratatui `Line` using theme colors and an explicit background `Color`
4. **`render_markdown_line`** — convenience wrapper combining classify + render in one call

#### Public API

```rust
pub enum LineKind<'a> {
    Code(&'a str),
    Heading(u8, &'a str),
    TodoUnchecked(&'a str, &'a str),
    TodoDone(&'a str, &'a str),
    Quote(&'a str),
    Bullet(&'a str, &'a str),
    Ordered(&'a str),
    Plain(&'a str),
}

pub fn classify_line<'a>(line: &'a str, in_code: &mut bool) -> LineKind<'a>;

pub fn parse_inline_formatting<'a>(text: &'a str, base_style: Style) -> Vec<Span<'a>>;

pub fn render_line_kind<'a>(kind: LineKind<'a>, theme: &Theme, bg: Color) -> Line<'a>;

pub fn render_markdown_line<'a>(
    line: &'a str,
    in_code: &mut bool,
    theme: &Theme,
    bg: Color,
) -> Line<'a>;
```

`LineKind` is `pub` so callers can inspect the kind before rendering (e.g., to determine wrap indentation in `chat_panel.rs`).

**Excluded from the shared module:** `VimCursor` and `MetaField` variants — these are document-specific and remain in `document.rs`.

---

### Modified: `src/ui/document.rs`

Imports and delegates to `markdown.rs` for all standard line kinds. Retains local handling of `VimCursor` and `MetaField` as a pre-pass before calling `classify_line`. The visible behavior of the notes panel is unchanged.

---

### Modified: `src/ui/chat_panel.rs`

Replaces the current word-wrap-then-plain-style rendering loop with markdown-aware rendering.

#### Message layout

Each message renders as:

```
You                          ← speaker label, dim style, own Line
<message body lines>         ← each line rendered through render_markdown_line

AI                           ← speaker label, regular style, own Line
<message body lines>         ← markdown-rendered
```

- Speaker label is `"You"` / `"AI"` (no colon) — User label in dim/muted style, Assistant label in regular style, both on `chat_panel_bg`
- One blank `Line::raw("")` separates consecutive messages
- No blank line after the final message

#### Word-wrap interaction with markdown

For each raw message line:

1. Call `classify_line` to determine the `LineKind`
2. Word-wrap the text content portion, respecting the available width
3. Determine the `Style` for the line kind (e.g., heading style, bullet prefix + style) and apply it to each wrapped segment
4. For bullet continuation lines: indent to align with the bullet text content (i.e., indent by the width of `"• "` plus any leading whitespace from the original)

Code block lines are wrapped but not otherwise transformed (preserving indentation). Heading lines are short enough in practice that wrapping rarely applies.

---

## Data Flow

```
chat message line: &str
        │
        ▼
classify_line(&str, &mut in_code)  →  LineKind
        │
        ▼
word_wrap(text_portion, width)  →  Vec<String>
        │
        ▼ (for each wrapped segment)
apply style for this LineKind  →  Line<'_>
        │
        ▼
pushed into Vec<Line> for Paragraph rendering
```

---

## Error Handling

No new error conditions introduced. Both modules operate on `&str` slices in memory with no I/O. Invalid or unexpected markdown syntax falls through to `LineKind::Plain`, matching the existing behavior of `document.rs`.

---

## Testing

- Unit tests for `classify_line` covering all `LineKind` variants (most already exist in `document.rs` — migrate them to `markdown.rs`)
- Unit tests for `parse_inline_formatting` (same: migrate existing tests)
- Integration: manually verify that the notes panel renders identically after the refactor
- Integration: verify AI responses with headings, bullets, code blocks, and bold/italic render correctly in the chat panel

---

## Out of Scope

- Horizontal rules (`---`)
- Tables
- Nested blockquotes
- Link rendering (`[text](url)`)
- Any changes to scroll behavior in the chat panel
