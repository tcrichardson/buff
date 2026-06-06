# Indent Markers Design

**Date:** 2026-06-06  
**Status:** Approved

## Problem

Users cannot create nested bullets or indented list content in the capture box. Two
`trim()` calls in the pipeline — one in `command::parse` and one in `dispatch` —
strip all leading whitespace before entries reach storage. Tab-based indentation was
previously added but is silently broken for this reason.

## Solution

Introduce a `->` prefix marker that converts to two spaces of indentation on submit.
Because `->` is not whitespace, it survives both `trim()` calls unchanged. Each leading
`->` at the start of a line represents one indent level (2 spaces). Markers stack.

---

## Conversion Rules

Conversion is applied **per line**, inside `format_entry` in `src/model/writer.rs`,
before the `looks_like_markdown` check.

| Input line | Stored as |
|---|---|
| `->- item` | `  - item` |
| `->->- item` | `    - item` |
| `->->->- item` | `      - item` |
| `->plain text` | `  plain text` |
| `->  1. item` | `    1. item` |
| `hello -> world` | `hello -> world` (mid-line: preserved) |

**Rule:** strip each leading `->` token (no whitespace requirement between markers),
replace the entire leading sequence with `N * 2` spaces where N is the count.  
`->` appearing anywhere other than the very start of a line is left as-is.

After conversion, the normal `looks_like_markdown` / plain-text path runs as usual.
The `.md` file contains only standard Markdown spaces — no `->` markers are ever
written to disk.

---

## Keyboard Shortcuts

Two keys produce `->` output in capture mode:

| Key | Behaviour |
|---|---|
| `Ctrl+.` | Prepend `->` at the **start of the current line**, regardless of cursor position. Cursor advances by 2. |
| `Tab` | Insert `->` at the **cursor position**. Cursor advances by 2. |

`Ctrl+.` is the "indent this line" shortcut — pressing it multiple times stacks
levels. `Tab` is the quick inline insert when the cursor is already at the start
of the line or the user wants `->` inline.

Both keys produce identical output when pressed at the start of an empty line.

**Note:** `Tab` previously inserted two literal spaces. Those spaces were stripped
by `trim()` on submit, making the feature broken. Changing Tab to produce `->` fixes
this without touching the trim logic.

---

## Data Flow

```
User presses Ctrl+.
  → UiAction::PrependIndent
  → execute_action: find last \n before cursor_pos (line start = pos+1, or 0)
  → insert "->" at line_start in state.input
  → cursor_pos += 2

User presses Tab
  → UiAction::TypeIndent  (renamed from current spaces impl)
  → execute_action: insert "->" at cursor_pos in state.input
  → cursor_pos += 2

User presses Enter
  → command::parse(input.trim())   ["->" survives trim]
  → Command::Entry("->- sub item")
  → dispatch: format_entry("->- sub item", time)
  → expand_indent_markers per line: "->- sub item" → "  - sub item"
  → looks_like_markdown("  - sub item") → true (trim_start → "- sub item")
  → stored verbatim as "  - sub item"
```

---

## Components Changed

| File | Change |
|---|---|
| `src/model/writer.rs` | Add `expand_indent_markers(line)` helper; apply to each line at top of `format_entry` before `looks_like_markdown` check |
| `src/app/input.rs` | Add `UiAction::PrependIndent`; change `Ctrl+.` to emit it; change `UiAction::TypeIndent` to insert `"->"` instead of `"  "` |
| `src/ui/help.rs` | Update capture-mode key table |
| `README.md` | Update capture box key table and Markdown notes section |

---

## Error Handling

No failure modes. `expand_indent_markers` is a pure string transformation with no
allocations beyond the output string. Invalid or partial `->` sequences (e.g. a lone
`-` at the start) are left untouched.

---

## Testing

### Unit — `src/model/writer.rs`
- `expand_indent_markers` with 0, 1, 2, 3 leading markers
- `expand_indent_markers` with mid-line `->` (preserved)
- `format_entry` with `->- bullet` produces `["  - bullet"]`
- `format_entry` with `->->- bullet` produces `["    - bullet"]`
- `format_entry` with `->plain` produces `["  plain"]`
- Multi-line: `->- parent\n->->- child` produces `["  - parent", "    - child"]`

### Unit — `src/app/input.rs`
- `Ctrl+.` on first line with cursor mid-line: `->` prepended at position 0
- `Ctrl+.` on second line: `->` prepended at correct line-start offset
- `Ctrl+.` pressed twice: `->->` at line start
- `Tab` inserts `->` at cursor position

### Integration — `src/app/actions.rs`
- Dispatch `"->- sub"` → doc contains `  - sub`
- Dispatch `"->->- deep"` → doc contains `    - deep`
- Indented bullet is an independent selectable (not merged with prior bullet)
