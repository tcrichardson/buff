# Default Light and Dark Color Themes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace buff's two built-in `light` and `dark` themes with a cohesive, deliberately designed pair ("calm blue heading ramp on crisp neutral surfaces"), and bring the code, tests, and README back into agreement.

**Architecture:** Pure data change. Only the `Color` values returned by `light()` and `dark()` in `src/ui/theme.rs` change — no struct fields, config keys, or rendering logic are added. Tests that assert specific theme colors (in `theme.rs`, `layout.rs`, `right_panel.rs`) are updated to the new values. The README's theme documentation is corrected.

**Tech Stack:** Rust (edition 2024), ratatui 0.30, `cargo test` (inline `#[cfg(test)]` modules, `TestBackend` for render tests).

---

## Background

The previous light-theme edit changed colors without updating dependent tests, so the suite currently has **7 failing tests**:

```
ui::layout::tests::render_h4_h5_h6_headings
ui::layout::tests::render_h1_uses_theme_color
ui::theme::tests::light_theme_heading2_is_blue
ui::theme::tests::resolve_ignores_invalid_override_uses_base
ui::theme::tests::resolve_light_theme
ui::theme::tests::resolve_unknown_theme_falls_back_to_light
ui::right_panel::tests::panel_uses_theme_panel_bg
```

This plan replaces both palettes with the approved final values and fixes every dependent test, ending with a fully green suite.

## File Structure

- `src/ui/theme.rs` — the `light()` and `dark()` builders plus their `#[cfg(test)]` assertions. Single responsibility: theme definitions + parsing.
- `src/ui/layout.rs` — render tests whose `test_theme()` fixture returns `light()`. Only test assertions change.
- `src/ui/right_panel.rs` — one render test whose `test_theme()` fixture returns `light()`. Only the assertion changes.
- `README.md` — the "Themes" section and overrides table. Documentation only.

## Approved Palette (reference for all tasks)

**`light()`** — `terminal_bg #fafbfc(250,251,252)`, `terminal_fg #2b3040(43,48,64)`, `notes_panel_bg #fafbfc(250,251,252)`, `heading1 #1a365d(26,54,93)`, `heading2 #2c5282(44,82,130)`, `heading3 #2b6cb0(43,108,176)`, `heading4 #3182ce(49,130,206)`, `heading5 #4299e1(66,153,225)`, `heading6 #718096(113,128,150)`, `border_focused #3182ce(49,130,206)`, `border_unfocused #d0d7de(208,215,222)`, `panel_bg #eef1f6(238,241,246)`, `chat_panel_bg #f4f6fa(244,246,250)`, `quote_marker #805ad5(128,90,213)`, `code #718096(113,128,150)`, `todo_done #38a169(56,161,105)`, `todo_overdue #c53030(197,48,48)`, `vim_cursor_line #e0e7ff(224,231,255)`, `metadata #a0aec0(160,174,192)`, `capture_bg Reset`.

**`dark()`** — `terminal_bg #1a1b26(26,27,38)`, `terminal_fg #d6dae3(214,218,227)`, `notes_panel_bg #1a1b26(26,27,38)`, `heading1 #e2e8f0(226,232,240)`, `heading2 #90cdf4(144,205,244)`, `heading3 #7fbce8(127,188,232)`, `heading4 #93c5fd(147,197,253)`, `heading5 #a5b4fc(165,180,252)`, `heading6 #94a3b8(148,163,184)`, `border_focused #63b3ed(99,179,237)`, `border_unfocused #2a2e3f(42,46,63)`, `panel_bg #24283b(36,40,59)`, `chat_panel_bg #1f2335(31,35,53)`, `quote_marker #b794f4(183,148,244)`, `code #8b95a7(139,149,167)`, `todo_done #68d391(104,211,145)`, `todo_overdue #fc8181(252,129,129)`, `vim_cursor_line #292e42(41,46,66)`, `metadata #6b7488(107,116,136)`, `capture_bg Reset`.

---

## Task 1: Redesign the `light()` theme and its dependent tests

**Files:**
- Modify: `src/ui/theme.rs` (the `light()` function, ~lines 28-51, and the light-theme tests)
- Modify: `src/ui/layout.rs` (3 render tests)
- Modify: `src/ui/right_panel.rs` (1 render test)

- [ ] **Step 1: Update the light-theme test assertions to the new values**

In `src/ui/theme.rs`, replace each of the following test functions with the new version.

Replace `light_theme_heading2_is_blue`:

```rust
    #[test]
    fn light_theme_heading2_is_blue() {
        let theme = light();
        assert_eq!(theme.heading2, Color::Rgb(44, 82, 130));
    }
```

Replace `resolve_light_theme`:

```rust
    #[test]
    fn resolve_light_theme() {
        let theme = resolve_theme("light", &ThemeOverrides::default());
        assert_eq!(theme.heading2, Color::Rgb(44, 82, 130));
        assert_eq!(theme.border_focused, Color::Rgb(49, 130, 206));
    }
```

Replace `resolve_unknown_theme_falls_back_to_light`:

```rust
    #[test]
    fn resolve_unknown_theme_falls_back_to_light() {
        let theme = resolve_theme("bogus", &ThemeOverrides::default());
        assert_eq!(theme.heading2, Color::Rgb(44, 82, 130));
    }
```

Replace `resolve_ignores_invalid_override_uses_base`:

```rust
    #[test]
    fn resolve_ignores_invalid_override_uses_base() {
        let mut overrides = ThemeOverrides::default();
        overrides.heading1 = Some("notacolor".to_string());
        let theme = resolve_theme("light", &overrides);
        // light default for heading1 is Rgb(26, 54, 93)
        assert_eq!(theme.heading1, Color::Rgb(26, 54, 93));
    }
```

Replace `light_theme_has_vim_cursor_line`:

```rust
    #[test]
    fn light_theme_has_vim_cursor_line() {
        let theme = light();
        assert_eq!(theme.vim_cursor_line, Color::Rgb(224, 231, 255));
    }
```

Replace `light_theme_terminal_bg_is_reset` (rename — light now paints a canvas):

```rust
    #[test]
    fn light_theme_terminal_bg_is_near_white() {
        let theme = light();
        assert_eq!(theme.terminal_bg, Color::Rgb(250, 251, 252));
    }
```

Replace `light_theme_terminal_fg_is_reset` (rename):

```rust
    #[test]
    fn light_theme_terminal_fg_is_slate() {
        let theme = light();
        assert_eq!(theme.terminal_fg, Color::Rgb(43, 48, 64));
    }
```

Add a new test immediately after `light_theme_terminal_fg_is_slate` to lock in the capture-bar behavior:

```rust
    #[test]
    fn light_theme_capture_bg_is_reset() {
        let theme = light();
        assert_eq!(theme.capture_bg, Color::Reset);
    }
```

In `src/ui/layout.rs`, update the assertion in `render_h1_uses_theme_color`. Replace:

```rust
        // light theme h1 = Color::Black
        let has_h1_color = buffer.content.iter().any(|cell| {
            cell.style().fg == Some(ratatui::style::Color::Black)
                && cell.style().add_modifier.contains(ratatui::style::Modifier::BOLD)
        });
```

with:

```rust
        // light theme h1 = Color::Rgb(26, 54, 93)
        let has_h1_color = buffer.content.iter().any(|cell| {
            cell.style().fg == Some(ratatui::style::Color::Rgb(26, 54, 93))
                && cell.style().add_modifier.contains(ratatui::style::Modifier::BOLD)
        });
```

In `src/ui/layout.rs`, update `render_h4_h5_h6_headings`. Replace:

```rust
        // h4 color = Color::Rgb(106, 27, 154) in light theme
        let has_h4_color = buffer.content.iter().any(|cell| {
            cell.style().fg == Some(ratatui::style::Color::Rgb(106, 27, 154))
        });
```

with:

```rust
        // h4 color = Color::Rgb(49, 130, 206) in light theme
        let has_h4_color = buffer.content.iter().any(|cell| {
            cell.style().fg == Some(ratatui::style::Color::Rgb(49, 130, 206))
        });
```

In `src/ui/layout.rs`, update `render_vim_normal_cursor_line_uses_vim_cursor_line_bg`. Replace:

```rust
        // light theme vim_cursor_line = Color::Rgb(219, 234, 254)
        let has_highlight = buffer.content.iter().any(|cell| {
            cell.style().bg == Some(Color::Rgb(219, 234, 254))
        });
```

with:

```rust
        // light theme vim_cursor_line = Color::Rgb(224, 231, 255)
        let has_highlight = buffer.content.iter().any(|cell| {
            cell.style().bg == Some(Color::Rgb(224, 231, 255))
        });
```

In `src/ui/right_panel.rs`, update `panel_uses_theme_panel_bg`. Replace:

```rust
        // light theme panel_bg = Color::Rgb(221, 232, 245)
        let has_panel_bg = buffer
            .content
            .iter()
            .any(|cell| cell.style().bg == Some(Color::Rgb(221, 232, 245)));
```

with:

```rust
        // light theme panel_bg = Color::Rgb(238, 241, 246)
        let has_panel_bg = buffer
            .content
            .iter()
            .any(|cell| cell.style().bg == Some(Color::Rgb(238, 241, 246)));
```

- [ ] **Step 2: Run the light-dependent tests to verify they now fail against the current implementation**

Run: `cargo test --lib ui::`
Expected: FAIL. The light-value tests above fail because `light()` still returns the old/intermediate values (e.g. `light_theme_heading2_is_blue` reports `left: Rgb(31, 31, 151)`, `right: Rgb(44, 82, 130)`). All `dark_*` tests still pass at this point.

- [ ] **Step 3: Rewrite the `light()` function to the final palette**

In `src/ui/theme.rs`, replace the entire `light()` function with:

```rust
pub fn light() -> Theme {
    Theme {
        heading1: Color::Rgb(26, 54, 93),
        heading2: Color::Rgb(44, 82, 130),
        heading3: Color::Rgb(43, 108, 176),
        heading4: Color::Rgb(49, 130, 206),
        heading5: Color::Rgb(66, 153, 225),
        heading6: Color::Rgb(113, 128, 150),
        border_focused: Color::Rgb(49, 130, 206),
        border_unfocused: Color::Rgb(208, 215, 222),
        notes_panel_bg: Color::Rgb(250, 251, 252),
        panel_bg: Color::Rgb(238, 241, 246),
        chat_panel_bg: Color::Rgb(244, 246, 250),
        quote_marker: Color::Rgb(128, 90, 213),
        code: Color::Rgb(113, 128, 150),
        todo_done: Color::Rgb(56, 161, 105),
        todo_overdue: Color::Rgb(197, 48, 48),
        vim_cursor_line: Color::Rgb(224, 231, 255),
        capture_bg: Color::Reset,
        metadata: Color::Rgb(160, 174, 192),
        terminal_bg: Color::Rgb(250, 251, 252),
        terminal_fg: Color::Rgb(43, 48, 64),
    }
}
```

- [ ] **Step 4: Run the light-dependent tests to verify they pass**

Run: `cargo test --lib ui::`
Expected: PASS for all light and layout/right_panel tests. (Dark tests still pass — `dark()` is unchanged.)

- [ ] **Step 5: Commit**

```bash
git add src/ui/theme.rs src/ui/layout.rs src/ui/right_panel.rs
git commit -m "feat(theme): redesign default light theme palette"
```

---

## Task 2: Redesign the `dark()` theme and its tests

**Files:**
- Modify: `src/ui/theme.rs` (the `dark()` function, ~lines 53-76, and the dark-theme tests)

- [ ] **Step 1: Update the dark-theme test assertions to the new values**

In `src/ui/theme.rs`, replace each of the following test functions.

Replace `dark_theme_heading1_is_white` (rename):

```rust
    #[test]
    fn dark_theme_heading1_is_near_white() {
        let theme = dark();
        assert_eq!(theme.heading1, Color::Rgb(226, 232, 240));
    }
```

Replace `dark_theme_heading2_is_cyan` (rename):

```rust
    #[test]
    fn dark_theme_heading2_is_light_blue() {
        let theme = dark();
        assert_eq!(theme.heading2, Color::Rgb(144, 205, 244));
    }
```

Replace `resolve_dark_theme`:

```rust
    #[test]
    fn resolve_dark_theme() {
        let theme = resolve_theme("dark", &ThemeOverrides::default());
        assert_eq!(theme.heading1, Color::Rgb(226, 232, 240));
        assert_eq!(theme.heading2, Color::Rgb(144, 205, 244));
        assert_eq!(theme.border_focused, Color::Rgb(99, 179, 237));
    }
```

Replace `dark_theme_has_vim_cursor_line`:

```rust
    #[test]
    fn dark_theme_has_vim_cursor_line() {
        let theme = dark();
        assert_eq!(theme.vim_cursor_line, Color::Rgb(41, 46, 66));
    }
```

Replace `dark_theme_terminal_bg_is_dark`:

```rust
    #[test]
    fn dark_theme_terminal_bg_is_dark() {
        let theme = dark();
        assert_eq!(theme.terminal_bg, Color::Rgb(26, 27, 38));
    }
```

Replace `dark_theme_terminal_fg_is_white` (rename):

```rust
    #[test]
    fn dark_theme_terminal_fg_is_light() {
        let theme = dark();
        assert_eq!(theme.terminal_fg, Color::Rgb(214, 218, 227));
    }
```

Add a new test immediately after `dark_theme_terminal_fg_is_light`:

```rust
    #[test]
    fn dark_theme_capture_bg_is_reset() {
        let theme = dark();
        assert_eq!(theme.capture_bg, Color::Reset);
    }
```

- [ ] **Step 2: Run the dark-theme tests to verify they now fail against the current implementation**

Run: `cargo test --lib ui::theme::tests`
Expected: FAIL. The updated `dark_*` tests fail because `dark()` still returns the old values (e.g. `dark_theme_heading1_is_near_white` reports `left: White`, `right: Rgb(226, 232, 240)`). The light tests from Task 1 still pass.

- [ ] **Step 3: Rewrite the `dark()` function to the final palette**

In `src/ui/theme.rs`, replace the entire `dark()` function with:

```rust
pub fn dark() -> Theme {
    Theme {
        heading1: Color::Rgb(226, 232, 240),
        heading2: Color::Rgb(144, 205, 244),
        heading3: Color::Rgb(127, 188, 232),
        heading4: Color::Rgb(147, 197, 253),
        heading5: Color::Rgb(165, 180, 252),
        heading6: Color::Rgb(148, 163, 184),
        border_focused: Color::Rgb(99, 179, 237),
        border_unfocused: Color::Rgb(42, 46, 63),
        notes_panel_bg: Color::Rgb(26, 27, 38),
        panel_bg: Color::Rgb(36, 40, 59),
        chat_panel_bg: Color::Rgb(31, 35, 53),
        quote_marker: Color::Rgb(183, 148, 244),
        code: Color::Rgb(139, 149, 167),
        todo_done: Color::Rgb(104, 211, 145),
        todo_overdue: Color::Rgb(252, 129, 129),
        vim_cursor_line: Color::Rgb(41, 46, 66),
        capture_bg: Color::Reset,
        metadata: Color::Rgb(107, 116, 136),
        terminal_bg: Color::Rgb(26, 27, 38),
        terminal_fg: Color::Rgb(214, 218, 227),
    }
}
```

- [ ] **Step 4: Run the theme tests to verify they pass**

Run: `cargo test --lib ui::theme::tests`
Expected: PASS for all theme tests (light and dark).

- [ ] **Step 5: Commit**

```bash
git add src/ui/theme.rs
git commit -m "feat(theme): redesign default dark theme palette"
```

---

## Task 3: Update the README theme documentation

**Files:**
- Modify: `README.md` (the "### Themes" section, the descriptions table, the overrides table, and the note beneath it)

- [ ] **Step 1: Update the theme descriptions table**

In `README.md`, replace the two-row descriptions table:

```markdown
| Theme | Description |
|---|---|
| `light` (default) | Clean light-blue focused borders with colored headings |
| `dark` | Cyan-focused borders with white headings for dark terminals |
```

with:

```markdown
| Theme | Description |
|---|---|
| `light` (default) | Calm blue heading ramp on a clean near-white canvas |
| `dark` | Calm blue heading ramp on a deep slate-indigo canvas |
```

- [ ] **Step 2: Update the overrides table "Default (light)" column**

In `README.md`, replace the entire overrides table with the corrected default values:

```markdown
| Override key | Default (light) | Example values |
|---|---|---|
| `heading1` | `#1a365d` | `"red"`, `"#ff0000"` |
| `heading2` | `#2c5282` | `"cyan"`, `"#00bcd4"` |
| `heading3` | `#2b6cb0` | `"yellow"`, `"#ff9800"` |
| `heading4` | `#3182ce` | `"magenta"`, `"#9c27b0"` |
| `heading5` | `#4299e1` | `"green"`, `"#4caf50"` |
| `heading6` | `#718096` | `"gray"`, `"#757575"` |
| `border_focused` | `#3182ce` | `"cyan"`, `"#0288d1"` |
| `border_unfocused` | `#d0d7de` | `"gray"`, `"#9e9e9e"` |
| `notes_panel_bg` | `#fafbfc` | `"white"`, `"#ffffff"` |
| `panel_bg` | `#eef1f6` | `"lightgray"`, `"#e3f2fd"` |
| `chat_panel_bg` | `#f4f6fa` | `"lightgray"`, `"#f3e5f5"` |
| `quote_marker` | `#805ad5` | `"magenta"`, `"#ab47bc"` |
| `code` | `#718096` | `"gray"`, `"#616161"` |
| `todo_done` | `#38a169` | `"lightgreen"`, `"#66bb6a"` |
| `todo_overdue` | `#c53030` | `"lightred"`, `"#ef5350"` |
| `vim_cursor_line` | `#e0e7ff` | `"lightgray"`, `"#e3f2fd"` |
| `capture_bg` | `reset` | `"white"`, `"#fafafa"` |
| `metadata` | `#a0aec0` | `"gray"`, `"#757575"` |
| `terminal_bg` | `#fafbfc` | `"black"`, `"#121212"` |
| `terminal_fg` | `#2b3040` | `"white"`, `"#e0e0e0"` |
```

- [ ] **Step 3: Update the note beneath the overrides table**

In `README.md`, replace:

```markdown
> **Note:** The `dark` theme sets `terminal_bg` to `#121212` and `terminal_fg` to `white` by default, so it renders correctly on terminals with a light background. Override these in `[theme_overrides]` to customise or restore terminal-inherited colours (`reset`).
```

with:

```markdown
> **Note:** Both themes now paint their own canvas: `light` uses `terminal_bg` `#fafbfc` / `terminal_fg` `#2b3040`, and `dark` uses `#1a1b26` / `#d6dae3`. This makes buff render consistently regardless of your terminal's own colours. To restore terminal-inherited colours, set `terminal_bg` and `terminal_fg` to `"reset"` in `[theme_overrides]`.
```

- [ ] **Step 4: Verify the docs build/read cleanly**

Run: `git diff --stat README.md`
Expected: `README.md` shows changes confined to the Themes section. Manually skim the rendered table to confirm all 20 rows are present and aligned.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: update README theme defaults to match redesigned palette"
```

---

## Task 4: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Run the entire test suite**

Run: `cargo test`
Expected: PASS. `test result: ok.` with `0 failed`. (Baseline was 7 failed; this confirms every dependent test was reconciled.)

- [ ] **Step 2: Confirm a clean build with no warnings**

Run: `cargo build`
Expected: `Finished` with no errors. No `unused`/`dead_code` warnings introduced (the renamed tests remain referenced as `#[test]` functions).

- [ ] **Step 3: Lint (if clippy is available)**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no errors. If `clippy` is not installed, skip this step.

- [ ] **Step 4: Manual smoke check (optional but recommended)**

Run: `cargo run -- --notes-dir /tmp/buff-theme-check` then, in another shell, repeat with a `config.toml` containing `theme = "dark"`.
Expected: light theme shows a near-white canvas with a calm blue heading ramp; dark theme shows a deep slate-indigo canvas with the light-blue heading ramp. Panels in dark mode are subtly raised (not the old jarring light-gray). Press `/quit` to exit.

---

## Self-Review Notes

- **Spec coverage:** Task 1 covers spec §1 (`light()`) + the light portions of §2/§3 tests; Task 2 covers `dark()` + dark §2 tests; Task 3 covers spec §4 (README); Task 4 covers the spec's "Test Coverage" section. The spec's backward-compat note (light now paints its own canvas) is realized by Task 1's `terminal_bg`/`terminal_fg` values and documented in Task 3.
- **No placeholders:** every code and test block is complete and copy-pasteable.
- **Type consistency:** all values are `ratatui::style::Color` variants (`Rgb`/`Reset`); field names match the `Theme` struct in `src/ui/theme.rs`; renamed tests (`*_is_near_white`, `*_is_slate`, `*_is_light_blue`, `*_is_light`, `*_capture_bg_is_reset`) are self-contained `#[test]` functions with no external references.
