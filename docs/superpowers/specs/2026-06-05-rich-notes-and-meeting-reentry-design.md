# Rich Notes, Multi-line Capture & Meeting Re-entry — Design

Date: 2026-06-05
Status: Approved (pending implementation plan)

## Overview

Three enhancements to Kua-Tin's capture and navigation experience, built on the
existing anchored-line Markdown model:

1. **Re-enter a meeting** — select a meeting heading in navigate mode and press
   `Enter` to resume it as the active capture context.
2. **Rich Markdown notes** — beyond plain bullets, capture headings, blockquotes,
   numbered lists, and fenced code blocks via raw-Markdown passthrough.
3. **Multi-line notes** — compose entries spanning several lines with `Alt+Enter`,
   committed as a single block with `Enter`.

All three share one underlying change: entries become **blocks** that can span
multiple lines, and more line types become selectable/editable/deletable in
navigate mode.

### Goals

- Resume an existing meeting's capture context without creating a duplicate.
- Let users write real Markdown (headings, quotes, ordered lists, code fences)
  that is stored verbatim, alongside the default bullet behavior.
- Compose multi-line entries from the capture bar.
- Make every entry type a first-class, navigable, editable, deletable block.
- Preserve the project's core invariant: untouched lines are written back
  verbatim; the file is never reflowed.

### Non-Goals (v1)

- In-line cursor movement within the composer (arrow keys inside the text).
  Editing stays append/backspace-only, consistent with today's behavior.
- Hiding or syntax-highlighting the contents of code blocks.
- Editing nested-list indentation as structure.
- Configurable keybindings.
- Enhanced (Kitty) keyboard protocol support (we deliberately use `Alt+Enter`,
  which does not require it).

## Decisions Captured During Brainstorming

- **Markdown input model:** raw-Markdown passthrough. Plain text becomes a
  bullet; text that already looks like Markdown is stored verbatim. No new
  per-type slash commands or mode toggles.
- **Newline key:** `Alt+Enter` (Option+Enter on macOS) only. `Enter` commits.
  Rationale: `Shift+Enter` and `Cmd+Enter` are indistinguishable from `Enter`
  unless the terminal supports the enhanced keyboard protocol (Terminal.app does
  not); `Alt+Enter` works without it.
- **Plain multi-line storage:** one bullet with continuation lines indented two
  spaces — a single logical, selectable entry.
- **Meeting re-entry gesture:** `Enter` on a selected meeting heading in navigate
  mode (no prior conflict — `Enter` is currently a no-op there).
- **Selectability scope:** all entry types are selectable, editable (`e`), and
  deletable (`dd`) as whole blocks.
- **Core model approach:** extend the anchored line model with block-aware
  selectables (chosen over a full typed AST, which would fight the
  verbatim-preservation invariant, and over keeping single-line selectables,
  which conflicts with whole-block editing).

## Architecture

The `Document` remains a `Vec<String>` of lines (source of truth). The change is
in how lines are *grouped and classified* into selectable blocks, how the
capture path *commits* text, and how blocks *render*.

Affected modules:

```
src/
  model/
    day.rs       # Selectable: line -> Range<usize>; expanded SelectableKind
    parser.rs    # block classifier (lines -> blocks with ranges + kinds)
    writer.rs    # unified commit (passthrough), range-based edit/delete
  app/
    actions.rs   # resume meeting; block-aware edit/delete; commit via passthrough
    state.rs     # (input already a String; may carry '\n')
  ui/
    capture.rs   # multi-line composer rendering + cursor
    document.rs  # render new line types; highlight whole multi-line blocks
    layout.rs    # dynamic capture-box height
  main.rs        # Alt+Enter newline; Enter commit; Enter resumes meeting
```

## Data Model

### Selectable becomes block-aware

```rust
pub enum SelectableKind {
    Bullet,                          // "- text" (+ indented continuation lines)
    Todo { done: bool },             // "- [ ]" / "- [x]"
    MeetingHeading { ordinal: usize },
    MarkdownHeading,                 // "# " … "###### " typed as a note
    Quote,                           // consecutive "> " lines
    Numbered,                        // "1. " / "2) " (+ continuation)
    CodeBlock,                       // ``` … ``` (fence + body)
    Raw,                             // anything else (external edits)
}

pub struct Selectable {
    pub lines: std::ops::Range<usize>, // was: line: usize
    pub kind: SelectableKind,
    pub text: String,                  // raw block text, lines joined with '\n'
}
```

### Block classifier (`parser.rs`)

Walks the line vector and groups lines into blocks. Rules, evaluated per line:

- **Code fence:** a line whose trimmed start is ` ``` ` opens a block; it extends
  through the next closing ` ``` ` line (inclusive). An unterminated fence
  extends to the end of the section.
- **Bullet / Todo:** a `- ` (or `- [ ]` / `- [x]` / `- [X]`) line, plus any
  immediately following **continuation lines** — non-empty lines indented by at
  least two spaces (or a tab). Stops at a blank line, another marker, a heading,
  or a section boundary.
- **Quote:** one or more consecutive lines whose trimmed start is `> `.
- **Meeting heading:** a `### ` line located within the Meetings section. Single
  line; carries its ordinal (position among meetings).
- **Markdown heading:** any `#`…`######` + space line that is not the day heading
  (`# YYYY-MM-DD …`), not a known section heading (`## Meetings/Notes/To-dos`),
  and not a meeting heading — i.e. one the user typed as a note. Single line.
- **Numbered item:** a line starting with digits followed by `. ` or `) `, plus
  continuation lines (same rule as bullets). Each item is its own block.
- **Raw:** any other non-blank line (e.g. content from external edits) is a
  single-line block so it remains selectable and is never silently dropped.

Blank lines are not part of any block and are not selectable.

Document/section headings (`# YYYY-MM-DD …`, `## Meetings/Notes/To-dos`) are
**not** selectable; only user content blocks are.

## Capture & Commit

### Unified commit path

Both adding a new entry and editing an existing block flow through the **same**
passthrough classifier; they differ only in placement:

- **Add:** classify the composed input, then insert the resulting line(s) into
  the active section (Notes or the active Meeting), using the existing
  block-insert logic.
- **Edit:** load the selected block's **raw text** (its lines joined with `\n`)
  into the capture bar. On commit, classify the new input and **replace** the
  block's line range with the result.

Because editing reloads raw text and commit re-classifies, every block kind
round-trips: re-committing an unchanged `- foo`, a `### 09:15 Standup`, or a
fenced code block reproduces the same lines.

### Passthrough rules

Examine the **first line** of the composed input (trimmed for detection):

- If it matches a Markdown signal, store the **entire input verbatim** (every
  line exactly as typed, joined with `\n`):
  - heading: starts with `#` then a space (`# `, `## `, `### `, …)
  - blockquote: starts with `> `
  - code fence: starts with ` ``` `
  - unordered list: starts with `- `, `* `, or `+ `
  - ordered list: starts with one or more digits then `. ` or `) `
- Otherwise it is **plain text**:
  - single line → `- <text>`
  - multi-line → `- <line 1>`, then each subsequent line indented two spaces
    (`  <line N>`)

### Timestamps

`timestamp_entries` (the `HH:MM` prefix) applies **only to plain-text bullets**,
and only to the first line. It is never applied to verbatim Markdown blocks, so
it cannot corrupt a code fence, quote, or heading. Meeting sub-headings continue
to record their start time as today.

When a user opts into raw Markdown (e.g. types `- foo` explicitly and then adds a
continuation line), they own the continuation formatting — the verbatim path does
not auto-indent. This is an accepted, documented edge.

## Multi-line Composer

- `AppState.input: String` may now contain `\n`.
- Key handling in capture mode:
  - `Alt+Enter` (`KeyCode::Enter` + `KeyModifiers::ALT`) → append `\n`.
  - `Enter` (no modifier) → commit (new entry, or commit edit).
  - `Backspace` → remove the last character, including a trailing `\n`.
- In-line cursor movement remains out of scope; input is append/backspace only,
  matching current behavior. Arrow keys keep their existing meaning.
- The capture box height grows with the number of input lines, capped at a
  maximum (e.g. 10 rows) after which it scrolls; the document pane shrinks to
  accommodate. The cursor is positioned at the end of the input, accounting for
  line and column.

### Terminal requirement

`Alt+Enter` requires "Use Option as Meta key" to be enabled in Terminal.app and
iTerm2 (other terminals generally deliver Alt/Option as Meta by default). This
will be documented in the README. No enhanced keyboard protocol is required.

## Navigate Mode

Selection moves over the richer block list (existing `j`/`k`/`↑`/`↓`, `g`/`G`).
Per-block keys:

| Key | Behavior |
|---|---|
| `Enter` | **Meeting heading only:** set that meeting as the active context (`Context::Meeting(ordinal)`), update the context display, and switch to capture mode. No-op on other block kinds. |
| `e` | Edit the selected block: load its raw text (multi-line aware) into the capture bar; commit replaces the block's range. Works on every kind, including renaming a meeting heading. |
| `d` then `d` | Delete the selected block's entire line range (two-step confirm, as today). |
| `Space` / `x` | Toggle done — todo blocks only (unchanged; error message on non-todos as today). |

### Meeting-heading delete

A meeting-heading block is just the heading line, so `dd` removes only that line.
Any bullets that were under it remain as plain bullets in the Meetings section
(unparented but not lost or corrupted). This is chosen deliberately over
"delete the whole meeting and its notes," because silently wiping all of a
meeting's content with two keystrokes is too destructive.

## Rendering

A faithful light-styling pass (not a full Markdown renderer). In addition to
today's headings/bullets/todos:

- **Numbered items** (`N.` / `N)`) render with their number preserved.
- **Quotes** (`>`) render with a styled prefix (e.g. `│ `) and dim/italic text.
- **Code blocks** render in a distinct color (e.g. gray); fence lines are shown
  as-is, not hidden.
- **Continuation lines** render indented as part of their owning bullet/numbered
  item.
- **Block highlight:** when the selection is on a multi-line block, **all** lines
  in the block's range are highlighted (today only a single line is). Scrolling
  ensures the block's last line stays visible.

## Error Handling

- Commit of empty/whitespace-only input is a no-op (as today).
- An unterminated code fence is tolerated: the block extends to the section end;
  saving never drops lines.
- Unrecognized external lines become `Raw` single-line blocks — selectable,
  editable, and preserved verbatim.
- Atomic saves (temp file + rename) are unchanged.
- `Enter` on a non-meeting block and `Space`/`x` on a non-todo behave
  predictably (no-op / existing status message); no panics on empty documents.

## Testing Strategy

Following the existing pure-core TDD pattern (string and temp-dir fixtures, plus
Ratatui `TestBackend` smoke tests):

- **Classifier (`parser`):**
  - each block kind detected with the correct line range
  - bullet/numbered continuation grouping (indent, stop conditions)
  - code-fence open/close, including unterminated fence to section end
  - meeting-heading ordinals within the Meetings section
  - blank lines excluded; section/day headings not selectable
- **Passthrough commit (`writer`/`actions`):**
  - plain single line → `- text`; plain multi-line → bullet + indented
    continuation
  - each Markdown signal (`#`, `>`, fence, `-`/`*`/`+`, `N.`/`N)`) → verbatim
  - timestamp applied only to plain bullets, first line only
- **Edit round-trip:** load raw → re-commit reproduces the same block for every
  kind (bullet, multi-line bullet, todo, quote, numbered, code block, meeting
  heading).
- **Delete:** removes the full range; selection index clamps; surrounding lines
  preserved.
- **Resume meeting:** `Enter` on a meeting heading sets `Context::Meeting(ord)`,
  updates the context display, switches to capture; a subsequent entry lands
  under that meeting.
- **Composer input:** `Alt+Enter` inserts a newline; `Enter` commits; backspace
  removes across a newline.
- **UI smoke (`TestBackend`):** whole multi-line block highlighted in navigate
  mode; quote/code/numbered render; multi-line capture box renders and grows.

## Documentation

Update `README.md`:
- Note that plain text becomes a bullet and Markdown is passed through verbatim,
  with examples (heading, quote, numbered list, code fence).
- Document `Alt+Enter` for multi-line entry and the "Use Option as Meta key"
  requirement on macOS terminals.
- Document `Enter` on a meeting heading (navigate mode) to resume it.
- Note that all entry types are now selectable/editable/deletable.
