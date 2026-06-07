# Meeting Time Commands Design

**Date:** 2026-06-07  
**Status:** Approved

## Summary

Replace the automatic timestamp prefix on meeting headings with explicit `/start`, `/end`, and `/scheduled` commands that insert labeled time lines into the meeting note. Add a live clock to the header that updates every minute.

## Background

Currently, running `/meeting Daily Standup` at 09:15 creates:

```
### 09:15 Daily Standup
```

The time is auto-stamped into the heading with no way to record scheduled time, start time, and end time separately. This design removes the auto-stamp and adds explicit commands for recording meeting times.

## Changes

### 1. Meeting heading format

`add_meeting()` in `src/model/writer.rs` drops the `time` parameter. The call site in `src/app/actions.rs` no longer passes `&state.current_time_hhmm()`. Meetings are created as:

```
### Daily Standup
```

### 2. New commands

Three new `Command` variants in `src/app/command.rs`:

| Command | Variant | Behavior |
|---|---|---|
| `/start` | `Command::Start` | Inserts/replaces `Started: HH:MM` using current time |
| `/end` | `Command::End` | Inserts/replaces `Ended: HH:MM` using current time |
| `/scheduled HH:MM` | `Command::Scheduled(String)` | Inserts/replaces `Scheduled: HH:MM` with the provided time |

**Validation:**
- `/scheduled` without an argument, or with a malformed time → `Command::InvalidArgs("invalid time, use HH:MM")`
- Valid `HH:MM` means hours 00–23 and minutes 00–59
- All three commands require an active meeting context; if the user is in Notes context, dispatch emits a status message "Not in a meeting" and takes no other action

### 3. Time metadata block

Each meeting can have zero or more labeled time lines immediately after the `### heading`, before any note content. These lines form the "time metadata block" and are always written in canonical order:

```
### Daily Standup
Scheduled: 09:00
Started: 09:15
Ended: 09:45
- first note
- second note
```

`src/model/writer.rs` gains a new function:

```rust
pub fn set_meeting_time_field(
    lines: &mut Vec<String>,
    meeting_line_idx: usize,
    key: &str,       // "Scheduled", "Started", or "Ended"
    value: &str,     // "HH:MM"
)
```

Implementation:
1. Scan forward from `meeting_line_idx + 1` collecting consecutive `Key: HH:MM` lines (the existing metadata block)
2. Update the target key (or add it if absent)
3. Rewrite all collected lines in fixed order: `Scheduled`, `Started`, `Ended` (omitting any keys not present)

The canonical order ensures predictable document output regardless of the order commands were issued.

### 4. Live clock in the header

**`AppState`** (`src/app/state.rs`) gains:

```rust
pub last_rendered_minute: u32,
```

Initialized to `61` (sentinel value that never matches a real minute) to force a draw on first render.

**Main event loop** (`src/main.rs`): after each `event::poll(100ms)` call, check `chrono::Local::now().minute()` against `state.last_rendered_minute`. If they differ, update the field and trigger a redraw (the loop already redraws unconditionally, so this just ensures redraws happen even when no key is pressed).

**Header rendering** (`src/ui/layout.rs`): the date/context line changes from:

```
2026-06-07 (Sun)
context: Daily Standup
```

to:

```
2026-06-07 (Sun)  16:27
context: Daily Standup
```

The current time (`HH:MM`) is appended to the date line with two spaces of separation.

## Data flow

```
User types /start
  → command::parse("/start") → Command::Start
  → actions::dispatch(Command::Start, state)
      → guard: state.context == Context::Meeting(ord) else status "Not in a meeting"
      → time = state.current_time_hhmm()
      → writer::set_meeting_time_field(&mut state.doc.lines, meeting_heading_idx, "Started", &time)
      → after_doc_mutation(state)  (saves file, re-parses doc)
```

## Error handling

| Situation | Behavior |
|---|---|
| `/start` / `/end` / `/scheduled` outside meeting context | Status bar: "Not in a meeting" |
| `/scheduled` with no argument | `InvalidArgs`: "invalid time, use HH:MM" |
| `/scheduled` with malformed time (e.g. `9am`) | `InvalidArgs`: "invalid time, use HH:MM" |
| `/start` or `/end` run a second time | Overwrites the existing `Started:`/`Ended:` line |

## Files changed

| File | Change |
|---|---|
| `src/app/command.rs` | Add `Start`, `End`, `Scheduled(String)` variants; parse `/start`, `/end`, `/scheduled` |
| `src/app/actions.rs` | Dispatch new commands; remove time arg from `add_meeting` call |
| `src/model/writer.rs` | Remove time param from `add_meeting`; add `set_meeting_time_field` |
| `src/app/state.rs` | Add `last_rendered_minute: u32` field |
| `src/main.rs` | Minute-change detection and forced redraw in event loop |
| `src/ui/layout.rs` | Append current time to header date line |

## Testing

- Unit tests in `command.rs` for `/start`, `/end`, `/scheduled <time>`, `/scheduled` (no arg), `/scheduled bad`
- Unit tests in `writer.rs` for `set_meeting_time_field`: insert when absent, overwrite when present, canonical ordering, all three fields present
- Manual verification: create meeting, run commands in various orders, confirm heading has no timestamp and metadata block is correct
