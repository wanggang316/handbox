# Commit Command

Manual git commit command with customizable options for HandBox project.

## Description

This command provides explicit control over git commits when you want to override the automatic `handbox-committer` Skill behavior or need specific commit options.

## Usage

### Basic commit with message
```
/commit "your commit message"
```

### Commit with type and scope (Conventional Commits)
```
/commit feat api "add streaming support"
/commit fix database "correct migration order"
/commit refactor backend "rename parameters"
```

### Special options
```
/commit --amend "corrected message"
/commit --no-push "work in progress"
/commit --wip "incomplete feature"
```

## Implementation

{{args}}

```bash
# Parse arguments
OPTION=""
TYPE=""
SCOPE=""
MESSAGE=""

# Check for options
if [[ "${1:-}" == --* ]]; then
    OPTION="$1"
    shift
fi

# Parse based on number of remaining arguments
if [ $# -eq 1 ]; then
    # Just message: /commit "message"
    MESSAGE="$1"
elif [ $# -eq 2 ]; then
    # Type and message: /commit feat "message"
    TYPE="$1"
    MESSAGE="$2"
elif [ $# -eq 3 ]; then
    # Type, scope, message: /commit feat api "message"
    TYPE="$1"
    SCOPE="$2"
    MESSAGE="$3"
fi

# Build commit message
build_commit_message() {
    local type="$1"
    local scope="$2"
    local msg="$3"

    if [ -n "$type" ]; then
        if [ -n "$scope" ]; then
            echo "${type}(${scope}): ${msg}"
        else
            echo "${type}: ${msg}"
        fi
    else
        echo "$msg"
    fi
}

# Add co-authorship footer
add_footer() {
    local msg="$1"
    cat <<EOF
$msg

🤖 Generated with [Claude Code](https://claude.com/claude-code)
via [Happy](https://happy.engineering)

Co-Authored-By: Claude <noreply@anthropic.com>
Co-Authored-By: Happy <yesreply@happy.engineering>
EOF
}

# Main execution
case "$OPTION" in
    --amend)
        # Amend last commit
        if [ -z "$MESSAGE" ]; then
            git commit --amend --no-edit
            echo "✅ Amended last commit (kept message)"
        else
            FULL_MSG=$(add_footer "$(build_commit_message "$TYPE" "$SCOPE" "$MESSAGE")")
            git commit --amend -m "$FULL_MSG"
            echo "✅ Amended last commit with new message"
        fi
        ;;

    --wip|--no-push)
        # Work in progress - commit but don't push
        if [ -z "$MESSAGE" ]; then
            MESSAGE="work in progress"
        fi
        FULL_MSG=$(add_footer "wip: $MESSAGE")
        git commit -m "$FULL_MSG"
        echo "✅ Created WIP commit (not pushed)"
        echo "Remember to amend or squash before pushing!"
        ;;

    *)
        # Normal commit
        if [ -z "$MESSAGE" ]; then
            echo "Error: Commit message required"
            echo ""
            echo "Usage:"
            echo "  /commit \"message\""
            echo "  /commit <type> \"message\""
            echo "  /commit <type> <scope> \"message\""
            echo "  /commit --amend \"new message\""
            echo "  /commit --wip \"incomplete work\""
            echo ""
            echo "Types: feat, fix, refactor, perf, test, docs, style, chore"
            echo "Scopes: backend, frontend, database, api, mcp, security, config"
            exit 1
        fi

        # Check for staged changes
        if ! git diff --cached --quiet; then
            FULL_MSG=$(add_footer "$(build_commit_message "$TYPE" "$SCOPE" "$MESSAGE")")

            # Show what will be committed
            echo "📋 Staged changes:"
            git diff --cached --stat
            echo ""

            # Create commit
            git commit -m "$FULL_MSG"

            # Get commit info
            COMMIT_HASH=$(git rev-parse --short HEAD)
            CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

            echo ""
            echo "✅ Commit created successfully!"
            echo "Commit: $COMMIT_HASH"
            echo "Branch: $CURRENT_BRANCH"
            echo ""

            # Suggest next steps
            if [[ "$OPTION" != "--no-push" ]] && [[ "$OPTION" != "--wip" ]]; then
                echo "Next steps:"
                echo "  git push origin $CURRENT_BRANCH"

                # Check if in worktree
                GIT_DIR=$(git rev-parse --git-dir)
                if [[ "$GIT_DIR" == *".git/worktrees"* ]]; then
                    echo "  OR use: /worktree-flow complete"
                fi
            fi
        else
            echo "❌ No staged changes to commit"
            echo "Use: git add <files>"
            echo ""
            echo "Modified files:"
            git status --short
        fi
        ;;
esac
```

## Examples

### Feature Development
```bash
# Simple feature
/commit feat "add user authentication"

# Feature with scope
/commit feat api "add OpenRouter streaming"

# Feature with detailed scope
/commit feat frontend "implement dark mode toggle"
```

### Bug Fixes
```bash
# Simple fix
/commit fix "correct null pointer error"

# Fix with scope
/commit fix database "resolve foreign key constraint"
```

### Refactoring
```bash
# Refactoring
/commit refactor backend "rename support_parameters to supported_parameters"

# Performance improvement
/commit perf "optimize database query"
```

### Work in Progress
```bash
# Save incomplete work
/commit --wip "implementing chat feature"

# Quick save
/commit --no-push "checkpoint before major refactor"
```

### Amending Commits
```bash
# Fix typo in last commit
/commit --amend "corrected typo"

# Amend without changing message
/commit --amend
```

## Conventional Commits Types

- **feat**: A new feature
- **fix**: A bug fix
- **refactor**: Code change that neither fixes a bug nor adds a feature
- **perf**: Performance improvement
- **test**: Adding or updating tests
- **docs**: Documentation only changes
- **style**: Changes that don't affect code meaning (formatting, whitespace)
- **chore**: Changes to build process or auxiliary tools

## HandBox Scopes

- **backend**: Rust/Tauri backend (`src-tauri/`)
- **frontend**: SvelteKit frontend (`src/`)
- **database**: Database schema, migrations
- **api**: API adapters (OpenAI, Anthropic, Google, etc.)
- **mcp**: MCP (Model Context Protocol) integration
- **security**: Security-related changes
- **config**: Configuration files
- **worktree**: Git worktree workflow

## Integration with Skills

This command is complementary to the `handbox-committer` Skill:

- **Skill (automatic)**: Claude decides when to commit and generates messages
- **Command (manual)**: You explicitly control timing and message format

Use this command when:
- You want specific commit message format
- You need to amend a commit
- You're creating a WIP commit
- You want to commit without pushing

Use the Skill when:
- You want Claude to analyze changes and decide
- You want automatically generated semantic messages
- You're following conversational workflow

## Notes

- All commits include co-authorship footer (Claude + Happy)
- Messages follow Conventional Commits format
- Compatible with worktree workflow
- Checks for staged changes before committing
- Provides helpful next-step suggestions
