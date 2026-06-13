# Word Wrapping in Capture, Notes, and Chat Panels

## Goal

When a line of text in the capture input box, notes/document panel, or chat panel exceeds the available width, render the overflow on the next display row rather than letting it run past the panel edge.

## Decisions

- **Wrapping style:** hybrid — wrap at word boundaries when possible, but break individual words that are too long to fit (e.g. URLs).
- **Continuation indent:** hanging indent so bullets, todos, quotes, and chat prefixes keep their continuation rows aligned under the text.
- **Vim movement:** `j`/`k` move by display rows, not logical lines.
- **Capture box:** fixed height; scroll when wrapped content exceeds it.

## Architecture

Add a single pure function in a new `src/ui/wrap.rs` module:

```rust
pub struct WrapOptions { pub width: u16, pub hanging_indent: u16 }
pub fn wrap_line(line: &Line, opts: &WrapOptions) -> Vec<Line>
```

The function flattens a styled `Line` into `(char, style, display_width)` tuples, walks them left-to-right, and produces wrapped display rows. It preserves inline styles across wrapped segments and uses `unicode-width` for display widths.

## Notes panel

`src/ui/document.rs`:

1. Style each logical document line into a `Line` exactly as today.
2. Wrap the styled line with a per-line hanging indent:
   - bullets / todos / quotes: `2` (after `• ` / `☐ ` / `│ `)
   - headings / plain / code / metadata / ordered lists: `0`
3. Collect wrapped rows into a flat `Vec<Line>`.
4. Build a side table mapping each logical line to its display-row range.
5. Scroll by display rows: map `doc_anchor_line` to its start display row and offset ~3 rows from the top.
6. Place the terminal cursor on the wrapped row containing the current logical `cursor_col`.

`src/app/input/vim_normal.rs`:

- Keep `cursor_line` and `cursor_col` as logical positions.
- Add `cursor_col_want: usize` (display column) to `VimState`.
- Horizontal motions (`h`, `l`, `w`, `b`, `e`, `0`, `$`) update `cursor_col_want`.
- `j`/`k` compute the current display row, move to the next/previous display row, map back to a logical line, and land on the byte offset closest to `cursor_col_want`.

## Capture panel

`src/ui/capture.rs`:

1. Build the first display line with the prefix (`› ` or `Edit: › `).
2. Wrap the full input text with `hanging_indent` equal to the prefix width.
3. Map `cursor_pos` to the wrapped `(row, col)` and place the terminal cursor.
4. Scroll vertically within the fixed box height to keep the cursor visible.

`src/ui/layout.rs`:

- Set the capture box to a fixed height: `app.config.capture_height.clamp(5, 12)`.
- Stop growing the box for explicit `Ctrl-J` newlines.

## Chat panel

`src/ui/chat_panel.rs`:

1. Replace the existing plain-text `wrap_line` with the shared `wrap_line`.
2. Build each message as a styled `Line`: prefix (`You: ` / `AI:  `) plus content.
3. Wrap with `hanging_indent = 5` so continuation rows align after the prefix.
4. Keep the existing bottom-aligned scroll behavior.

## Testing

- Unit tests in `src/ui/wrap.rs`: word wrap, long-word character break, hanging indent, empty line, width zero, Unicode wide chars, style preservation.
- Renderer tests in `src/ui/document.rs`: long plain line wraps; bullet wraps with continuation indent; cursor on correct wrapped row.
- Renderer tests in `src/ui/capture.rs`: long input wraps and scrolls; cursor mapped correctly.
- Renderer tests in `src/ui/chat_panel.rs`: long message wraps with prefix indent; existing tests still pass.
- Input tests in `src/app/input/vim_normal.rs`: `j`/`k` move by display rows across wrapped lines.
- Run `cargo test` before finishing.

## Risks

- The vim cursor/scrolling rework is the most complex part. Keeping logical coordinates as the source of truth and computing display coordinates on demand keeps the change localized.
- Wide characters and tab characters need explicit handling; tabs should be treated as a single display cell for wrapping purposes.
- Existing tests that assert exact cursor line positions after movements may need updating if they assumed one logical line maps to one display row.
