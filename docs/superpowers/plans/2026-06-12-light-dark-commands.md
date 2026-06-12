# /light and /dark Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `/light` and `/dark` slash commands to switch the UI theme at runtime.

**Architecture:** Extend the existing `Command` enum and parser with two new variants, dispatch them to mutate `state.config.theme`, and re-resolve the theme each frame so the change is visible immediately.

**Tech Stack:** Rust, ratatui, existing buff command parser and dispatch architecture.

---

### Task 1: Add `Light` and `Dark` to `Command` enum and parser

**Files:**
- Modify: `src/app/command.rs:1-137`

- [ ] **Step 1: Write the failing tests**

Append these tests to the existing `tests` module at the bottom of `src/app/command.rs`:

```rust
    #[test]
    fn parse_light() {
        assert_eq!(parse("/light"), Command::Light);
    }

    #[test]
    fn parse_dark() {
        assert_eq!(parse("/dark"), Command::Dark);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test parse_light parse_dark`
Expected: FAIL — `Command::Light` and `Command::Dark` not found.

- [ ] **Step 3: Add enum variants and parser cases**

In `src/app/command.rs`, add `Light` and `Dark` to the `Command` enum after `Unknown`:

```rust
pub enum Command {
    // ... existing variants ...
    Unknown(String),
    InvalidArgs(String),
    Light,
    Dark,
}
```

Add two match arms in `parse()` before the catch-all `_` arm:

```rust
        "/light" => Command::Light,
        "/dark" => Command::Dark,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test parse_light parse_dark`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/app/command.rs
git commit -m "feat: add Light and Dark command variants and parser cases"
```

---

### Task 2: Handle `Light` and `Dark` dispatch in `actions.rs`

**Files:**
- Modify: `src/app/actions.rs:110-135`

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `src/app/actions.rs`:

```rust
    #[test]
    fn dispatch_light_sets_theme() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        assert_eq!(state.config.theme, "light"); // default
        dispatch(&mut state, Command::Dark).unwrap();
        assert_eq!(state.config.theme, "dark");
        assert_eq!(state.status, "Theme: dark");
    }

    #[test]
    fn dispatch_dark_then_light() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state(&tmp);
        dispatch(&mut state, Command::Dark).unwrap();
        assert_eq!(state.config.theme, "dark");
        dispatch(&mut state, Command::Light).unwrap();
        assert_eq!(state.config.theme, "light");
        assert_eq!(state.status, "Theme: light");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test dispatch_light_sets_theme dispatch_dark_then_light`
Expected: FAIL — `Command::Light` and `Command::Dark` not handled in `dispatch()`.

- [ ] **Step 3: Add dispatch handlers**

In `dispatch()`, add two new match arms before `Command::Unknown`:

```rust
        Command::Light => { state.config.theme = "light".to_string(); state.status = "Theme: light".to_string(); }
        Command::Dark  => { state.config.theme = "dark".to_string();  state.status = "Theme: dark".to_string(); }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test dispatch_light_sets_theme dispatch_dark_then_light`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/app/actions.rs
git commit -m "feat: handle Light and Dark dispatch"
```

---

### Task 3: Re-resolve theme each frame in `main.rs`

**Files:**
- Modify: `src/main.rs:68-83`

- [ ] **Step 1: Move theme resolution into the render loop**

Remove `let theme = ...;` from the startup block (currently around line 68).

Inside the `loop {` block, before `terminal.draw(...)`, add:

```rust
    let theme = buff::ui::theme::resolve_theme(&app.config.theme, &app.config.theme_overrides);
```

The render call already takes `&theme`, so no other changes needed.

The startup code block should now look like:

```rust
    let mut app =
        buff::app::state::AppState::open_day(notes_dir, config, chrono::Local::now().date_naive())
            .context("Failed to open day")?;
```

And the loop should start with:

```rust
    loop {
        let theme = buff::ui::theme::resolve_theme(&app.config.theme, &app.config.theme_overrides);
        terminal.draw(|frame| {
            buff::ui::render(frame, &app, &theme);
        })?;
```

- [ ] **Step 2: Run the build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: re-resolve theme each frame for runtime switching"
```

---

### Task 4: Update help text in `src/ui/help.rs`

**Files:**
- Modify: `src/ui/help.rs:21-35`

- [ ] **Step 1: Add `/light` and `/dark` to help text**

In the Commands section of `src/ui/help.rs`, after the `/clear` line, add:

```
  /light           switch to light theme
  /dark            switch to dark theme
```

The Commands block should now include these two lines between `/clear` and the Chat panel section.

- [ ] **Step 2: Verify the help text test still passes**

Run: `cargo test render_help_overlay`
Expected: PASS (the test only asserts that `/meeting`, `/ask`, `/start`, `/end`, and `/scheduled` are present; adding lines won't break it).

- [ ] **Step 3: Commit**

```bash
git add src/ui/help.rs
git commit -m "docs: document /light and /dark in help overlay"
```

---

### Task 5: Full test verification

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Commit**

```bash
git commit --allow-empty -m "test: verify full suite passes for /light /dark feature"
```

---

## Spec Coverage Check

| Spec Requirement | Implementing Task |
|---|---|
| Add `Light`, `Dark` to `Command` enum | Task 1 |
| Parse `/light` and `/dark` | Task 1 |
| Dispatch handlers update `state.config.theme` | Task 2 |
| Re-resolve theme each frame | Task 3 |
| Update help text | Task 4 |
| Unit tests for parser and dispatch | Tasks 1, 2 |

## Placeholder Scan

No placeholders found — every step contains exact code, exact file paths, and exact commands.
