# Kua-Tin

A keyboard-driven terminal UI for bullet-journal-style daily notes. Notes are stored as plain Markdown — one file per day — so they work with any editor, version control, or sync tool.

## Quick start

```bash
cargo run -- --notes-dir ~/Documents/kuatin
```

The app opens today's note, creating it from a template if it doesn't exist yet.

## How it works

### Capture mode (default)

Type at the bottom bar and press **Enter**. Plain text becomes a note entry. Use slash commands to route entries into the right section.

| Command | What it does |
|---|---|
| `/meeting "Name"` | Start a meeting context. Subsequent entries go under `### HH:MM Name` until you leave. |
| `/note` | Switch context to Notes (the default). |
| `/todo Buy milk` | Add a to-do to the central To-dos list. If you're in a meeting, it gets tagged `_(Meeting Name)_`. |
| `/leave` | Exit the current meeting context and return to Notes. |
| `/goto 2026-06-05` | Jump to a specific date. `/goto` with no args opens the calendar overlay. |
| `/today` | Jump to today's note. |
| `/help` | Show the help overlay. |
| `/quit` | Exit. |

### Navigate mode

Press **Esc** to move focus into the document. Use these keys to act on entries:

| Key | Action |
|---|---|
| `j` / `k` or `↑` / `↓` | Move selection up/down |
| `g` / `G` | Jump to first / last entry |
| `Space` or `x` | Toggle a to-do done / not done |
| `e` | Edit the selected entry |
| `d` then `d` | Delete the selected entry (two-step confirm) |
| `?` | Open help overlay |
| `i` or `Esc` | Return to capture mode |

### Global shortcuts

| Key | Action |
|---|---|
| `Ctrl-T` | Jump to today |
| `[` / `]` | Previous / next day |
| `Ctrl-G` | Open calendar overlay |
| `Ctrl-C` | Quit |

### Calendar overlay

`Ctrl-G` or `/goto` opens a month calendar. Arrow keys move the selection, `Enter` opens the chosen day, `Esc` closes the overlay. Days that already have notes are marked with a dot.

## Configuration

Create `~/.config/kuatin/config.toml`:

```toml
notes_dir = "~/Documents/kuatin"   # where daily files are stored
timestamp_entries = false          # prefix every bullet with HH:MM
week_starts_on = "sunday"          # calendar layout: sunday or monday
date_format = "%Y-%m-%d-%a"       # day-file naming pattern
```

All fields are optional. `notes_dir` can also be set via `--notes-dir <path>` on the command line.

## File format

Each day is a Markdown file named `YYYY-MM-DD-DOW.md`, e.g. `2026-06-04-Thu.md`:

```markdown
# 2026-06-04 (Thu)

## Meetings

### 09:15 Standup
- Shipped the parser refactor
- Alice is blocked on API keys

## Notes
- Coffee machine is fixed

## To-dos
- [ ] Follow up with Alice _(Standup)_
- [x] Renew SSL cert
```

Files are plain Markdown — edit them in your normal editor, sync them via git, or back them up however you like. The app preserves any extra content you add manually.

## Building

```bash
cargo build --release
```

Requires Rust 1.85+ (edition 2024).

## Architecture

- **Pure core** (`src/model/`, `src/storage/`, `src/config/`, `src/app/command.rs`, `src/app/actions.rs`) holds all behavior and is unit-tested with string/temp-dir fixtures. No terminal dependency.
- **TUI layer** (`src/ui/`) is a thin Ratatui front-end that renders state and routes key events into the core.
- **Markdown handling** uses an anchored structural model: the file is kept as a `Vec<String>` of lines; mutations splice specific lines and re-index, so untouched lines are preserved verbatim. Saves are atomic (temp file + rename).

## Future features (not yet implemented)

- Local LLM integration for note-taking assistance and `/summarize`
- Automated to-do tracking, rollover, and migration between days
- Vector index for semantic search across notes
