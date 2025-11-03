---
description: Create a new git worktree
argument-hint: <branch-name> [base-branch]
---

Create a new worktree for branch: $1
Base branch: ${2:-main}

I'll create a new worktree in a sibling directory:

1. **Determine worktree path**:
   ```bash
   WORKTREE_DIR="../$(basename $(pwd))-$1"
   ```

2. **Create worktree**:
   ```bash
   git worktree add -b "$1" "$WORKTREE_DIR" "${2:-main}"
   ```

3. **Show result**:
   - Worktree created at: `$WORKTREE_DIR`
   - Branch: `$1`
   - Based on: `${2:-main}`
   - To switch: `cd $WORKTREE_DIR`

**Example**:
- `/worktree-create feature/auth` - Create from main
- `/worktree-create feature/ui develop` - Create from develop
