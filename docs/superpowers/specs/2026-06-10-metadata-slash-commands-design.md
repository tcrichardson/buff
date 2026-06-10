# Metadata Slash Commands Design

**Date:** 2026-06-10  
**Status:** Approved

## Overview

Add `/purpose <text>` and `/topic <text>` slash commands that write metadata onto note objects. Introduce a `meta:` prefix convention in raw storage to unambiguously identify metadata lines. Migrate existing time fields (Scheduled, Started, Ended) to the same prefix. Display all metadata fields with the `meta:` prefix stripped.

---

## Storage Format

Every metadata line in the raw Markdown file uses a `meta:` prefix. The prefix is the sole signal that a line is metadata — no hard-coded key enumeration is needed at the detection level.

**Meeting example:**
```markdown
### Daily Standup
meta:Purpose: Align on blockers for the sprint
meta:Scheduled: 09:00
meta:Started: 09:05
meta:Ended: 09:45
- bullet point
- [ ] todo item
```

**Note block example:**
```markdown
### Architecture Review
meta:Topic: API design for v2
- bullet point
```

### Canonical Field Order

Enforced on every write (same mechanism as the existing `TIME_FIELD_ORDER`):

```
Purpose → Topic → Scheduled → Started → Ended
```

### Migration

Legacy files containing bare `Scheduled: `, `Started: `, `Ended: ` lines (without the `meta:` prefix) are silently rewritten to `meta:Scheduled: ` etc. the first time that meeting block is written to. No separate migration pass or one-time file scan is needed.

---

## Write / Mutation Layer (`src/model/writer.rs`)

### `is_metadata_line(line: &str) -> bool`

Replaces `is_time_field_line()`. Returns `true` if the line starts with `meta:`. Works for any future field without code changes.

### `set_metadata_field(lines: &mut Vec<String>, heading_line: usize, key: &str, value: &str)`

Generalized replacement for `set_meeting_time_field()`. Algorithm:

1. Scan lines after `heading_line` while `is_metadata_line()` is true — this is the metadata block boundary.
2. During the scan, also recognize legacy bare `Scheduled: `, `Started: `, `Ended: ` lines (migration support) — treat them as if they had the `meta:` prefix.
3. Parse all found lines into a `HashMap<String, String>` (key stripped of `meta:` prefix).
4. Insert or overwrite `key → value`.
5. Rewrite the entire block in canonical field order:

```rust
const METADATA_FIELD_ORDER: &[&str] = &[
    "Purpose", "Topic", "Scheduled", "Started", "Ended",
];
```

Only fields present in the map are written; absent fields are omitted.

### Callers Updated

All existing callers of `set_meeting_time_field()` are updated to call `set_metadata_field()` with equivalent arguments:

- `handle_start()` → `set_metadata_field(..., "Started", &time)`
- `handle_end()` → `set_metadata_field(..., "Ended", &time)`
- `handle_scheduled()` → `set_metadata_field(..., "Scheduled", &time)`

Behavior for these commands is unchanged from the user's perspective.

---

## Command Parsing & Dispatch

### `src/app/command.rs`

Two new variants added to the `Command` enum:

```rust
Purpose(String),   // /purpose <text>
Topic(String),     // /topic <text>
```

Parsing rules in `parse()`:

| Input | Result |
|-------|--------|
| `/purpose kick off Q3` | `Command::Purpose("kick off Q3".to_string())` |
| `/topic API design for v2` | `Command::Topic("API design for v2".to_string())` |
| `/purpose` (no text) | error status: `"Usage: /purpose <text>"` |
| `/topic` (no text) | error status: `"Usage: /topic <text>"` |

### `src/app/actions.rs`

Two new handlers called from `dispatch()`:

**`handle_purpose(state, text)`**
- Requires `Context::Meeting(ord)` — sets status `"Not in a meeting"` otherwise.
- Looks up `meeting.heading_line`.
- Calls `set_metadata_field(&mut state.doc.lines, heading_line, "Purpose", &text)`.
- Calls `after_doc_mutation(state)`.

**`handle_topic(state, text)`**
- Requires `Context::NoteBlock(ord)` — sets status `"Not in a note block"` otherwise.
- Looks up the note block's `heading_line`.
- Calls `set_metadata_field(&mut state.doc.lines, heading_line, "Topic", &text)`.
- Calls `after_doc_mutation(state)`.

---

## Display Layer (`src/ui/document.rs`)

### New `LineKind` variant

```rust
MetaField(String),   // line with "meta:" prefix stripped, e.g. "Purpose: kick off Q3"
```

### `classify_line()` — new arm

Added before the `Raw` fallback:

```rust
if line.starts_with("meta:") {
    return LineKind::MetaField(line[5..].to_string());
}
```

### `render_line_kind()` — new arm

`MetaField` renders in a muted, italic style to visually distinguish it from note body content — consistent with how blockquotes use the pipe glyph and italic:

```
Purpose: kick off Q3        ← theme.metadata color, italic
Scheduled: 09:00            ← same style
```

The theme color used is `theme.metadata` (a new theme field, defaulting to a muted/dim value).

### VimCursor behavior

The existing `VimCursor` arm already renders the raw line verbatim. When the cursor is on a `meta:` line, the user sees `meta:Purpose: kick off Q3` — consistent with how `#` heading sigils appear on the cursor line. No change needed.

---

## Error Cases

| Situation | Status message |
|-----------|---------------|
| `/purpose` with no text | `"Usage: /purpose <text>"` |
| `/topic` with no text | `"Usage: /topic <text>"` |
| `/purpose` outside meeting context | `"Not in a meeting"` |
| `/topic` outside note block context | `"Not in a note block"` |

---

## Files Changed

| File | Change |
|------|--------|
| `src/model/writer.rs` | Add `is_metadata_line()`, `set_metadata_field()`, deprecate `is_time_field_line()` / `set_meeting_time_field()` |
| `src/app/command.rs` | Add `Purpose(String)` and `Topic(String)` to `Command` enum; add parsing |
| `src/app/actions.rs` | Add `handle_purpose()`, `handle_topic()`; update existing handlers to use `set_metadata_field()` |
| `src/ui/document.rs` | Add `LineKind::MetaField`, classify/render `meta:` lines |
| `src/ui/theme.rs` | Add `metadata` color field |
