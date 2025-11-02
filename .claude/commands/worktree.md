# Worktree Command

A comprehensive command for managing Git worktrees and coding in worktree environments.

## Description

This command helps you work with Git worktrees by providing utilities to:
- List all existing worktrees
- Switch between worktrees
- Create new worktrees
- Execute coding tasks in worktree contexts
- Safely manage git operations across worktrees

## Usage

### List all worktrees
```
/worktree list
```

### Switch to a specific worktree
```
/worktree switch <branch-name>
```

### Create a new worktree
```
/worktree create <branch-name> [base-branch]
```

### Code in current worktree context
```
/worktree code <task-description>
```

## Implementation

{{args}}

```bash
# Parse command arguments
SUBCOMMAND="${1:-list}"
ARG1="${2:-}"
ARG2="${3:-}"

case "$SUBCOMMAND" in
  list)
    echo "## Current Worktrees"
    git worktree list
    echo ""
    echo "Current worktree: $(git rev-parse --show-toplevel 2>/dev/null || echo 'Not in a git repo')"
    ;;

  switch)
    if [ -z "$ARG1" ]; then
      echo "Error: Branch name required"
      echo "Usage: /worktree switch <branch-name>"
      exit 1
    fi

    # Find worktree for the branch
    WORKTREE_PATH=$(git worktree list | grep "\\[$ARG1\\]" | awk '{print $1}')

    if [ -z "$WORKTREE_PATH" ]; then
      echo "Error: No worktree found for branch '$ARG1'"
      echo ""
      echo "Available worktrees:"
      git worktree list
      exit 1
    fi

    echo "Switching to worktree at: $WORKTREE_PATH"
    echo "Branch: $ARG1"
    echo ""
    echo "To switch in your terminal, run:"
    echo "cd $WORKTREE_PATH"
    ;;

  create)
    if [ -z "$ARG1" ]; then
      echo "Error: Branch name required"
      echo "Usage: /worktree create <branch-name> [base-branch]"
      exit 1
    fi

    BASE_BRANCH="${ARG2:-main}"
    WORKTREE_DIR="../$(basename $(pwd))-$ARG1"

    echo "Creating new worktree:"
    echo "  Branch: $ARG1"
    echo "  Base: $BASE_BRANCH"
    echo "  Path: $WORKTREE_DIR"
    echo ""

    git worktree add -b "$ARG1" "$WORKTREE_DIR" "$BASE_BRANCH"

    if [ $? -eq 0 ]; then
      echo ""
      echo "Worktree created successfully!"
      echo "To switch to it, run:"
      echo "cd $WORKTREE_DIR"
    fi
    ;;

  code)
    if [ -z "$ARG1" ]; then
      echo "Error: Task description required"
      echo "Usage: /worktree code <task-description>"
      exit 1
    fi

    # Get current worktree info
    CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
    WORKTREE_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)
    IS_MAIN_WORKTREE=$(git rev-parse --git-dir | grep -q "^.git$" && echo "yes" || echo "no")

    echo "## Worktree Context"
    echo "- Branch: $CURRENT_BRANCH"
    echo "- Path: $WORKTREE_ROOT"
    echo "- Main worktree: $IS_MAIN_WORKTREE"
    echo ""
    echo "## Task"
    echo "$ARG1"
    echo ""
    echo "I'll now work on this task in the context of the current worktree."
    echo "All changes will be isolated to this worktree and branch."
    ;;

  finish)
    # Get current worktree info
    CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
    WORKTREE_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)
    IS_MAIN_WORKTREE=$(git rev-parse --git-dir | grep -q "^.git$" && echo "yes" || echo "no")
    MAIN_WORKTREE=$(git worktree list | head -1 | awk '{print $1}')
    TARGET_BRANCH="${ARG1:-main}"

    if [ "$IS_MAIN_WORKTREE" = "yes" ]; then
      echo "Error: You are in the main worktree. This command is for linked worktrees only."
      exit 1
    fi

    echo "## Finishing Worktree"
    echo "- Current branch: $CURRENT_BRANCH"
    echo "- Current path: $WORKTREE_ROOT"
    echo "- Target branch: $TARGET_BRANCH"
    echo "- Main worktree: $MAIN_WORKTREE"
    echo ""
    echo "This will:"
    echo "1. Commit all changes in current worktree"
    echo "2. Push current branch to remote"
    echo "3. Switch to main worktree"
    echo "4. Merge changes into $TARGET_BRANCH"
    echo "5. Clean up the worktree"
    echo ""
    echo "Please confirm you want to proceed with these steps."
    echo ""
    echo "Next steps for Claude:"
    echo "1. Run 'git status' to check uncommitted changes"
    echo "2. Create a commit with appropriate message"
    echo "3. Push the branch: git push -u origin $CURRENT_BRANCH"
    echo "4. Switch to main worktree: cd $MAIN_WORKTREE"
    echo "5. Merge: git checkout $TARGET_BRANCH && git merge $CURRENT_BRANCH"
    echo "6. Clean up: git worktree remove $WORKTREE_ROOT"
    ;;

  merge)
    if [ -z "$ARG1" ]; then
      echo "Error: Branch name required"
      echo "Usage: /worktree merge <branch-name> [target-branch]"
      exit 1
    fi

    SOURCE_BRANCH="$ARG1"
    TARGET_BRANCH="${ARG2:-main}"
    CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)

    echo "## Merge Worktree Branch"
    echo "- Source branch: $SOURCE_BRANCH"
    echo "- Target branch: $TARGET_BRANCH"
    echo "- Current branch: $CURRENT_BRANCH"
    echo ""

    # Check if we're on the target branch
    if [ "$CURRENT_BRANCH" != "$TARGET_BRANCH" ]; then
      echo "Warning: You're not on $TARGET_BRANCH. Switching..."
      git checkout "$TARGET_BRANCH" || exit 1
    fi

    # Merge
    echo "Merging $SOURCE_BRANCH into $TARGET_BRANCH..."
    git merge "$SOURCE_BRANCH" --no-ff -m "Merge branch '$SOURCE_BRANCH' into $TARGET_BRANCH"

    if [ $? -eq 0 ]; then
      echo ""
      echo "✓ Merge successful!"
      echo ""
      echo "Next steps:"
      echo "1. Review the merge"
      echo "2. Push changes: git push origin $TARGET_BRANCH"
      echo "3. Remove worktree: git worktree remove <path>"
      echo "4. Delete branch: git branch -d $SOURCE_BRANCH"
    else
      echo ""
      echo "✗ Merge failed. Please resolve conflicts manually."
      exit 1
    fi
    ;;

  remove)
    if [ -z "$ARG1" ]; then
      echo "Error: Branch name required"
      echo "Usage: /worktree remove <branch-name>"
      exit 1
    fi

    BRANCH_NAME="$ARG1"
    WORKTREE_PATH=$(git worktree list | grep "\\[$BRANCH_NAME\\]" | awk '{print $1}')

    if [ -z "$WORKTREE_PATH" ]; then
      echo "Error: No worktree found for branch '$BRANCH_NAME'"
      exit 1
    fi

    echo "Removing worktree:"
    echo "  Branch: $BRANCH_NAME"
    echo "  Path: $WORKTREE_PATH"
    echo ""

    git worktree remove "$WORKTREE_PATH"

    if [ $? -eq 0 ]; then
      echo ""
      echo "✓ Worktree removed successfully!"
      echo ""
      echo "To delete the branch, run:"
      echo "git branch -d $BRANCH_NAME"
    fi
    ;;

  *)
    echo "Unknown subcommand: $SUBCOMMAND"
    echo ""
    echo "Available subcommands:"
    echo "  list              - List all worktrees"
    echo "  switch <branch>   - Switch to a worktree"
    echo "  create <branch>   - Create a new worktree"
    echo "  code <task>       - Execute coding task in current worktree"
    echo "  finish [target]   - Complete worktree work and merge (interactive)"
    echo "  merge <source>    - Merge a worktree branch into target"
    echo "  remove <branch>   - Remove a worktree"
    exit 1
    ;;
esac
```

## Examples

```
/worktree list
/worktree switch feature/new-feature
/worktree create feature/user-auth main
/worktree code "Add user authentication to the login page"
```

## Notes

- This command is designed to work safely with Git worktrees
- Each worktree has its own working directory and can be on a different branch
- Changes in one worktree don't affect others
- The `code` subcommand provides context to Claude about the current worktree
