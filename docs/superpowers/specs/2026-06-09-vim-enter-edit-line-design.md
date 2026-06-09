# Design: VimNormal Enter Key — Edit Selected Line in Capture Box

**Date:** 2026-06-09  
**Status:** Approved

---

## Summary

When the user presses `Enter` in VimNormal mode, the line currently under the vim cursor is loaded into the capture box for editing. The raw markdown text (including bullet/checkbox prefixes) is placed in the input field. Focus shifts to the capture box. Committing with `Enter` replaces the original line in the document and returns focus to VimNormal with the cursor on the same line.

---

## Behavior

- **Trigger:** `Enter` key in `Focus::VimNormal`
- **Selectable line:** Load the full raw markdown text of the selectable into `state.input`, set `state.editing = Some(selectable_idx)`, move `cursor_pos` to end of input, switch focus to `Focus::Capture`. The capture box renders `"Edit: › <text>"` (existing behavior).
- **Non-selectable line (empty, timestamp, plain text):** No-op. Nothing happens.
- **Multi-line selectables:** Lines are joined with `\n` — the capture box already supports multi-line input.
- **Commit:** User presses `Enter` in capture box → existing `CommitEdit` path: replaces the selectable's lines in the document, clears `state.input`/`state.editing`, returns focus to `Focus::VimNormal`. The vim cursor stays on the same line.
- **Cancel:** User presses `Esc` in capture box → existing `ExitCaptureMode`/`CancelEdit` path: discards edits, returns to VimNormal.

---

## Data Flow

```
Enter (VimNormal)
  → vim_normal::key_to_action() → UiAction::VimBeginEditLine
  → dispatch → actions::vim_begin_edit_line(state)
      find selectable where sel.lines.contains(&vim.cursor_line)
      if found:
          state.input      = doc.lines[sel.lines].join("\n")
          state.editing    = Some(selectable_idx)
          state.cursor_pos = state.input.len()
          state.focus      = Focus::Capture
      if not found: no-op

Enter (Capture, editing.is_some())
  → existing CommitEdit
      doc.replace_selectable(idx, &formatted_lines)
      state.editing = None, state.input.clear()
      state.focus   = Focus::VimNormal
      vim.cursor_line unchanged
```

---

## Code Changes

| File | Change |
|------|--------|
| `src/app/input/mod.rs` | Add `VimBeginEditLine` variant to `UiAction` enum |
| `src/app/input/vim_normal.rs` | Map `KeyCode::Enter` → `Some(UiAction::VimBeginEditLine)` |
| `src/app/actions.rs` | Add `pub fn vim_begin_edit_line(state: &mut AppState) -> anyhow::Result<()>` |
| `src/app/input/mod.rs` | Dispatch `UiAction::VimBeginEditLine` → `actions::vim_begin_edit_line(state)?` |

No changes to `CommitEdit`, `AppState`, or the rendering layer.

---

## Out of Scope

- Editing non-selectable lines (timestamps, plain text paragraphs)
- Opening capture box on empty lines
- Changing post-commit cursor behavior (cursor stays on edited line by design)
