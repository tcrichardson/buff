# /light and /dark Commands Design

**Date:** 2026-06-12
**Status:** Approved

---

## Overview

Add `/light` and `/dark` slash commands to switch the UI theme at runtime without restarting the app.

---

## Decisions

| Question | Decision |
|---|---|
| Persist to config file? | No — runtime-only toggle. Users edit `config.toml` for permanent defaults. |
| When does theme take effect? | Immediately on the next render frame. |
| Unknown theme names | Not applicable — only `light` and `dark` are supported. |

---

## Architecture

### 1. `src/app/command.rs` — Add new variants

Add `Light` and `Dark` to the `Command` enum:

```rust
pub enum Command {
    // ... existing variants ...
    Light,
    Dark,
}
```

Add parser cases in `parse()`:

```rust
"/light" => Command::Light,
"/dark"  => Command::Dark,
```

Add unit tests for both commands.

### 2. `src/app/actions.rs` — Handle dispatch

In `dispatch()`, add:

```rust
Command::Light => { state.config.theme = "light".to_string(); state.status = "Theme: light".to_string(); }
Command::Dark  => { state.config.theme = "dark".to_string();  state.status = "Theme: dark".to_string(); }
```

### 3. `src/main.rs` — Re-resolve theme each frame

Move `theme = resolve_theme(...)` from startup into the render loop so the active theme reflects the latest `config.theme`:

```rust
loop {
    let theme = buff::ui::theme::resolve_theme(&app.config.theme, &app.config.theme_overrides);
    terminal.draw(|frame| {
        buff::ui::render(frame, &app, &theme);
    })?;
    // ...
}
```

### 4. `src/ui/help.rs` — Document new commands

Add under the Commands section:

```
/light           switch to light theme
/dark            switch to dark theme
```

---

## File Change Summary

| File | Change |
|---|---|
| `src/app/command.rs` | Add `Light`, `Dark` variants; add parser cases; add tests |
| `src/app/actions.rs` | Add dispatch handlers for `Light` and `Dark` |
| `src/main.rs` | Move `resolve_theme` into the render loop |
| `src/ui/help.rs` | Document `/light` and `/dark` |

---

## Out of Scope

- Persisting theme changes to `~/.config/buff/config.toml`
- Adding new built-in themes beyond `light` and `dark`
- External theme file loading
