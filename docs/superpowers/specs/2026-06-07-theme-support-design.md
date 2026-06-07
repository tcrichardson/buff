# Theme Support Design

**Date:** 2026-06-07
**Status:** Approved

---

## Overview

Add a hybrid theme system to buff: named built-in themes selectable with a single config key, plus optional per-element color overrides. Also includes two housekeeping UI changes (bold "Notes" pane header, bold to-do items in the right panel).

---

## Decisions

| Question | Decision |
|---|---|
| Theme approach | Hybrid: named base theme + `[theme_overrides]` overrides in config.toml |
| Built-in themes | `light` (default) and `dark`; more can be added later |
| Heading levels | All 6 (h1–h6) |
| Overdue todo color | Stub the `Theme` field now; detection logic is future work |

---

## Architecture

### 1. `src/ui/theme.rs` (new file)

Contains the `Theme` struct, built-in theme definitions, color string parsing, and theme resolution.

#### `Theme` struct

```rust
pub struct Theme {
    // Headings
    pub heading1: Color,
    pub heading2: Color,
    pub heading3: Color,
    pub heading4: Color,
    pub heading5: Color,
    pub heading6: Color,

    // Borders
    pub border_focused: Color,
    pub border_unfocused: Color,

    // Panels
    pub notes_panel_bg: Color,
    pub panel_bg: Color,        // right panel background
    pub chat_panel_bg: Color,

    // Document elements
    pub quote_marker: Color,
    pub code: Color,
    pub todo_done: Color,       // color for completed ☑ items
    pub todo_overdue: Color,    // stubbed; detection logic is future work
}
```

#### Built-in themes

**`light` (default):**

| Field | Value |
|---|---|
| heading1 | `Color::Black` (bold) |
| heading2 | `Color::Rgb(2, 119, 189)` — medium blue |
| heading3 | `Color::Rgb(230, 81, 0)` — burnt orange |
| heading4 | `Color::Rgb(106, 27, 154)` — purple |
| heading5 | `Color::Rgb(46, 125, 50)` — dark green |
| heading6 | `Color::DarkGray` |
| border_focused | `Color::Rgb(2, 119, 189)` — blue |
| border_unfocused | `Color::DarkGray` |
| notes_panel_bg | `Color::Reset` (terminal default) |
| panel_bg | `Color::Rgb(221, 232, 245)` — light blue-gray |
| chat_panel_bg | `Color::Rgb(230, 230, 240)` |
| quote_marker | `Color::Rgb(123, 31, 162)` — purple |
| code | `Color::DarkGray` |
| todo_done | `Color::Green` |
| todo_overdue | `Color::Red` |

**`dark`:**

| Field | Value |
|---|---|
| heading1 | `Color::White` (bold) |
| heading2 | `Color::Cyan` |
| heading3 | `Color::Yellow` |
| heading4 | `Color::Magenta` |
| heading5 | `Color::Green` |
| heading6 | `Color::Gray` |
| border_focused | `Color::Cyan` |
| border_unfocused | `Color::DarkGray` |
| notes_panel_bg | `Color::Reset` |
| panel_bg | `Color::Rgb(220, 220, 220)` |
| chat_panel_bg | `Color::Rgb(230, 230, 240)` |
| quote_marker | `Color::Magenta` |
| code | `Color::DarkGray` |
| todo_done | `Color::Green` |
| todo_overdue | `Color::Red` |

#### Color string parsing

`parse_color(s: &str) -> Result<Color, String>` accepts:
- Named terminal colors (case-insensitive): `"white"`, `"cyan"`, `"yellow"`, `"black"`, `"red"`, `"green"`, `"blue"`, `"magenta"`, `"gray"`, `"dark_gray"`, `"light_red"`, `"light_green"`, `"light_yellow"`, `"light_blue"`, `"light_magenta"`, `"light_cyan"`, `"reset"`
- Hex RGB: `"#rrggbb"` → `Color::Rgb(r, g, b)`

Returns an error string on unrecognized input; caller logs a warning and falls back to the base theme value.

#### Theme resolution

```
fn resolve_theme(name: &str, overrides: &ThemeOverrides) -> Theme
```

1. Match `name` to a built-in theme fn (`light()`, `dark()`); unknown names fall back to `light` with a warning.
2. For each `Some(color_str)` in `overrides`, call `parse_color`, and if successful replace the corresponding field in `Theme`.
3. Return the resolved `Theme`.

---

### 2. `src/config.rs` changes

Two new fields on `Config`:

```rust
pub theme: String,                    // default: "light"
pub theme_overrides: ThemeOverrides,  // default: all None
```

`ThemeOverrides` mirrors `Theme` with all `Option<String>` fields:

```rust
#[derive(Deserialize, Default)]
pub struct ThemeOverrides {
    pub heading1: Option<String>,
    pub heading2: Option<String>,
    pub heading3: Option<String>,
    pub heading4: Option<String>,
    pub heading5: Option<String>,
    pub heading6: Option<String>,
    pub border_focused: Option<String>,
    pub border_unfocused: Option<String>,
    pub notes_panel_bg: Option<String>,
    pub panel_bg: Option<String>,
    pub chat_panel_bg: Option<String>,
    pub quote_marker: Option<String>,
    pub code: Option<String>,
    pub todo_done: Option<String>,
    pub todo_overdue: Option<String>,
}
```

`#[derive(Default)]` ensures missing TOML sections deserialize cleanly to all-`None`.

**Example `~/.config/buff/config.toml`:**

```toml
theme = "light"

[theme_overrides]
heading2 = "#0055aa"
border_focused = "green"
```

---

### 3. Render pipeline

`Theme` is built once in `main.rs` after config loads:

```rust
let theme = resolve_theme(&config.theme, &config.theme_overrides);
```

Then passed as `&Theme` through the render chain:

```
render(&frame, &state, &config, &theme)
  ├── render_layout(...)      → layout.rs       border_focused, border_unfocused, notes_panel_bg
  ├── render_document(...)    → document.rs     heading1–6, quote_marker, code, todo_done
  ├── render_right_panel(...) → right_panel.rs  panel_bg
  └── render_chat_panel(...)  → chat_panel.rs   chat_panel_bg
```

All hardcoded `Color::*` and `Color::Rgb(...)` constants in these files are replaced with `theme.*` field references.

---

### 4. Heading levels h4–h6

`document.rs` currently only handles `# `, `## `, `### ` prefixes; h4–h6 fall through to plain text. This change adds explicit rendering for `#### `, `##### `, `###### ` using `theme.heading4/5/6` colors with `Modifier::BOLD`.

---

### 5. Housekeeping changes

#### "Notes" bold header in the notes pane

The notes pane `Block` in `layout.rs` currently has no title. Add:

```rust
.title(Span::styled(" Notes ", Style::default().add_modifier(Modifier::BOLD)))
```

to the block wrapping the notes pane. No new component required.

#### Bold to-do items in the right panel

In `right_panel.rs`, todo item text spans currently use plain `Style::default()`. Add `Modifier::BOLD` to each todo text span (both incomplete `☐` and complete `☑` variants).

---

## File Change Summary

| File | Change |
|---|---|
| `src/ui/theme.rs` | **New.** `Theme` struct, `light()`, `dark()`, `parse_color()`, `resolve_theme()` |
| `src/config.rs` | Add `theme: String`, `theme_overrides: ThemeOverrides`; add `ThemeOverrides` struct |
| `src/main.rs` | Call `resolve_theme()` after config load; pass `&theme` to `render()` |
| `src/ui/mod.rs` | Thread `&Theme` through `render()` signature |
| `src/ui/layout.rs` | Use `theme.border_focused`, `theme.border_unfocused`, `theme.notes_panel_bg`; add "Notes" bold block title |
| `src/ui/document.rs` | Replace hardcoded heading/quote/code colors; add h4–h6 rendering |
| `src/ui/right_panel.rs` | Use `theme.panel_bg`; add `Modifier::BOLD` to todo item spans |
| `src/ui/chat_panel.rs` | Use `theme.chat_panel_bg` |

---

## Out of Scope

- Overdue todo detection logic (color field is stubbed; detection is a future feature)
- External theme files (`~/.config/buff/themes/*.toml`)
- More than two built-in themes at launch
