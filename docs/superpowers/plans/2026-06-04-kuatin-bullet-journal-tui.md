# Kua-Tin Bullet Journal TUI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Tasks use checkbox (`- [ ]`) syntax for tracking.
>
> **Note:** This is a higher-level plan (per user request). Each task lists files, the key types/signatures it introduces, the behavior to implement, and the test cases to write. Follow TDD: write the listed tests first, watch them fail, implement, watch them pass, then commit. Keep files small and focused.

**Goal:** A keyboard-driven Rust TUI for bullet-journal daily notes stored as Markdown, with active-context capture via slash commands and a calendar overlay for navigation.

**Architecture:** A pure, terminal-free core (`storage`, `config`, `model`, `command`, `actions`) holds all behavior and is unit-tested with string/temp-dir fixtures. A thin Ratatui front-end (`ui`, `main`) renders state and routes key events into the core. Markdown is handled with an anchored model: the file is kept as a `Vec<String>` of lines; mutations splice specific lines and re-index, so untouched lines are preserved verbatim. Saves are atomic (temp file + rename).

**Tech Stack:** Rust (edition 2024), ratatui 0.30 (+ its re-exported crossterm), chrono 0.4, serde 1, toml 1, directories 6, shellexpand 3, anyhow 1; tempfile 3 (dev).

**Reference spec:** `docs/superpowers/specs/2026-06-04-kuatin-bullet-journal-tui-design.md`

---

## Shared type contracts (define once, used across tasks)

These names/signatures MUST stay consistent across tasks.

```rust
// model/day.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SectionKind { Meetings, Notes, Todos }

// Where a plain entry is routed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntryTarget { Notes, Meeting(usize) } // Meeting(ordinal among meetings)

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SelectableKind { Entry, Todo { done: bool } }

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Selectable { pub line: usize, pub kind: SelectableKind, pub text: String }

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Meeting { pub ordinal: usize, pub heading_line: usize, pub time: String, pub name: String }

pub struct Document { lines: Vec<String> }

// app/command.rs
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    Entry(String),
    Meeting(String),
    Note,
    Todo(String),
    Leave,
    Goto(Option<chrono::NaiveDate>),
    Today,
    Help,
    Quit,
    Summarize,          // reserved; dispatch returns "not implemented yet"
    Unknown(String),    // unrecognized /word
    InvalidArgs(String) // e.g. "/meeting" with no name (message is human-readable)
}

// app/state.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus { Capture, Navigate }

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context { Notes, Meeting(usize) } // mirrors EntryTarget

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Overlay { None, Calendar, Help }
```

---

## Task 1: Project setup

**Files:** `Cargo.toml`, `src/main.rs`, delete stray `1`

- [ ] Rename package to `name = "kua-tin"` (lowercase, conventional binary name); keep `edition = "2024"`.
- [ ] Add dependencies listed in Tech Stack; add `tempfile = "3"` under `[dev-dependencies]`.
- [ ] Create empty module files so the tree compiles: `src/config.rs`, `src/storage.rs`, `src/model/mod.rs` (re-exports `day`, `parser`, `writer`), `src/model/day.rs`, `src/model/parser.rs`, `src/model/writer.rs`, `src/app/mod.rs` (re-exports `state`, `command`, `actions`), `src/app/state.rs`, `src/app/command.rs`, `src/app/actions.rs`, `src/ui/mod.rs` (re-exports `layout`, `document`, `capture`, `calendar`), `src/ui/layout.rs`, `src/ui/document.rs`, `src/ui/capture.rs`, `src/ui/calendar.rs`. Declare modules in `main.rs`.
- [ ] Delete the stray `1` file.
- [ ] Verify: `cargo build` succeeds and `cargo test` runs (zero tests).
- [ ] Commit: `chore: scaffold modules and dependencies`.

---

## Task 2: storage — date <-> filename and file discovery

**Files:** `src/storage.rs` (+ tests inline)

Implement pure helpers:
- `pub fn file_name_for(date: NaiveDate, date_format: &str) -> String` → `date.format(date_format)` + `".md"`.
- `pub fn path_for(notes_dir: &Path, date: NaiveDate, date_format: &str) -> PathBuf`.
- `pub fn note_exists(notes_dir, date, date_format) -> bool`.
- `pub fn dates_with_notes(notes_dir, date_format) -> BTreeSet<NaiveDate>` — read dir, parse each `*.md` filename back to a date via `NaiveDate::parse_from_str(stem, date_format)`, ignore non-matching files.

**Tests:** filename for a known date with default format `%Y-%m-%d-%a` equals `2026-06-04-Thu.md`; round-trip parse of that stem yields the date; `dates_with_notes` over a `tempfile::tempdir()` containing two valid files plus one junk file returns exactly the two dates.

- [ ] TDD each helper. Commit: `feat: storage date/filename mapping and note discovery`.

---

## Task 3: config — Config struct, defaults, and notes-dir resolution

**Files:** `src/config.rs`

```rust
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub notes_dir: Option<String>, // raw, may contain ~
    pub timestamp_entries: bool,
    pub week_starts_on: WeekStart, // Sunday | Monday (serde rename lowercase)
    pub date_format: String,       // default "%Y-%m-%d-%a"
}
```
- `Default` impl: `timestamp_entries=false`, `week_starts_on=Sunday`, `date_format="%Y-%m-%d-%a"`, `notes_dir=None`.
- `pub fn config_path() -> PathBuf` via `directories::ProjectDirs::from("", "", "kuatin")` → `config.toml`.
- `pub fn default_notes_dir() -> PathBuf` via `directories::UserDirs::document_dir()` joined with `kuatin`, falling back to home/`kuatin` if no document dir.
- `pub fn load(cli_notes_dir: Option<String>) -> anyhow::Result<(Config, PathBuf)>` — read+parse config file if present (missing file = defaults), then resolve effective notes dir: `cli_notes_dir` > `config.notes_dir` > `default_notes_dir()`, expanding `~` with `shellexpand::tilde`. Returns the resolved absolute notes-dir path alongside the config.

**Tests:** parsing a TOML string sets fields; missing fields fall back to defaults; resolution precedence (cli beats config beats default); `~/foo` expands. Use string inputs / a pure `resolve_notes_dir(cli, cfg_value, default)` helper so tests need no real filesystem for precedence.

- [ ] TDD. Commit: `feat: config loading and notes-dir resolution`.

---

## Task 4: Document core — construct, render text, locate sections

**Files:** `src/model/day.rs`, `src/model/parser.rs`

- `Document::from_text(&str) -> Document` (split on `\n`, drop a single trailing empty element from the final newline).
- `Document::to_text(&self) -> String` (join with `\n`, ensure exactly one trailing `\n`).
- `Document::new_for_date(date, date_format_for_title) -> Document` produces the canonical template lines:
  `# {YYYY-MM-DD} ({Day})`, ``, `## Meetings`, ``, `## Notes`, ``, `## To-dos`. (Title uses `date.format("%Y-%m-%d (%a)")` regardless of file date_format.)
- Internal helpers (in `parser.rs`, operating on `&[String]`): `heading_line(lines, SectionKind) -> Option<usize>` (matches exact `## Meetings`/`## Notes`/`## To-dos`); `section_end(lines, start) -> usize` (first later line starting with `## `, else `lines.len()`); `block_insert_index(lines, start_excl, end_excl) -> usize` (after the last non-blank line in the half-open range, else `start_excl`).

**Tests:** `from_text`→`to_text` round-trips a populated example verbatim (including a trailing-newline normalization); `new_for_date(2026-06-04)` text equals the canonical template with title `# 2026-06-04 (Thu)`; `heading_line` finds each section; `block_insert_index` returns expected indices for empty vs populated sections.

- [ ] TDD. Commit: `feat: document model, template, and section locators`.

---

## Task 5: Document mutations — add_meeting and add_entry

**Files:** `src/model/writer.rs` (impl on `Document`), `src/model/day.rs`

- `pub fn meetings(&self) -> Vec<Meeting>` — scan Meetings section for `### HH:MM Name` lines (tolerate `### Name` with no time → empty time); ordinal = order of appearance.
- `pub fn add_meeting(&mut self, time: &str, name: &str) -> usize` — insert `### {time} {name}` at end of Meetings block (via `block_insert_index`); return the new meeting's ordinal.
- `pub fn add_entry(&mut self, target: &EntryTarget, text: &str, time: Option<&str>)` — bullet text is `- {text}` or `- {time} {text}` when `time` is `Some`. For `EntryTarget::Notes`, insert at end of Notes block. For `EntryTarget::Meeting(ord)`, find that meeting's block (heading line → next `### `/`## `/EOF) and insert at its end.

**Tests:** adding a meeting to an empty doc yields `### 09:15 Standup` under Meetings with ordinal 0; adding two meetings gives ordinals 0,1 in order; `add_entry(Notes, "hi", None)` appends `- hi` under Notes and leaves other sections untouched; `add_entry(Meeting(0), "point", None)` nests `- point` under the first meeting, not the second; `time=Some("09:20")` produces `- 09:20 point`; untouched lines are byte-identical after each op.

- [ ] TDD. Commit: `feat: add meetings and route entries to sections`.

---

## Task 6: Document mutations — add_todo with meeting provenance

**Files:** `src/model/writer.rs`

- `pub fn add_todo(&mut self, text: &str, meeting_name: Option<&str>)` — insert `- [ ] {text}` at end of To-dos block; when `meeting_name` is `Some(n)`, append ` _({n})_`.

**Tests:** standalone todo → `- [ ] Renew cert`; meeting todo → `- [ ] Follow up _(Standup)_`; todos always land in To-dos section regardless of any current meeting content; ordering preserved when multiple todos added.

- [ ] TDD. Commit: `feat: add todos with optional meeting tag`.

---

## Task 7: Document — selectables and toggle_todo

**Files:** `src/model/writer.rs`, `src/model/parser.rs`

- `pub fn selectables(&self) -> Vec<Selectable>` — in file order, every line matching `- [ ] `/`- [x] ` → `Todo { done }`, every other `- ` bullet → `Entry`. `text` excludes the marker (keep any trailing `_(tag)_` as part of text). `line` is the source line index. (Headings are NOT selectable.)
- `pub fn toggle_todo(&mut self, sel_index: usize) -> anyhow::Result<()>` — resolve the selectable; if `Todo`, flip `[ ]`↔`[x]` on its line; if `Entry`, return an error (caller surfaces a status message).

**Tests:** selectables over the spec example returns the expected ordered list with correct kinds/done flags/text; toggling an unchecked todo checks it and vice-versa; toggling an `Entry` selectable returns `Err`; non-todo lines are unchanged.

- [ ] TDD. Commit: `feat: enumerate selectables and toggle todos`.

---

## Task 8: Document — edit_selectable and delete_selectable

**Files:** `src/model/writer.rs`

- `pub fn edit_selectable(&mut self, sel_index, new_text: &str) -> anyhow::Result<()>` — replace the content after the marker with `new_text`, preserving the marker: `- ` for entries, `- [ ] `/`- [x] ` (current done state) for todos.
- `pub fn delete_selectable(&mut self, sel_index) -> anyhow::Result<()>` — remove that single line.

**Tests:** editing an entry keeps the `- ` prefix and swaps text; editing a checked todo keeps `- [x] ` and swaps text; deleting the middle selectable removes exactly that line and shifts later selectable indices/line numbers correctly; out-of-range index returns `Err`.

- [ ] TDD. Commit: `feat: edit and delete selectable entries`.

---

## Task 9: Tolerant parsing — ensure missing sections on write

**Files:** `src/model/writer.rs`, `src/model/parser.rs`

- `fn ensure_section(&mut self, SectionKind) -> usize` — if the section heading is missing, append a blank line (if the file doesn't already end blank) plus the `## ` heading at EOF, then return its line index. Used by `add_entry`/`add_todo`/`add_meeting` before inserting.
- Confirm parser tolerance: unknown/extra lines (e.g. a stray paragraph, an extra `###`) are never dropped by any mutation.

**Tests:** starting from a file missing `## To-dos`, `add_todo` creates the heading and the item without disturbing existing content; a file with arbitrary extra prose round-trips unchanged after an unrelated `add_entry`; adding to a completely empty document creates only the section(s) it writes to.

- [ ] TDD. Commit: `feat: tolerant section creation on write`.

---

## Task 10: Command parsing

**Files:** `src/app/command.rs`

- `pub fn parse(input: &str) -> Command`:
  - Trim. No leading `/` → `Entry(trimmed)`.
  - `/meeting <name>`: strip surrounding quotes from name; empty → `InvalidArgs("/meeting needs a name")`.
  - `/note` → `Note`; `/leave` → `Leave`; `/today` → `Today`; `/help` → `Help`; `/quit` → `Quit`; `/summarize` → `Summarize`.
  - `/todo <text>`: empty → `InvalidArgs("/todo needs text")` else `Todo(text)`.
  - `/goto` no arg → `Goto(None)`; with arg → parse `%Y-%m-%d`, ok → `Goto(Some)`, bad → `InvalidArgs("invalid date, use YYYY-MM-DD")`.
  - Other `/word` → `Unknown(word)`.

**Tests:** each command variant including `/meeting "Daily Standup"` and `/meeting Daily Standup` both yield `Meeting("Daily Standup")`; empty meeting/todo → `InvalidArgs`; `/goto 2026-01-02` parses, `/goto nope` → `InvalidArgs`; plain text → `Entry`; `/bogus` → `Unknown("bogus")`.

- [ ] TDD. Commit: `feat: slash command parser`.

---

## Task 11: AppState + capture dispatch (no terminal)

**Files:** `src/app/state.rs`, `src/app/actions.rs`

- `AppState` fields: `doc: Document`, `date: NaiveDate`, `notes_dir: PathBuf`, `config: Config`, `context: Context`, `focus: Focus`, `selected: usize`, `status: String`, `input: String`, `overlay: Overlay`, `editing: Option<usize>`, `should_quit: bool`.
- `AppState::open_day(notes_dir, config, date) -> Result<Self>` — load file text if it exists else `Document::new_for_date`; context = Notes, focus = Capture, selected = 0, overlay = None.
- `fn save(&self) -> Result<()>` — atomic write: write `doc.to_text()` to `{path}.tmp` then `fs::rename` over the real path; create `notes_dir` if missing.
- `fn current_time_hhmm(&self) -> String` — `chrono::Local::now().format("%H:%M")`.
- `pub fn dispatch(&mut self, cmd: Command) -> Result<()>` for capture-side commands:
  - `Entry(t)`: skip if empty; route by `context` (Notes → `add_entry(Notes,..)`; Meeting(o) → `add_entry(Meeting(o),..)`), passing a timestamp when `config.timestamp_entries`; save.
  - `Meeting(name)`: `o = add_meeting(now, name)`; set `context = Meeting(o)`; save.
  - `Note`: `context = Notes`.
  - `Todo(t)`: meeting tag = current meeting's name when in a meeting; `add_todo`; save (context unchanged).
  - `Leave`: `context = Notes`.
  - `Today`/`Goto`: handled in Task 13 (set status "…").
  - `Help`/`Quit`/`Summarize`/`Unknown`/`InvalidArgs`: set `status` appropriately; `Quit` sets `should_quit`; `Summarize` sets "summarize is not implemented yet".

**Tests (drive dispatch, assert `doc.to_text()` and `context`):** typing two plain lines appends two Notes bullets; `/meeting "Standup"` then a line nests the bullet under the meeting and sets context; `/todo x` while in a meeting writes `- [ ] x _(Standup)_` and keeps context = Meeting; `/leave` resets context to Notes; `/note` resets context; empty entry is a no-op; `/summarize` sets the not-implemented status. Use a `tempdir` notes dir and assert the file on disk matches after save.

- [ ] TDD. Commit: `feat: app state and capture-mode dispatch`.

---

## Task 12: Navigate-mode actions

**Files:** `src/app/actions.rs`, `src/app/state.rs`

- Navigation helpers: `select_next`/`select_prev` (clamp to `selectables().len()`), `select_first`/`select_last`.
- `toggle_selected()` → `doc.toggle_todo(selected)`, save; on `Err`, set status "not a to-do".
- `delete_selected()` → `doc.delete_selectable(selected)`, clamp `selected`, save.
- `begin_edit_selected()` → set `editing = Some(selected)`, load selectable text into `input`, `focus = Capture`.
- `commit_edit()` → `doc.edit_selectable(idx, &input)`, clear `editing`/`input`, save, return to the entry (`focus = Navigate`).

**Tests:** toggle flips the selected todo and persists; toggle on an entry sets the status and changes nothing; delete removes the selected line and clamps selection; edit flow (`begin_edit` → modify `input` → `commit_edit`) rewrites the entry text while preserving its marker.

- [ ] TDD. Commit: `feat: navigate-mode toggle, edit, delete`.

---

## Task 13: Day navigation

**Files:** `src/app/actions.rs`, `src/app/state.rs`

- `go_to_date(date)` → `save()` current, then `open_day` for `date` (load or template). Resets context/selection/overlay.
- `go_today()` → `go_to_date(Local::now().date_naive())`.
- `go_prev_day()`/`go_next_day()` → `date ± 1 day`.
- Wire `Command::Today`/`Command::Goto(Some(d))` in dispatch to call these; `Goto(None)` opens the calendar overlay (`overlay = Calendar`, init calendar state to current date — see Task 17).

**Tests (tempdir):** writing into today then `go_prev_day` then back shows the original content persisted on disk; navigating to a fresh date yields a template document; `Goto(Some(date))` switches the active date.

- [ ] TDD. Commit: `feat: day navigation and persistence on switch`.

---

## Task 14: Terminal bootstrap, CLI, and event loop

**Files:** `src/main.rs`

- Minimal manual CLI parse: support `--notes-dir <path>`, `--help`, `--version` (print and exit); unknown flag → error to stderr, exit non-zero.
- `config::load(cli_notes_dir)` → build `AppState::open_day(.., today)`. On error, print to stderr and exit non-zero (no half-broken TUI).
- `let mut terminal = ratatui::init();` … `ratatui::restore();` on exit (ratatui installs a panic hook that restores the terminal).
- Loop: `terminal.draw(|f| ui::render(f, &app))?;` then read events via `ratatui::crossterm::event` (`poll` + `read`), filtering `KeyEventKind::Press`; dispatch keys (Task 16); break when `app.should_quit`.

**Verify:** `cargo run` opens today's note (creating the file), shows the layout, and `Ctrl-C` exits cleanly leaving the terminal usable. Commit: `feat: terminal bootstrap, CLI, and main loop`.

---

## Task 15: Rendering — layout, document pane, capture bar

**Files:** `src/ui/mod.rs`, `src/ui/layout.rs`, `src/ui/document.rs`, `src/ui/capture.rs`

- `ui::render(frame, &AppState)` composes a vertical layout: document pane (fills), status line (1), capture bar (1+border). Title shows the date.
- Document pane: render each line styled — `# `/`## `/`### ` headings bold/colored; `- [ ] `→`☐`, `- [x] `→`☑`, `- `→`•`. In Navigate focus, highlight the line of the currently selected selectable; auto-scroll to keep it visible.
- Status line: `context: <Notes|name>` on the left, `[? help]` hint on the right; show `status` message when present.
- Capture bar: `› {input}` with a cursor; show an edit indicator when `editing.is_some()`.

**Tests (ratatui `TestBackend`):** render an empty day, a populated day, and a navigate-mode state with a selection; assert the buffer contains expected glyphs (`☐`/`☑`/`•`), the `context:` label, and the input prefix. (Render into a fixed-size backend and snapshot key cells/substrings.)

- [ ] Implement + smoke tests. Commit: `feat: main screen rendering`.

---

## Task 16: Key routing (capture + navigate modes)

**Files:** `src/main.rs` (or `src/app/input.rs` if it keeps `main` small)

- Capture mode: printable chars append to `input`; Backspace; Enter → if `editing` then `commit_edit` else `parse(input)` → `dispatch`, clear `input`; `Esc` → enter Navigate mode (and cancel an in-progress edit). Global keys still active.
- Navigate mode: `j/k/↑/↓` move selection; `g/G` first/last; `Space`/`x` toggle; `e` begin edit; `d` delete (with a confirm: a `pending_delete` flag where `d` then `y` confirms, any other key cancels — keep simple); `?` help overlay; `i`/`Esc` back to Capture.
- Global (both modes, via modifiers so they don't clash with text): `Ctrl-T` today, `[`/`]` prev/next day (Navigate mode, or also Capture when input empty), `Ctrl-G` calendar, `Ctrl-C` quit, `Esc` closes any open overlay first.

**Verify:** manual run exercising capture, meeting flow, todo toggle, edit, delete-with-confirm, and day switching. Commit: `feat: keyboard routing for capture and navigate modes`.

---

## Task 17: Calendar overlay — state and grid logic

**Files:** `src/ui/calendar.rs`

- `CalendarState { visible_month: (i32, u32), selected: NaiveDate }` with `new(focus_date)`.
- Pure helpers: `weeks(visible_month, week_start) -> Vec<[Option<NaiveDate>;7]>` (leading/trailing `None` padding); `move_selection(dx_days, dy_weeks)` adjusting `selected` and rolling `visible_month` when crossing month boundaries.
- `marked(date, &BTreeSet<NaiveDate>) -> bool` for note-existence dots.

**Tests:** June 2026 with Sunday start has day 1 in the correct weekday column and the right number of week rows; moving left from the 1st rolls to the previous month; `marked` reflects the provided set; Monday-start layout shifts columns. (All pure, no terminal.)

- [ ] TDD. Commit: `feat: calendar state and month-grid logic`.

---

## Task 18: Calendar overlay — render and wire

**Files:** `src/ui/calendar.rs`, `src/ui/mod.rs`, key routing

- Render a centered popup (`Clear` + bordered block) with month title, weekday header, the grid; mark days that have notes (trailing dot/dim style) using `storage::dates_with_notes`; highlight `selected`; footer hint `←/→ day  ↑/↓ week · Enter open · Esc cancel`.
- When `overlay == Calendar`: arrows move selection; `Enter` → `go_to_date(selected)` and close overlay; `Esc` closes overlay.

**Tests (`TestBackend`):** rendering the overlay shows the month title, weekday header, and a marked-day indicator for a date present in the set. Manual verify: `Ctrl-G`/`/goto`, navigate, open a day (creating it), cancel with `Esc`.

- [ ] Implement + smoke test. Commit: `feat: calendar overlay rendering and navigation`.

---

## Task 19: Help overlay, status/error surfacing, startup errors

**Files:** `src/ui/mod.rs`, `src/app/state.rs`, `src/main.rs`

- Help overlay (`overlay == Help`, opened by `/help` or `?`): centered popup listing commands and key bindings; `Esc`/`?` closes.
- Ensure `Unknown`/`InvalidArgs` commands set a visible status message and never mutate the doc; status clears on the next successful action.
- Startup IO failures (unreadable notes dir, bad config TOML) print a clear message to stderr and exit non-zero before entering the TUI.

**Tests:** dispatching `Unknown`/`InvalidArgs` leaves `doc.to_text()` unchanged and sets `status`; `TestBackend` render of the help overlay contains a known command line. Manual verify of a deliberately bad config path.

- [ ] Implement + tests. Commit: `feat: help overlay and error surfacing`.

---

## Task 20: Final pass — quality gates and smoke test

**Files:** repo-wide

- [ ] `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`; fix issues.
- [ ] `cargo test` all green.
- [ ] Manual end-to-end smoke: launch, `/meeting "Standup"`, add notes, `/todo follow up`, `/leave`, add a Note, switch days with `[`/`]`, open calendar with `Ctrl-G` and jump to a date, toggle a todo, edit and delete an entry, quit; reopen and confirm persistence and that the Markdown file matches the spec format.
- [ ] Confirm the stray `1` file is gone and `.gitignore` still ignores `/target`.
- [ ] Commit: `chore: formatting, lints, and final smoke pass`.

---

## Self-review notes (spec coverage)

- Capture model / active context: Tasks 11–13. Slash commands: Task 10. Sections + meeting sub-headings + central tagged to-dos: Tasks 5–6. Toggle/edit/delete + navigable pane: Tasks 7–8, 12, 15–16. Calendar with note marks: Tasks 17–18. Storage/naming + config: Tasks 2–3. Atomic writes + tolerant parsing + errors: Tasks 9, 11 (save), 19. Rendering glyphs/layout: Task 15. `/summarize` reserved, no LLM/vector deps: Task 10/11. Cleanup of stray `1`: Tasks 1 and 20.
