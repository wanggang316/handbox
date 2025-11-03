---
description: Complete worktree work and merge to main
argument-hint: [target-branch]
when: User says "完成这个worktree", "finish this worktree", or similar completion phrases
---

Complete the current worktree and merge changes to: ${1:-main}

I'll help you complete the worktree workflow:

1. **Verify we're in a linked worktree** (not main):
   ```bash
   git rev-parse --git-dir | grep -q ".git/worktrees" || echo "Error: Not in a linked worktree"
   ```

2. **Get current state**:
   ```bash
   CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
   WORKTREE_ROOT=$(git rev-parse --show-toplevel)
   MAIN_WORKTREE=$(git worktree list | head -1 | awk '{print $1}')
   TARGET_BRANCH="${1:-main}"
   ```

3. **Commit all changes**:
   - Run `git status` to check changes
   - Create commit with proper message
   - Push: `git push -u origin $CURRENT_BRANCH`

4. **Switch to main worktree and merge**:
   ```bash
   cd $MAIN_WORKTREE
   git checkout $TARGET_BRANCH
   git pull origin $TARGET_BRANCH
   git merge $CURRENT_BRANCH --no-ff -m "Merge $CURRENT_BRANCH"
   git push origin $TARGET_BRANCH
   ```

5. **Clean up**:
   ```bash
   git worktree remove $WORKTREE_ROOT
   git branch -d $CURRENT_BRANCH
   ```

**Safety checks**:
- Only works in linked worktrees
- Uses --no-ff to preserve history
- Pulls before merging
- Only removes worktree after successful merge
