# Claude Code Custom Commands

This directory contains custom slash commands for Claude Code, designed for the HandBox project development workflow.

## Available Commands

### `/commit` - Intelligent Git Commit

Analyzes changes and creates a commit with Conventional Commits format.

**Usage:**
```
/commit
/commit "custom message"
```

**What it does:**
- Analyzes git status and staged changes
- Generates semantic commit message (feat, fix, refactor, etc.)
- Adds co-authorship footer
- Follows Conventional Commits format

**Example:**
```
User: /commit
Claude: [analyzes changes, creates commit like "feat(frontend): add dark mode toggle"]
```

---

### `/db-inspect` - Database Inspector

Quick SQLite database inspection for debugging.

**Usage:**
```
/db-inspect                          # Show all tables
/db-inspect models                   # Show table schema and data
/db-inspect "SELECT * FROM models"   # Custom query
```

**What it does:**
- Connects to HandBox SQLite database
- Shows tables, schemas, and data
- Executes read-only queries

**Example:**
```
/db-inspect models
[Shows model table schema and sample data]
```

---

### `/test-run` - Run Rust Tests

Execute backend tests with optional filters.

**Usage:**
```
/test-run                    # Run all tests
/test-run test_send_message  # Run specific test
/test-run services::chat     # Run module tests
/test-run --verbose          # Show output
```

**What it does:**
- Runs tests in `src-tauri/` directory
- Shows pass/fail results
- Optionally shows test output

**Example:**
```
/test-run services::chat
[Runs all tests in chat service module]
```

---

### `/worktree-list` - List Git Worktrees

Show all git worktrees in the repository.

**Usage:**
```
/worktree-list
```

**What it does:**
- Shows all worktree paths
- Displays current branch for each
- Helps navigate between worktrees

---

### `/worktree-create` - Create New Worktree

Create a new git worktree for isolated development.

**Usage:**
```
/worktree-create feature/auth           # Create from main
/worktree-create feature/ui develop     # Create from develop
```

**What it does:**
- Creates new worktree in sibling directory
- Creates new branch from base (default: main)
- Isolates changes from main worktree

**Example:**
```
/worktree-create feature/auth
[Creates worktree at ../handbox-feature/auth]
```

---

### `/worktree-complete` - Complete Worktree Work

Finish worktree development and merge to main branch.

**Usage:**
```
/worktree-complete           # Merge to main
/worktree-complete develop   # Merge to develop
```

**What it does:**
1. Commits all changes
2. Pushes branch to remote
3. Switches to main worktree
4. Merges with --no-ff (preserves history)
5. Pushes merged changes
6. Removes worktree
7. Deletes branch

**Example:**
```
User: "完成这个worktree"
Claude: /worktree-complete
[Automatically completes entire merge workflow]
```

---

## Command Design Philosophy

All commands follow these principles:

1. **Atomic Operations** - Each command does one thing well
2. **Simple Prompts** - No complex bash scripts, just clear instructions
3. **Context-Aware** - Uses frontmatter and argument hints
4. **Composable** - Commands work together naturally

## Integration with Other Features

### Hooks

Commands work seamlessly with configured hooks:

- **git-add.sh** (UserPromptSubmit) - Auto-stages changes before commit
- **worktree-context.sh** (UserPromptSubmit) - Provides worktree context
- **auto-format.sh** (PostToolUse) - Auto-formats code after edits

### Skills

- **handbox-committer** - Automatically triggered by "提交代码", "commit this", etc.

### Subagents

For complex tasks, use specialized subagents:

- **rust-backend** - Rust/Tauri development
- **svelte-frontend** - SvelteKit/Svelte development
- **security-reviewer** - Security audits
- **database-inspector** - Complex database debugging
- **rust-test-writer** - Unit test creation

## Typical Workflows

### Feature Development in Worktree

```
1. Create worktree
   /worktree-create feature/auth

2. Work on feature
   [Make changes, test, iterate]

3. Commit changes
   /commit

4. Complete and merge
   /worktree-complete
```

### Database Debugging

```
1. Inspect database
   /db-inspect models

2. For complex investigation
   "调查为什么模型插入失败"
   [Uses database-inspector subagent]
```

### Test-Driven Development

```
1. Write test
   "为 chat_service 编写单元测试"
   [Uses rust-test-writer subagent]

2. Run tests
   /test-run services::chat

3. Iterate until passing
   /test-run --verbose
```

## Best Practices

1. **Use commands for simple, frequent operations**
   - Running tests
   - Checking database
   - Creating commits
   - Managing worktrees

2. **Use subagents for complex tasks**
   - Writing tests
   - Security reviews
   - Detailed code analysis
   - Debugging complex issues

3. **Let hooks automate repetitive tasks**
   - Auto-staging changes
   - Auto-formatting code
   - Providing context

## Adding New Commands

To add a new command:

1. Create `command-name.md` in `.claude/commands/`
2. Use frontmatter for metadata:
   ```markdown
   ---
   description: Brief description
   argument-hint: [arg1] [arg2]
   when: Optional trigger phrases
   ---
   ```
3. Write simple, clear instructions (not bash scripts)
4. Use `$1`, `$2`, or `$ARGUMENTS` for arguments
5. Keep it atomic (single purpose)
6. Update this README

## References

- [Claude Code Slash Commands Documentation](https://docs.claude.com/en/docs/claude-code/slash-commands)
- Project configuration: `.claude/settings.json`
- Skills directory: `.claude/skills/`
- Subagents directory: `.claude/agents/`
- Hooks directory: `.claude/hooks/`
