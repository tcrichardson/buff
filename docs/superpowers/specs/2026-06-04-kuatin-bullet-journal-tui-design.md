# Kua-Tin — Bullet Journal TUI — Design

Date: 2026-06-04
Status: Approved (pending implementation plan)

## Overview

Kua-Tin is a Rust terminal UI (TUI) application for bullet-journal-style daily
note taking. Notes are stored as plain Markdown files, one per day. Users
capture entries through a command bar that routes them into the correct section
of the day's note based on an active "context" set by slash commands.

The application is keyboard-driven and optimized for fast capture. It preserves
the user's Markdown files faithfully so they can also be edited in an external
editor or tracked in git.

### Goals (v1)

- Fast, keyboard-driven capture of daily notes into structured Markdown.
- Stateful "active context" capture: a slash command sets where subsequent
  plain entries go; entries flow there until the context changes.
- Three fixed sections per day: Meetings, Notes, To-dos.
- Named meeting sub-headings with start times.
- A single canonical to-do list per day, with provenance tags for to-dos
  captured during meetings.
- Toggle to-dos done/undone, and edit/delete entries from within the TUI
  (a navigable document pane can select any entry).
- Calendar overlay for navigating between days, highlighting days that have
  notes.
- Robust, non-destructive Markdown read/write that survives external edits.

### Non-Goals (explicitly deferred)

- Local LLM features (note-taking assistance, `/summarize`).
- Automated to-do tracking, rollover, or migration between days.
- Vector index / semantic search across notes.
- Configurable section names (sections are fixed in v1).
- Full Markdown rendering (tables, images, inline HTML).
- Multi-day / full-text search.

These are future features. The v1 architecture keeps the core model and storage
layers pure and reusable so these can be added later without rework.

## Architecture

### Technology choices

- **TUI framework: Ratatui + crossterm.** De-facto standard Rust TUI stack;
  immediate-mode rendering, strong widget/layout primitives (for the document
  pane and calendar overlay), large ecosystem, actively maintained.
- **Markdown handling: anchored structural model.** The day's file is parsed
  into a lightweight model (the three sections, meeting sub-headings, entries,
  and to-do items), where each element tracks its line range in the original
  source text. New entries are inserted under the correct heading; to-do
  toggles and edits/deletes are surgical line edits. Untouched regions are
  written back verbatim, so external formatting and content are preserved. A
  canonical template is generated only when a brand-new day file is created.

  This is preferred over a full Markdown AST round-trip (which would normalize
  and reflow the whole file on every save, fighting the user's manual edits) and
  over a pure text-append approach (which would make to-do toggling, editing,
  and the navigable pane awkward).

### Module structure

Units are kept small and independently testable. The `model` and `storage`
layers are pure (no terminal dependency) to isolate future LLM/vector work.

```
src/
  main.rs          # entry point, CLI parsing, terminal setup/teardown
  config.rs        # load/resolve config + paths
  model/
    day.rs         # DayNote, Section, Meeting, Entry, TodoItem + line ranges
    parser.rs      # markdown text  -> model (tolerant)
    writer.rs      # surgical edits + atomic save (model/edits -> text)
  app/
    state.rs       # AppState: current day, active context, focus mode, selection
    command.rs     # slash-command parsing + dispatch
    actions.rs     # mutations: add entry, add todo, toggle, edit, delete
  ui/
    layout.rs      # ratatui frame composition
    document.rs    # rendered document pane + selection
    capture.rs     # capture bar + status line
    calendar.rs    # calendar overlay widget
  storage.rs       # list day files, existence checks (for calendar marks), path<->date
```

## Data Model

Parsed from the day's Markdown file. Each element tracks its line range in the
source text to enable surgical edits.

- **`DayNote`** — date, file path, original text, and the three sections.
- **`Section`** (Meetings / Notes / To-dos) — fixed set for v1; each has its
  heading line and child entries.
- **`Meeting`** — a sub-heading under Meetings of the form `### HH:MM Name`,
  containing `Entry` bullets.
- **`Entry`** — a bullet line of plain text.
- **`TodoItem`** — a checkbox bullet with `done: bool` and an optional
  `_(Meeting Name)_` provenance tag.

### Active-context state

Lives in the application, not in the file. Tracks where new plain entries are
routed:

- `Notes` (default)
- `Meeting(name)`
- `Todos`

The active context is shown persistently in the UI status line.

## Markdown Format

### File naming & location

- One file per day, named `YYYY-MM-DD-DOW.md` (e.g. `2026-06-04-Thu.md`).
- Stored in a notes directory resolved in priority order:
  1. CLI flag (`--notes-dir`)
  2. `~/.config/kuatin/config.toml`
  3. Built-in default: `~/Documents/kuatin/`
- The notes directory and config directory are created on first run if missing.

### Canonical day template

Generated only when a new day file is first opened:

```markdown
# 2026-06-04 (Thu)

## Meetings

## Notes

## To-dos
```

### Example populated day

```markdown
# 2026-06-04 (Thu)

## Meetings

### 09:15 Standup
- Shipped the parser refactor
- Alice is blocked on the API keys

### 11:00 Design Review
- Approved the calendar overlay approach

## Notes
- Coffee machine is fixed
- Idea: cache parsed notes in memory

## To-dos
- [ ] Follow up with Alice on API keys _(Standup)_
- [ ] Send design notes to the team _(Design Review)_
- [x] Renew SSL cert
```

## Slash Commands (v1)

| Command | Effect |
|---|---|
| `/meeting "Name"` | Create a `### HH:MM Name` sub-heading under Meetings and set it as the active context. Plain entries append as bullets under it. |
| `/note` | Set active context to Notes (the default). |
| `/todo <text>` | Add `- [ ] <text>` to the To-dos section. If a meeting is the active context, append ` _(Meeting Name)_`. Does **not** change the active context. |
| `/leave` | Exit the current meeting context, returning to Notes. |
| `/goto [YYYY-MM-DD]` | Open the calendar overlay (no arg) or jump directly to a date (with arg). |
| `/today` | Jump to today's note. |
| `/help` | Show command reference. |
| `/quit` | Exit the app. |

- Plain text with no leading slash is appended as a bullet to the active
  context.
- `/summarize` is **reserved but not implemented** in v1 (the future LLM hook).

### Key behaviors flagged during design

- The three sections are **fixed** in v1; configurable sections are deferred.
- `/todo` keeps the user in their current context rather than switching to
  To-dos, so an action item can be fired off mid-meeting while continuing to
  take meeting notes.

## UI Design

### Main screen layout

```
┌─ Kua-Tin ──────────────────────────── 2026-06-04 (Thu) ─┐
│ ## Meetings                                             │
│   ### 09:15 Standup                                     │
│   • Shipped the parser refactor                         │
│   • Alice is blocked on the API keys                    │
│   ### 11:00 Design Review                               │
│   • Approved the calendar overlay approach              │
│                                                         │
│ ## Notes                                                │
│   • Coffee machine is fixed                             │
│   • Idea: cache parsed notes in memory                  │
│                                                         │
│ ## To-dos                                               │
│   ☐ Follow up with Alice on API keys (Standup)          │
│   ☐ Send design notes to the team (Design Review)       │
│   ☑ Renew SSL cert                                      │
│                                                         │  ← scrollable document pane
├─────────────────────────────────────────────────────────┤
│ context: Standup                          [? help]      │  ← status line (active context)
│ › follow up with the vendor_                            │  ← capture bar (input)
└─────────────────────────────────────────────────────────┘
```

### Focus modes

- **Capture mode (default).** The capture bar has focus. The user types
  entries/commands and presses Enter. The status line always shows the active
  context (e.g. `context: Notes` or `context: Standup`).
- **Navigate mode.** Press `Esc` (or `Tab`) to move focus into the document
  pane. A highlight cursor selects entries; `j/k` or arrows move between them.
  `Esc`/`i` returns to capture mode.

### Navigate-mode keys (operate on the selected entry)

| Key | Action |
|---|---|
| `Space` / `x` | Toggle a to-do done/undone (only on to-do items) |
| `e` | Edit the selected entry's text (loads it into the capture bar for editing; Enter saves) |
| `d` | Delete the selected entry (with a confirm prompt) |
| `j` / `k` / `↑` / `↓` | Move selection |
| `g` / `G` | Jump to top / bottom |
| `?` | Show help (the `[? help]` hint in the status line) |

### Global keys (work in capture mode too)

| Key | Action |
|---|---|
| `Ctrl-T` | Jump to today |
| `[` / `]` | Previous / next day |
| `Ctrl-G` | Open calendar overlay |
| `Ctrl-C` | Quit |

### Calendar overlay (opened via `Ctrl-G` or `/goto`)

```
        ┌─ Go to date ──────────────┐
        │      June 2026            │
        │ Su Mo Tu We Th Fr Sa      │
        │  1  2  3 [4] 5  6  7       │   • = day has a note
        │  8  9 10 11 12 13 14•      │   [ ] = selected day
        │ 15 16 17•18 19 20 21       │
        │ 22 23 24 25 26 27 28       │
        │ 29 30                      │
        │ ←/→ day  ↑/↓ week          │
        │ Enter open · Esc cancel    │
        └───────────────────────────┘
```

- Days that already have a note file are visually marked.
- Arrow keys move the selection across days and months.
- `Enter` opens the selected day (creating the file from the template if it
  does not exist).
- `Esc` cancels.

### Markdown rendering

The document pane shows a lightly styled view (headings bold/colored,
`- [ ]` → `☐`, `- [x]` → `☑`, bullets → `•`), a faithful 1:1 reflection of the
file structure — not a full Markdown renderer (no tables/images). A
raw-Markdown toggle may come later.

## Configuration

`~/.config/kuatin/config.toml`, all fields optional with defaults:

```toml
notes_dir         = "~/Documents/kuatin"   # where day files live
timestamp_entries = false                  # if true, prefix every bullet with HH:MM
week_starts_on    = "sunday"               # calendar layout: sunday | monday
date_format       = "%Y-%m-%d-%a"          # day-file naming (default -> 2026-06-04-Thu)
```

Resolution order: CLI flag (`--notes-dir`) → config file → built-in default.

Note: `timestamp_entries` controls whether individual note/todo bullets are
prefixed with `HH:MM`. Meeting sub-headings always record their start time
(`### HH:MM Name`) regardless of this setting.

## Error Handling

The application must never lose notes or crash on bad input.

- **Capture errors** (unknown command, malformed `/meeting` with no name) →
  non-destructive inline message in the status line; the input stays so the
  user can fix it.
- **Parse tolerance** — if a day file contains unexpected Markdown (from
  external edits), unrecognized lines are preserved verbatim and attached to the
  nearest preceding section so saving never drops content. If the three
  canonical headings are missing, they are treated as empty and only added when
  the user actually writes into them.
- **Atomic disk writes** — write to a temp file in the same directory, then
  rename over the target, so an interrupted save cannot corrupt a note. Saves
  happen on each committed entry/edit.
- **Startup/IO failures** (unreadable directory, permissions) → clear error to
  stderr and a non-zero exit, rather than a half-broken TUI.

## Testing Strategy

- **Unit tests** on the pure core (no terminal):
  - `parser` — round-trips, tolerant parsing of messy/externally-edited input.
  - `writer` — surgical insert/toggle/edit/delete preserves untouched lines;
    atomic write behavior.
  - `command` — slash-command parsing (including `/meeting "Name"` quoting).
  - `actions` — active-context routing, meeting-tagged to-dos.
- **Date/storage tests** — filename↔date mapping; calendar note-existence marks.
- **Light UI smoke tests** via ratatui's `TestBackend` — render key states
  (empty day, populated day, calendar overlay) and assert on the buffer.
- TDD throughout for the model/command/action layers.

## Forward Compatibility (not built in v1)

- The `model` and `storage` modules are pure and reusable: a later
  `summarize`/LLM module can consume a `Meeting`/`DayNote` directly, and a
  vector-index module can iterate day files via `storage`.
- `/summarize` is reserved in the command parser.
- No LLM or vector dependencies enter v1.

## Cleanup note

The repository currently contains a stray file named `1` (an accidental
duplicate of `Cargo.toml`) that should be removed during implementation.
