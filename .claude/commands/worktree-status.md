---
description: Check git status in worktree
---

Check the git status in the specified worktree branch.

Usage: `/worktree-status <branch-name>`

Example: `/worktree-status feature/claude`

Run `git -C "../$(basename $(pwd))-$1" status` to check the status of the worktree.
