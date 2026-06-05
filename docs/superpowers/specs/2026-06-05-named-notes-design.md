# Named Notes — Design

Date: 2026-06-05
Status: Approved

## Overview

Add `/note "Note Name"` support so that Notes can have named sub-sections in exactly the same way that Meetings do with `/meeting "Name"`.

## Behavior

- `/note "Note Name"` creates a `### Note Name` heading under `## Notes` and switches capture context to that note block, so subsequent entries are inserted under it.
- `/note` without arguments continues to reset context to the general Notes section.
- `### ` headings in the Notes section become `NoteHeading` selectables (symmetric to `MeetingHeading` in Meetings).
- In navigate mode, pressing `Enter` on a `NoteHeading` resumes that note's capture context.

## Data Model Changes

- `Command::Note` → `Note(Option<String>)`
- `Context` → add `NoteBlock(usize)`
- `EntryTarget` → add `NoteBlock(usize)`
- `SelectableKind` → add `NoteHeading { ordinal: usize }`

## Document Methods

- `note_headings()` — finds `### ` headings in the Notes section
- `add_note_heading(name)` — inserts `### Name` under `## Notes`

## UI / Help Updates

- Help text updated to show `/note "Name"` syntax
- `### ` headings in Notes rendered with the same yellow bold style as today

## Testing

- Command parse tests for quoted/unquoted note names
- Action tests for note creation, context switching, entry nesting, and re-entry
