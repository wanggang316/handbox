#!/bin/bash
# Worktree context hook for Claude Code
# Provides worktree-specific context before tool execution

set -e

# Get the project directory
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"

cd "$PROJECT_DIR"

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    exit 0
fi

# Get worktree information
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
WORKTREE_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo "unknown")
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null || echo ".git")

# Check if this is a linked worktree (not the main worktree)
IS_LINKED_WORKTREE="no"
if [[ "$GIT_DIR" != ".git" ]] && [[ "$GIT_DIR" == *".git/worktrees"* ]]; then
    IS_LINKED_WORKTREE="yes"
fi

# Get main worktree path
MAIN_WORKTREE=$(git worktree list | head -1 | awk '{print $1}')

# Only output context if we're in a linked worktree
if [ "$IS_LINKED_WORKTREE" = "yes" ]; then
    cat << EOF

<worktree-context>
You are currently working in a Git worktree environment:
- Current branch: $CURRENT_BRANCH
- Worktree path: $WORKTREE_ROOT
- Main worktree: $MAIN_WORKTREE
- This is a linked worktree (isolated from main worktree)

Important considerations:
1. Changes here are isolated to this worktree and won't affect the main worktree
2. Be careful with git operations that might affect shared refs
3. Each worktree has its own HEAD and working tree
4. Some files may be shared across worktrees (.git/config, hooks, etc.)
5. Use 'git worktree list' to see all worktrees
</worktree-context>

EOF
fi

exit 0
