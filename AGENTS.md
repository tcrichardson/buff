Agent Instructions

These rules apply to every agent and subagent working in this repository.

---

## 1. Worktree Discipline

When a git worktree is active, **all** bash commands must use `workdir` pointing at the worktree path — not the main repository root. This applies to every command without exception including `cargo`and `git` commands that operate on the feature branch.

```
# After: git worktree add .worktrees/my-branch -b feature/my-branch
# EVERY subsequent command uses:
workdir = /path/to/repo/.worktrees/my-branch
```

Only two commands legitimately run from the main repository root:
- `git worktree list` — to inspect worktrees
- `git worktree remove <path>` — to clean up after merging

---