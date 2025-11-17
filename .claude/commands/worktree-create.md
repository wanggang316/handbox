---
description: Create a git worktree
argument-hint: <branch-name> [base-branch]
---

Create a worktree for branch `$1` from `${2:-main}` in a sibling directory.

Use: `git worktree add -b "$1" "../$(basename $(pwd))-$1" "${2:-main}"`
