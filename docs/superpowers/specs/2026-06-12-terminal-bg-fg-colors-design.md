# Design: Terminal Background and Foreground Colors

**Date:** 2026-06-12  
**Status:** Approved

## Problem

When a user selects buff's `dark` theme on a machine whose terminal emulator is configured with a light background, the themed panels and markdown are styled for dark (e.g. white heading text, light-on-dark colours), but the canvas behind those panels remains the terminal's own light background. The result is a split appearance — dark-styled content sitting on a light base.

The root cause: buff's theme system has per-panel background fields (`notes_panel_bg`, `panel_bg`, etc.) but no field that paints the overall terminal canvas. Text colour is also inherited from the host terminal, which may clash with the chosen theme's styling.

## Goal

Add two new theme fields — `terminal_bg` and `terminal_fg` — that control the full-canvas background and the default foreground (text) colour. These fields are:

- Part of both built-in themes (`light`, `dark`)
- Overridable via `[theme_overrides]` in `config.toml`
- Applied as the first painted layer every frame, so per-panel colours always render on top

## Approach: Option A (Opinionated dark defaults)

`light` defaults to `Color::Reset`/`Color::Reset` — no behavior change for existing users.
`dark` gets explicit defaults: `terminal_bg = Rgb(18, 18, 18)`, `terminal_fg = Color::White` — so the dark theme looks correct on any terminal without manual configuration.

## Changes

### 1. `src/ui/theme.rs` — `Theme` struct

Add two fields:

```rust
pub terminal_bg: Color,
pub terminal_fg: Color,
```

Update `light()`:

```rust
terminal_bg: Color::Reset,
terminal_fg: Color::Reset,
```

Update `dark()`:

```rust
terminal_bg: Color::Rgb(18, 18, 18),
terminal_fg: Color::White,
```

Add `apply!(terminal_bg)` and `apply!(terminal_fg)` in `resolve_theme()`.

### 2. `src/config.rs` — `ThemeOverrides`

Add two optional fields:

```rust
pub terminal_bg: Option<String>,
pub terminal_fg: Option<String>,
```

### 3. `src/ui/layout.rs` — `render()`

At the very top of `render()`, before any other widget, paint the full canvas:

```rust
frame.render_widget(
    Block::default().style(Style::default().bg(theme.terminal_bg).fg(theme.terminal_fg)),
    frame.area(),
);
```

This fills every cell with `terminal_bg`/`terminal_fg` as the base layer. Per-panel widgets (notes, chat, right panel, etc.) paint on top of this, overriding as needed.

### 4. `README.md` — theme overrides table

Add two rows:

| `terminal_bg` | `reset` (light) / `#121212` (dark) | `"black"`, `"#1e1e1e"` |
| `terminal_fg` | `reset` (light) / `white` (dark)   | `"white"`, `"#e0e0e0"` |

## Backward Compatibility

- **Light theme users:** No change — both fields default to `Color::Reset`, which inherits from the host terminal exactly as before.
- **Dark theme users on dark terminals:** Minor change — the canvas is now an explicit `Rgb(18,18,18)` instead of inheriting. Users can set `terminal_bg = "reset"` in `[theme_overrides]` to revert.
- **Dark theme users on light terminals:** The core problem is fixed — the dark canvas is now enforced by buff rather than inherited from the terminal.

## Test Coverage

New unit tests (in addition to existing theme tests):

- `light_theme_terminal_bg_is_reset`
- `light_theme_terminal_fg_is_reset`
- `dark_theme_terminal_bg_is_dark`
- `dark_theme_terminal_fg_is_white`
- `resolve_applies_terminal_bg_override`
- `resolve_applies_terminal_fg_override`
- `layout_renders_terminal_bg_as_base_layer` — uses `TestBackend` and confirms a cell in an otherwise-unpainted area carries the expected background colour when `terminal_bg` is a non-Reset colour

## Config Example

```toml
theme = "dark"

# optional overrides
[theme_overrides]
terminal_bg = "#1e1e1e"
terminal_fg = "#e0e0e0"
```
