# Design: Default Light and Dark Color Themes

**Date:** 2026-06-12  
**Status:** Approved

## Problem

buff's two built-in themes (`light`, `dark`) grew incrementally and are no longer
cohesive:

- The `dark` theme uses **light-gray panel backgrounds** (`panel_bg = #dcdcdc`,
  `chat_panel_bg = #e6e6f0`) on a near-black canvas, which looks jarring.
- The two themes handle the canvas inconsistently: `light` inherits the terminal
  background (`Reset`) while `dark` paints `#121212`.
- Colors are a mix of named ANSI colors and ad-hoc RGB values, so neither palette
  reads as deliberately designed.
- The most recent change to `light()` updated the heading colors but **left the
  unit tests asserting the old values** — 4 theme tests currently fail
  (`cargo test --lib ui::theme`).
- The README's "Default (light)" column documents yet another set of values that
  matches neither the code nor the tests.

## Goal

Replace the two built-in themes with a cohesive, deliberately designed pair.
No new theme fields, no new config surface — only the default `Color` values
returned by `light()` and `dark()` change, plus the tests, README, and the
canvas-painting behavior implied by the new values.

## Design Decisions

Settled during brainstorming:

1. **Each theme paints its own full canvas.** Both themes set explicit
   `terminal_bg` / `terminal_fg` so buff looks identical regardless of the
   host terminal's colors. (Previously only `dark` did this.)
2. **Truecolor RGB throughout.** Every field is an explicit `Color::Rgb(...)`
   value (except `capture_bg`, see below) for a precise, harmonious palette.
   Targets modern truecolor terminals.
3. **Aesthetic = "Calm Editorial text on crisp neutral surfaces."** The colored
   text (heading ramp + accents) comes from a calm, low-saturation blue family;
   the surfaces (canvas, panels, cursor line, unfocused borders) are crisp
   neutrals (clean near-white in light; Tokyo-Night `#1a1b26` family in dark).

### Heading ramp

Both themes use a **monochromatic blue ramp** that steps from a strong anchor at
`heading1` to lighter/cooler steps at `heading5`, with `heading6` dropping to a
neutral slate gray. `heading4` and `heading5` are now distinct (previously
identical in `light`).

### Capture bar

`capture_bg` stays `Color::Reset` in both themes. Because the canvas is now
always painted as the first layer (`terminal_bg`), a `Reset` capture bar shows
that canvas color — so the capture bar tracks the canvas automatically, including
when a user overrides `terminal_bg`.

## The Palette

### `light()`

| Field | Hex | `Color::Rgb` |
|---|---|---|
| `terminal_bg` | `#fafbfc` | `Rgb(250, 251, 252)` |
| `terminal_fg` | `#2b3040` | `Rgb(43, 48, 64)` |
| `notes_panel_bg` | `#fafbfc` | `Rgb(250, 251, 252)` |
| `heading1` | `#1a365d` | `Rgb(26, 54, 93)` |
| `heading2` | `#2c5282` | `Rgb(44, 82, 130)` |
| `heading3` | `#2b6cb0` | `Rgb(43, 108, 176)` |
| `heading4` | `#3182ce` | `Rgb(49, 130, 206)` |
| `heading5` | `#4299e1` | `Rgb(66, 153, 225)` |
| `heading6` | `#718096` | `Rgb(113, 128, 150)` |
| `border_focused` | `#3182ce` | `Rgb(49, 130, 206)` |
| `border_unfocused` | `#d0d7de` | `Rgb(208, 215, 222)` |
| `panel_bg` | `#eef1f6` | `Rgb(238, 241, 246)` |
| `chat_panel_bg` | `#f4f6fa` | `Rgb(244, 246, 250)` |
| `quote_marker` | `#805ad5` | `Rgb(128, 90, 213)` |
| `code` | `#718096` | `Rgb(113, 128, 150)` |
| `todo_done` | `#38a169` | `Rgb(56, 161, 105)` |
| `todo_overdue` | `#c53030` | `Rgb(197, 48, 48)` |
| `vim_cursor_line` | `#e0e7ff` | `Rgb(224, 231, 255)` |
| `metadata` | `#a0aec0` | `Rgb(160, 174, 192)` |
| `capture_bg` | — | `Color::Reset` |

### `dark()`

| Field | Hex | `Color::Rgb` |
|---|---|---|
| `terminal_bg` | `#1a1b26` | `Rgb(26, 27, 38)` |
| `terminal_fg` | `#d6dae3` | `Rgb(214, 218, 227)` |
| `notes_panel_bg` | `#1a1b26` | `Rgb(26, 27, 38)` |
| `heading1` | `#e2e8f0` | `Rgb(226, 232, 240)` |
| `heading2` | `#90cdf4` | `Rgb(144, 205, 244)` |
| `heading3` | `#7fbce8` | `Rgb(127, 188, 232)` |
| `heading4` | `#93c5fd` | `Rgb(147, 197, 253)` |
| `heading5` | `#a5b4fc` | `Rgb(165, 180, 252)` |
| `heading6` | `#94a3b8` | `Rgb(148, 163, 184)` |
| `border_focused` | `#63b3ed` | `Rgb(99, 179, 237)` |
| `border_unfocused` | `#2a2e3f` | `Rgb(42, 46, 63)` |
| `panel_bg` | `#24283b` | `Rgb(36, 40, 59)` |
| `chat_panel_bg` | `#1f2335` | `Rgb(31, 35, 53)` |
| `quote_marker` | `#b794f4` | `Rgb(183, 148, 244)` |
| `code` | `#8b95a7` | `Rgb(139, 149, 167)` |
| `todo_done` | `#68d391` | `Rgb(104, 211, 145)` |
| `todo_overdue` | `#fc8181` | `Rgb(252, 129, 129)` |
| `vim_cursor_line` | `#292e42` | `Rgb(41, 46, 66)` |
| `metadata` | `#6b7488` | `Rgb(107, 116, 136)` |
| `capture_bg` | — | `Color::Reset` |

## Changes

### 1. `src/ui/theme.rs` — `light()` and `dark()`

Replace the body of both builder functions with the field values in the tables
above. No struct fields are added or removed; `resolve_theme()` and `parse_color()`
are unchanged.

### 2. `src/ui/theme.rs` — unit tests

The existing tests hard-code the old color values. Update assertions (and rename
where the name no longer matches) to the new palette. Affected tests:

- `light_theme_heading2_is_blue` → assert `Rgb(44, 82, 130)`
- `dark_theme_heading1_is_white` → now `Rgb(226, 232, 240)` (rename, e.g.
  `dark_theme_heading1_is_near_white`)
- `dark_theme_heading2_is_cyan` → now `Rgb(144, 205, 244)` (rename, e.g.
  `dark_theme_heading2_is_light_blue`)
- `resolve_light_theme` → `heading2 = Rgb(44, 82, 130)`,
  `border_focused = Rgb(49, 130, 206)`
- `resolve_dark_theme` → `heading1 = Rgb(226, 232, 240)`,
  `heading2 = Rgb(144, 205, 244)`, `border_focused = Rgb(99, 179, 237)`
- `resolve_unknown_theme_falls_back_to_light` → `heading2 = Rgb(44, 82, 130)`
- `resolve_ignores_invalid_override_uses_base` → base `heading1 = Rgb(26, 54, 93)`
  (update the comment too)
- `light_theme_has_vim_cursor_line` → `Rgb(224, 231, 255)`
- `dark_theme_has_vim_cursor_line` → `Rgb(41, 46, 66)`
- `light_theme_terminal_bg_is_reset` → light now paints a canvas; assert
  `Rgb(250, 251, 252)` and rename (e.g. `light_theme_terminal_bg_is_near_white`)
- `light_theme_terminal_fg_is_reset` → assert `Rgb(43, 48, 64)` and rename
- `dark_theme_terminal_bg_is_dark` → assert `Rgb(26, 27, 38)`
- `dark_theme_terminal_fg_is_white` → assert `Rgb(214, 218, 227)` and rename

Tests that exercise the override mechanism rather than specific base values stay
as-is: `parse_*`, `resolve_applies_valid_override`, `resolve_hex_override`,
`resolve_applies_metadata_override`, `resolve_applies_terminal_bg_override`,
`resolve_applies_terminal_fg_override`, `*_has_metadata_color`.

Optionally add `light_theme_capture_bg_is_reset` / `dark_theme_capture_bg_is_reset`
to lock in the documented "tracks the canvas" behavior.

### 3. `README.md` — Themes section

- Update the theme descriptions table: `light` = "calm blue heading ramp on a
  clean near-white canvas"; `dark` = "calm blue heading ramp on a deep
  slate-indigo canvas".
- Update every value in the **"Default (light)"** column of the overrides table to
  the `light()` hex values above (it is currently stale).
- Update the `terminal_bg` / `terminal_fg` rows and the note beneath the table:
  `light` now paints `#fafbfc` / `#2b3040` (no longer `reset`); `dark` paints
  `#1a1b26` / `#d6dae3`.

## Backward Compatibility

- **No config or API change.** Theme names, override keys, and the `Theme` struct
  are unchanged. Existing `[theme_overrides]` keep working and still win over the
  new defaults.
- **Light-theme canvas behavior changes.** Previously the `light` theme inherited
  the terminal background (`Reset`); it now paints an explicit near-white canvas
  (`#fafbfc`) and slate text (`#2b3040`). A user who wants the old
  terminal-inherited behavior can set `terminal_bg = "reset"` and
  `terminal_fg = "reset"` (and `notes_panel_bg = "reset"`) in `[theme_overrides]`.
- **Dark theme** already painted its own canvas; only the specific shades change.

## Test Coverage

- All updated theme unit tests pass: `cargo test --lib ui::theme`.
- Full suite green: `cargo test`.
- Existing `layout_renders_terminal_bg_as_base_layer` continues to pass — the
  base-layer paint that makes `light` look correct (and makes `Reset` capture bars
  track the canvas) is already implemented and tested.

## Out of Scope

- No additional themes beyond `light` and `dark`.
- No new theme fields or config options.
- No changes to the override/parsing machinery (`resolve_theme`, `parse_color`).
