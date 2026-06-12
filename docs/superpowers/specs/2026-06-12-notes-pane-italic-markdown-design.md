# Notes Pane Italic Markdown

## Summary

Render inline italic markdown (`*text*` and `_text_`) in the notes pane using `Modifier::ITALIC`, matching the existing treatment of inline bold markdown (`**text**` and `__text__`).

## Motivation

The notes pane already stylizes bullets (with a `•` marker) and bold text (via `Modifier::BOLD`). Italic markdown is currently left as raw characters, so users do not see the emphasis they intended.

## Scope

- In-scope: parsing and rendering `*italic*` and `_italic_` everywhere inline bold is already supported.
- Out-of-scope: nested/combined markers such as `***bold-italic***`; block-level italic styles; new theme colors for italic.

## Design

### Code changes

All work is in `src/ui/document.rs`.

1. Rename `parse_inline_bold` to `parse_inline_formatting` and update its doc comment.
2. Extend the marker scan to find the nearest of `**`, `__`, `*`, and `_`.
   - `**...**` and `__...__` apply `Modifier::BOLD`.
   - `*...*` and `_..._` apply `Modifier::ITALIC`.
3. Preserve the current behavior for unmatched markers: treat the remaining text as plain.
4. Update every existing call site from `parse_inline_bold` to `parse_inline_formatting`.

### Rendering behavior

Italic rendering applies to the same line kinds that already receive bold rendering:

- `LineKind::Plain`
- `LineKind::Bullet`
- `LineKind::TodoUnchecked`
- `LineKind::TodoDone`
- `LineKind::Quote`
- `LineKind::Ordered`
- `LineKind::MetaField`

Bullets are still recognized by the existing `classify_line` logic (`* `, `- `, `+ `) before inline formatting runs, so `* item` remains a bullet and `*item*` renders as italic.

No new theme colors are added; italic text uses the current foreground color plus `Modifier::ITALIC`, just as bold uses the current foreground color plus `Modifier::BOLD`.

### Tests

Add unit tests for `parse_inline_formatting` covering:

- `*italic*` renders with `Modifier::ITALIC`.
- `_italic_` renders with `Modifier::ITALIC`.
- `**bold**` still renders with `Modifier::BOLD`.
- Mixed bold and italic on one line.
- Unmatched `*` / `_` markers remain plain text.

Add render tests for italic inside bullets, todos, quotes, and plain lines.

## Verification

Run `cargo test` and confirm the full test suite passes.

## Risks / Open Questions

- `*italic*` inside a bullet line could theoretically conflict with the bullet marker, but `classify_line` strips the bullet prefix first, so only text after `* ` is parsed for inline formatting.
- Triple-asterisk syntax (`***text***`) is not explicitly supported; it will parse as bold containing literal asterisks, which is acceptable for this scope.
