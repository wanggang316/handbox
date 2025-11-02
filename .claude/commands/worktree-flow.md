# Worktree Flow Command

Complete workflow automation for Git worktree development cycle.

## Description

This command automates the complete worktree development workflow from start to finish. It's designed to be used conversationally with Claude Code:

1. "在 feature/xxx worktree 上完成某个功能"
2. "完成这个 worktree" - triggers automatic merge and cleanup

## Usage

```
/worktree-flow <action> [args...]
```

## Actions

### Start a new feature
```
/worktree-flow start <branch-name> [base-branch] "<task-description>"
```

Creates a new worktree, switches to it, and begins working on the task.

**Example:**
```
/worktree-flow start feature/user-auth main "实现用户认证功能"
```

### Complete and merge feature
```
/worktree-flow complete [target-branch] "<commit-message>"
```

Automatically:
1. Commits all changes
2. Pushes to remote
3. Switches to main worktree
4. Merges changes
5. Cleans up worktree
6. Optionally creates PR

**Example:**
```
/worktree-flow complete main "Add user authentication feature"
```

## Implementation

{{args}}

```bash
ACTION="${1:-help}"
BRANCH_ARG="${2:-}"
BASE_OR_TARGET="${3:-main}"
MESSAGE="${4:-}"

case "$ACTION" in
  start)
    if [ -z "$BRANCH_ARG" ]; then
      echo "Error: Branch name required"
      echo "Usage: /worktree-flow start <branch-name> [base-branch] \"<task>\""
      exit 1
    fi

    BASE_BRANCH="${BASE_OR_TARGET:-main}"
    TASK="${MESSAGE:-Development task}"
    WORKTREE_DIR="../$(basename $(pwd))-$BRANCH_ARG"

    echo "## Starting Worktree Workflow"
    echo "- Feature branch: $BRANCH_ARG"
    echo "- Base branch: $BASE_BRANCH"
    echo "- Worktree path: $WORKTREE_DIR"
    echo "- Task: $TASK"
    echo ""

    # Create worktree
    git worktree add -b "$BRANCH_ARG" "$WORKTREE_DIR" "$BASE_BRANCH"

    if [ $? -ne 0 ]; then
      echo "Failed to create worktree"
      exit 1
    fi

    echo ""
    echo "✓ Worktree created successfully!"
    echo ""
    echo "Claude should now:"
    echo "1. Navigate to: cd $WORKTREE_DIR"
    echo "2. Work on task: $TASK"
    echo "3. When done, use: /worktree-flow complete"
    ;;

  complete)
    # Get current worktree info
    CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
    WORKTREE_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)
    IS_MAIN_WORKTREE=$(git rev-parse --git-dir | grep -q "^.git$" && echo "yes" || echo "no")
    MAIN_WORKTREE=$(git worktree list | head -1 | awk '{print $1}')
    TARGET_BRANCH="${BRANCH_ARG:-main}"
    COMMIT_MSG="${BASE_OR_TARGET:-}"

    if [ "$IS_MAIN_WORKTREE" = "yes" ]; then
      echo "Error: You are in the main worktree. This command is for linked worktrees only."
      exit 1
    fi

    echo "## Completing Worktree Workflow"
    echo ""
    echo "📍 Current State:"
    echo "  - Branch: $CURRENT_BRANCH"
    echo "  - Path: $WORKTREE_ROOT"
    echo "  - Target: $TARGET_BRANCH"
    echo "  - Main worktree: $MAIN_WORKTREE"
    echo ""
    echo "🔄 Workflow Steps:"
    echo "  1. ✓ Check git status"
    echo "  2. → Commit changes"
    echo "  3. → Push to remote"
    echo "  4. → Switch to main worktree"
    echo "  5. → Merge into $TARGET_BRANCH"
    echo "  6. → Push merged changes"
    echo "  7. → Clean up worktree"
    echo ""
    echo "Claude will now execute these steps automatically."
    echo ""
    echo "---"
    echo ""
    echo "## Step 1: Check Status"
    git status
    echo ""
    echo "Claude, please proceed with:"
    echo "- If there are uncommitted changes, commit them with message: ${COMMIT_MSG:-feat: complete $CURRENT_BRANCH}"
    echo "- Push: git push -u origin $CURRENT_BRANCH"
    echo "- Change directory: cd $MAIN_WORKTREE"
    echo "- Checkout target: git checkout $TARGET_BRANCH"
    echo "- Pull latest: git pull origin $TARGET_BRANCH"
    echo "- Merge: git merge $CURRENT_BRANCH --no-ff"
    echo "- Push: git push origin $TARGET_BRANCH"
    echo "- Remove worktree: git worktree remove $WORKTREE_ROOT"
    echo "- Delete branch: git branch -d $CURRENT_BRANCH"
    echo ""
    echo "After completion, report success and suggest creating a PR if needed."
    ;;

  status)
    echo "## Worktree Status"
    echo ""
    git worktree list
    echo ""
    CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
    IS_MAIN=$(git rev-parse --git-dir | grep -q "^.git$" && echo "yes" || echo "no")

    echo "Current context:"
    echo "  - Branch: $CURRENT_BRANCH"
    echo "  - Main worktree: $IS_MAIN"
    ;;

  help|*)
    echo "Worktree Flow - Automated workflow for Git worktrees"
    echo ""
    echo "Usage:"
    echo "  /worktree-flow start <branch> [base] \"<task>\"  - Start new feature"
    echo "  /worktree-flow complete [target] \"<message>\"   - Complete and merge"
    echo "  /worktree-flow status                          - Show status"
    echo ""
    echo "Conversational Usage:"
    echo "  User: \"在 feature/xxx worktree 上完成某个功能\""
    echo "  Assistant: Uses /worktree-flow start"
    echo "  User: \"完成这个 worktree\""
    echo "  Assistant: Uses /worktree-flow complete"
    echo ""
    echo "Examples:"
    echo "  /worktree-flow start feature/auth main \"Add authentication\""
    echo "  /worktree-flow complete main \"feat: add user authentication\""
    ;;
esac
```

## Conversational Patterns

Claude should recognize these phrases and trigger the appropriate workflow:

### Starting Work
- "在 feature/xxx worktree 上完成..."
- "创建一个 worktree 来..."
- "Start working on ... in a new worktree"

→ Use `/worktree-flow start`

### Completing Work
- "完成这个 worktree"
- "合并到主分支"
- "Finish this worktree"
- "Merge and cleanup"

→ Use `/worktree-flow complete`

## Best Practices

1. **Always commit before completing**: The workflow expects clean commits
2. **Test before merging**: Run tests in the worktree before completion
3. **Pull before merge**: The workflow pulls latest changes before merging
4. **Use --no-ff**: Preserves feature branch history in main branch
5. **Clean up promptly**: Removes worktree and branch after successful merge

## Safety Features

- ✅ Prevents completion from main worktree
- ✅ Checks for uncommitted changes
- ✅ Uses --no-ff merge to preserve history
- ✅ Pulls latest changes before merging
- ✅ Only deletes branch after successful merge

## Integration with Hooks

This command works seamlessly with:
- `git-add.sh` - Auto-stages changes
- `worktree-context.sh` - Provides context awareness
