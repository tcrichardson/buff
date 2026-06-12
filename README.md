# buff

A keyboard-driven terminal UI for bullet-journal-style daily notes. Notes are stored as plain Markdown — one file per day — so they work with any editor, version control, or sync tool.

## Quick start

```bash
cargo run -- --notes-dir ~/Documents/buff
```

The app opens today's note, creating it from a template if it doesn't exist yet.

## How it works

### Capture mode (default)

Type at the bottom bar and press **Enter**. Plain text is stored as-is. Use slash commands to route entries into the right section. Press **Esc** to switch to vim normal mode.

| Command | What it does |
|---|---|
| `/meeting "Name"` | Start a meeting context. Subsequent entries go under `### HH:MM Name` until you leave. |
| `/note "Name"` | Start a named note block context. Subsequent entries go under `### Name`. |
| `/note` | Switch context to Notes (the default). |
| `/section "Name"` | Add a sub-section one heading level deeper (max `######`). Works inside meetings and note blocks. |
| `/todo Buy milk` | Add a to-do to the central To-dos list. If you're in a meeting, it gets tagged `_(Meeting Name)_`. |
| `/leave` | Exit the current meeting or note context and return to Notes. |
| `/start` | Record the meeting start time (current HH:MM) as metadata. Also activates the meeting assistant in the chat panel. |
| `/end` | Record the meeting end time (current HH:MM) and trigger AI summary generation. |
| `/scheduled HH:MM` | Record the scheduled start time for the current meeting. |
| `/purpose text` | Record the purpose of the current meeting as metadata. |
| `/topic text` | Record the topic of the current note block as metadata. |
| `/goto 2026-06-05` | Jump to a specific date. |
| `/today` | Jump to today's note. |
| `/ask <message>` | Ask the local LLM; reply streams into the chat panel. While in meeting assistant mode (after `/start`), the LLM has the meeting context injected automatically. |
| `/clear` | Clear the current day's chat conversation. |
| `/help` | Show the help overlay. |
| `/quit` | Exit. |

### Markdown notes

Plain text is stored as-is. If you want a bullet, type it explicitly:

To create nested bullets or indented content, prefix the line with `->` (one level = 2 spaces). Stack for deeper nesting: `->->- item`. Use `Ctrl+.` to prepend `->` at the line start, or `Tab` to insert it at the cursor.

```markdown
->- Sub bullet          stored as:   - Sub bullet
->->- Deep bullet       stored as:     - Deep bullet
```

- `# Heading`, `## Subheading`, `### Heading 3`, `#### Heading 4`, `##### Heading 5`, `###### Heading 6` — headings
- `> quoted text` — blockquote
- `1. first`, `2. second` — numbered list
- `- item`, `* item`, `+ item` — bullet
- ```` ``` ```` fenced code blocks

### Multi-line notes

Press **Enter** to commit. To insert a line break within a note, press
**Ctrl+J** (the standard terminal key for a newline). Multi-line text is stored
as-is; code fences and quotes are also stored verbatim.

### Editing in the capture box

You can move the cursor within the capture box to fix typos or edit text
before committing.

| Key | Action |
|---|---|
| `←` / `→` | Move cursor one character left / right |
| `Home` / `Ctrl+A` | Jump to start of current line |
| `End` / `Ctrl+E` | Jump to end of current line |
| `Backspace` | Delete character before cursor |
| `Tab` | Insert indent marker (`->`) at cursor |
| `Ctrl+.` | Prepend indent marker (`->`) at start of current line |

### Vim normal mode

Press **Esc** from capture mode to move focus into the document and navigate with vim-style keys. A status line at the bottom shows `-- NORMAL --` and the current context. The current line is highlighted and the cursor is shown as a solid block.

| Key | Action |
|---|---|
| `j` / `k` or `↑` / `↓` | Move cursor up/down |
| `h` / `l` or `←` / `→` | Move cursor left/right |
| `w` / `b` | Move to start of next / previous word |
| `e` | Move to end of current/next word |
| `0` / `$` | Jump to start / end of line |
| `gg` / `G` | Jump to first / last line |
| `t` | Toggle a to-do done / not done |
| `x` | Delete character under cursor |
| `dd` | Delete the current line (also yanks it) |
| `yy` | Yank (copy) the current line |
| `p` | Paste yanked line below cursor |
| `P` | Paste yanked line above cursor |
| `u` | Undo last edit |
| `i` | Enter insert mode at cursor |
| `a` | Enter insert mode after cursor |
| `A` | Enter insert mode at end of line |
| `o` | Insert new line below and enter insert mode |
| `O` | Insert new line above and enter insert mode |
| `Enter` | Open the current line for editing in the capture box |
| `?` | Open help overlay |
| `Esc` | Return to capture mode |

### Vim insert mode

Press **i** from normal mode to edit the document directly at the cursor position. The status line shows `-- INSERT --` and the cursor is shown as a vertical bar.

| Key | Action |
|---|---|
| `←` / `→` / `↑` / `↓` | Move cursor |
| `Backspace` | Delete character before cursor |
| `Enter` | Insert newline |
| `Tab` | Insert two spaces |
| `Ctrl+W` | Delete word before cursor |
| Any character | Insert at cursor |
| `Esc` | Return to normal mode |

All entry types — plain text, bullets, to-dos, meeting headings, and Markdown blocks — are selectable, editable (`Enter` or `i`), and deletable (`dd`).

### Focus cycle

`Tab` and `Shift+Tab` cycle focus across the UI:

| From | `Tab` goes to | `Shift+Tab` goes to |
|---|---|---|
| Vim normal mode | Right panel | Right panel |
| Chat panel | Right panel | Vim normal mode |
| Right panel | Vim normal mode | Vim normal mode |

`Esc` returns to vim normal mode from most contexts; from vim normal mode it switches back to capture mode.

### Right panel

A persistent panel on the right side of the terminal shows three sections stacked vertically:

1. **Calendar** — current month with today highlighted and days that have notes marked with `·`
2. **Agenda** — meetings with a scheduled time (`/scheduled HH:MM`) for today's note, shown when present
3. **To-do list** — all incomplete to-dos from the last 7 days, grouped by date

| Key | Action |
|---|---|
| `Tab` (in vim normal mode) | Move focus into the right panel |
| `j` / `k` or `↑` / `↓` | Navigate between to-dos in the panel |
| `Space` or `x` | Toggle the selected to-do done |
| `Esc` or `Tab` | Return focus to the document |

Toggling a to-do from the panel updates the source day's file immediately. If the to-do belongs to today's note, the left document view also updates in place.

### Chat panel

A middle panel streams replies from a local LM Studio server (or any OpenAI-compatible endpoint). Conversations are saved per-day in `.chat.json` sidecars, so they survive restarts.

| Key | Action |
|---|---|
| `Ctrl-L` | Show / hide the chat panel |
| `/ask <message>` | Send a message; the reply streams token-by-token |
| `/clear` | Erase the current day's conversation |
| `j` / `k` or `↑` / `↓` | Scroll the chat history one line |
| `PageUp` / `PageDown` | Scroll the chat history ten lines |
| `Esc` or `Tab` | Return focus to the document |

The chat panel is visible by default. If LM Studio is not running, a red error line appears and the app stays responsive.

### Global shortcuts

| Key | Action |
|---|---|
| `Ctrl-T` | Jump to today |
| `[` / `]` | Previous / next day |
| `Ctrl-C` | Quit |

## Configuration

Create `~/.config/buff/config.toml`:

```toml
notes_dir = "~/Documents/buff"   # where daily files are stored
timestamp_entries = false          # prefix every entry with HH:MM
week_starts_on = "sunday"          # calendar layout: sunday or monday
date_format = "%Y-%m-%d-%a"       # day-file naming pattern
panel_width = 30                   # right panel width in terminal columns (or "25%" for percentage)
todo_lookback_days = 7             # days to scan for incomplete to-dos
capture_height = 5                 # height of the capture bar in rows

# Chat / LLM settings
llm_base_url = "http://localhost:1234/v1"   # OpenAI-compatible local server
llm_model = "google/gemma-4-12b-qat"        # model id served by LM Studio
llm_system_prompt = ""                       # optional system prompt for general chat
llm_api_key = ""                             # API key (leave empty for local servers)
chat_visible = true                          # show the chat panel on startup

# Theme
theme = "light"                              # "light" or "dark"

[theme_overrides]
heading1 = "red"
border_focused = "#0000ff"
```

All fields are optional. `notes_dir` can also be set via `--notes-dir <path>` on the command line.

### Themes

buff ships with two built-in themes:

| Theme | Description |
|---|---|
| `light` (default) | Clean light-blue focused borders with colored headings |
| `dark` | Cyan-focused borders with white headings for dark terminals |

Set the theme with the `theme` config field. You can override any individual color via the `[theme_overrides]` table:

| Override key | Default (light) | Example values |
|---|---|---|
| `heading1` | `black` | `"red"`, `"#ff0000"` |
| `heading2` | `#0277bd` | `"cyan"`, `"#00bcd4"` |
| `heading3` | `#e65100` | `"yellow"`, `"#ff9800"` |
| `heading4` | `#6a1b9a` | `"magenta"`, `"#9c27b0"` |
| `heading5` | `#2e7d32` | `"green"`, `"#4caf50"` |
| `heading6` | `darkgray` | `"gray"`, `"#757575"` |
| `border_focused` | `#0277bd` | `"cyan"`, `"#0288d1"` |
| `border_unfocused` | `darkgray` | `"gray"`, `"#9e9e9e"` |
| `notes_panel_bg` | `reset` | `"white"`, `"#fafafa"` |
| `panel_bg` | `#dde8f5` | `"lightgray"`, `"#e3f2fd"` |
| `chat_panel_bg` | `#e6e6f0` | `"lightgray"`, `"#f3e5f5"` |
| `quote_marker` | `#7b1fa2` | `"magenta"`, `"#ab47bc"` |
| `code` | `darkgray` | `"gray"`, `"#616161"` |
| `todo_done` | `green` | `"lightgreen"`, `"#66bb6a"` |
| `todo_overdue` | `red` | `"lightred"`, `"#ef5350"` |
| `vim_cursor_line` | `#dbeafe` | `"lightgray"`, `"#e3f2fd"` |
| `capture_bg` | `reset` | `"white"`, `"#fafafa"` |
| `metadata` | `darkgray` | `"gray"`, `"#757575"` |
| `terminal_bg` | `reset` | `"black"`, `"#121212"` |
| `terminal_fg` | `reset` | `"white"`, `"#e0e0e0"` |

> **Note:** The `dark` theme sets `terminal_bg` to `#121212` and `terminal_fg` to `white` by default, so it renders correctly on terminals with a light background. Override these in `[theme_overrides]` to customise or restore terminal-inherited colours (`reset`).

Colors can be specified as:
- **Named colors**: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `gray`, `darkgray`, `white`, `reset`
- **Hex colors**: `#rrggbb` (e.g., `#0277bd`)
- **Case-insensitive**: `Cyan`, `DARK_GRAY`, `DarkGray` all work

## File format

Each day is a Markdown file named `YYYY-MM-DD-DOW.md`, e.g. `2026-06-04-Thu.md`:

```markdown
# 2026-06-04 (Thu)

## Meetings

### 09:15 Standup
Shipped the parser refactor
- Alice is blocked on API keys

## Notes
Coffee machine is fixed

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
- **Input layer** (`src/app/input/`) translates raw `crossterm` key events into typed `UiAction` values, dispatched per focus mode (capture, vim normal, vim insert, right panel, chat). Global hotkeys and focus-cycle keys are resolved before mode dispatch.
- **LLM layer** (`src/app/llm.rs`) spawns background threads for streaming OpenAI-compatible requests and sends `LlmEvent` values back to the event loop via an `mpsc` channel. Meeting assistant mode injects live meeting context before each request.
- **TUI layer** (`src/ui/`) is a thin Ratatui front-end that renders state and routes key events into the core. The right panel (`src/ui/right_panel.rs`) is a self-contained module handling calendar rendering, agenda display, todo collection, and panel display.
- **Markdown handling** uses an anchored structural model: the file is kept as a `Vec<String>` of lines; mutations splice specific lines and re-index, so untouched lines are preserved verbatim. Saves are atomic (temp file + rename).
- **Vim editing** is built on top of the line-based model: `VimState` tracks cursor position, undo history, and yank buffer; normal mode provides line-oriented operations (`dd`, `yy`, `p`, `u`) while insert mode allows direct character editing.

## Future features (not yet implemented)

- `/summarize` command for arbitrary note summarization (the meeting assistant's `/end` auto-summary is already implemented)
- Automated to-do rollover and migration between days
- Vector index for semantic search across notes
