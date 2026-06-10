# Agenda Section in Right Panel — Design Spec

**Date:** 2026-06-10  
**Status:** Approved

## Overview

Add an "Agenda" section to the right panel that lists today's meetings which have a `Scheduled: HH:MM` metadata line, sorted earliest to latest, displayed as `HH:MM - Meeting Name`. The section appears above the existing To-do list.

## Background

Meetings are stored in the `## Meetings` section as `### Meeting Name` headings. The `/scheduled HH:MM` command inserts a `Scheduled: HH:MM` metadata line immediately after the heading. This design shows only meetings with that explicit `Scheduled:` field — not all meetings, and not meetings whose time appears only in the heading (e.g., `### 09:30 Standup`).

## Scope

- Today's document only (the currently-viewed note)
- Meetings with a `Scheduled: HH:MM` line (as written by `/scheduled`)
- Sorted by time ascending
- Section hidden when there are no scheduled meetings

## Data Model

### `Document::meetings_with_scheduled()` (new method in `src/model/writer.rs`)

Returns `Vec<(String, String)>` — `(scheduled_time, name)` pairs.

Algorithm:
1. Call `self.meetings()` to get the parsed list of `Meeting` structs (name and heading_line already extracted, avoids duplicating heading-parsing logic).
2. For each `Meeting`, scan `lines[heading_line + 1..]` for consecutive time-field lines; look for a line that starts with `Scheduled: `.
3. Include the meeting only if a `Scheduled:` line was found; use that line's value as the time.
4. Sort the resulting vec by time string (lexicographic `HH:MM` sort is correct for 24-hour times).

No changes to the existing `Meeting` struct or `meetings()` method.

## AppState

New field in `AppState` (`src/app/state.rs`):

```rust
pub panel_agenda: Vec<(String, String)>, // (scheduled_time, meeting_name)
```

Populated in `AppState::open_day()` by calling `doc.meetings_with_scheduled()`.

Helper function in `src/ui/right_panel.rs`:

```rust
pub fn collect_agenda_items(doc: &Document) -> Vec<(String, String)>
```

Thin wrapper around `doc.meetings_with_scheduled()`, kept in right_panel for symmetry with `collect_panel_todos`.

## Refresh Points

`panel_agenda` must be refreshed wherever `panel_todos` is refreshed:

- `after_doc_mutation()` in `src/app/actions.rs`
- `after_vim_edit()` in `src/app/actions.rs`
- `go_to_date()` / `go_today()` in `src/app/actions.rs`
- `AppState::open_day()` in `src/app/state.rs`

All test fixtures that construct `AppState` directly must set `panel_agenda: Vec::new()`.

## Right Panel Layout

Current: `Calendar (9 lines) → Todo list (remaining)`

New: `Calendar (9 lines) → Agenda (variable, 0 when empty) → Todo list (remaining)`

Layout strategy in `render()`:
- If `panel_agenda` is non-empty: three-way split — fixed calendar, fixed agenda block, remaining todos.
- If `panel_agenda` is empty: existing two-way split (no agenda header rendered at all).

Agenda height = `1 (header) + panel_agenda.len() (items)`.

## Rendering

New function `render_agenda()`:

```
Agenda                    ← bold header
09:30 - Standup
14:00 - Design Review
```

- Header: `"Agenda"` with `Modifier::BOLD`
- Items: `"HH:MM - Name"` in default style
- No selected/highlighted state (agenda items are not interactive in this iteration)
- Section not rendered at all when `panel_agenda` is empty

## Error Handling

- If `## Meetings` section is absent, `meetings_with_scheduled()` returns an empty vec — no agenda shown.
- Malformed `Scheduled:` lines (missing value, non-`HH:MM` format) are silently skipped.

## Testing

### Unit tests for `meetings_with_scheduled()` (in `src/model/writer.rs`):
- Meeting with no metadata → not included
- Meeting with `Scheduled:` only → included
- Meeting with `Scheduled:`, `Started:`, `Ended:` → included, only time extracted
- Multiple meetings, mixed — only those with `Scheduled:` returned
- Result is sorted by time
- Meeting whose heading has an embedded time (e.g., `### 09:30 Name`) but no `Scheduled:` line → not included

### Unit tests for `render_agenda()` (in `src/ui/right_panel.rs`):
- Empty `panel_agenda` → "Agenda" header not present in output
- Non-empty `panel_agenda` → header and items rendered correctly
- Items appear above "To-dos" header

### Integration: AppState refresh
- After `/scheduled HH:MM` command, `panel_agenda` is updated

## Out of Scope

- Interactive selection of agenda items
- Multi-day agenda lookahead
- Clicking/navigating to a meeting from the agenda
