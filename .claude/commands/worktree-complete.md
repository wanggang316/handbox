---
description: Complete worktree and merge to main
argument-hint: [target-branch]
---

Complete the current worktree workflow and merge to `${1:-main}`:

1. Commit and push current changes
2. Switch to main worktree
3. Merge with `--no-ff`
4. Push merged changes
5. Remove worktree and delete branch
