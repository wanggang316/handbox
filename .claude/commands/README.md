# Claude Code Custom Commands

This directory contains custom slash commands for Claude Code, specifically designed for Git worktree workflows.

## 🚀 Quick Start

### Simple Workflow (Recommended)
```
User: "在 feature/new-ui worktree 上实现新的用户界面"
Claude: Uses /worktree-flow start feature/new-ui main "实现新的用户界面"

[... work on the feature ...]

User: "完成这个 worktree"
Claude: Uses /worktree-flow complete - automatically commits, merges, and cleans up
```

## 📋 Available Commands

### `/worktree-flow` - Complete Workflow Automation ⭐

**Recommended for most users.** Automates the entire worktree development cycle.

**Actions:**
- `/worktree-flow start <branch> [base] "<task>"` - Start new feature in worktree
- `/worktree-flow complete [target] "<message>"` - Finish and merge automatically
- `/worktree-flow status` - Show current worktree status

**Conversational Usage:**
Claude recognizes these phrases automatically:
- "在 XXX worktree 上完成..." → starts workflow
- "完成这个 worktree" → completes and merges
- "Finish this worktree" → completes and merges

**What it does automatically:**
1. ✅ Commits all changes with proper message
2. ✅ Pushes branch to remote
3. ✅ Switches to main worktree
4. ✅ Merges with --no-ff (preserves history)
5. ✅ Pushes merged changes
6. ✅ Removes worktree
7. ✅ Deletes feature branch

**Example:**
```bash
# Start working
/worktree-flow start feature/auth main "Add user authentication"

# When done
/worktree-flow complete main "feat: add user authentication system"
```

---

### `/worktree` - Manual Worktree Management

For users who want more control over individual steps.

**Subcommands:**
- `/worktree list` - List all existing worktrees
- `/worktree switch <branch>` - Switch to a specific worktree
- `/worktree create <branch> [base]` - Create a new worktree
- `/worktree code <task>` - Execute coding task with context
- `/worktree finish [target]` - Interactive completion (shows steps)
- `/worktree merge <source> [target]` - Merge a branch
- `/worktree remove <branch>` - Remove a worktree

**Examples:**
```bash
# Create and work manually
/worktree create feature/new-ui main
/worktree code "Redesign user interface"

# When done (step by step)
/worktree finish main
# Claude will guide you through each step

# Or merge directly
/worktree merge feature/new-ui main
/worktree remove feature/new-ui
```

## 🎯 Best Practices

### 1. Use Conversational Flow (Easiest)
```
You: "在 feature/payment worktree 上添加支付功能"
Claude: [automatically creates worktree and starts coding]

You: "完成这个 worktree"
Claude: [automatically commits, merges, cleans up]
```

### 2. Manual Control Flow
```
/worktree-flow start feature/payment main "Add payment integration"
[work on feature...]
/worktree-flow complete main "feat: integrate payment gateway"
```

### 3. Step-by-Step Flow (Most Control)
```
/worktree create feature/payment main
/worktree code "Add payment integration"
[work on feature...]
/worktree finish main
[follow Claude's guidance for each step]
```

## 🔧 How It Works

### Hooks Integration

The commands work seamlessly with configured hooks:

1. **git-add.sh** (UserPromptSubmit)
   - Automatically stages all changes before each message
   - Ensures nothing is forgotten

2. **worktree-context.sh** (UserPromptSubmit)
   - Detects when you're in a linked worktree
   - Provides context to Claude automatically
   - Shows warnings about worktree-specific considerations

### Context Awareness

When in a linked worktree, Claude receives:
```
<worktree-context>
You are currently working in a Git worktree environment:
- Current branch: feature/xxx
- Worktree path: /path/to/worktree
- Main worktree: /path/to/main
...
</worktree-context>
```

This helps Claude:
- Understand the isolation environment
- Make appropriate git operation suggestions
- Avoid operations that might affect other worktrees

## 🛡️ Safety Features

1. **Prevents accidental operations in main worktree**
   - `complete` and `finish` commands check if you're in a linked worktree

2. **Always uses --no-ff merge**
   - Preserves feature branch history
   - Makes it easy to revert entire features

3. **Pulls before merging**
   - Ensures you're merging into latest code
   - Reduces conflicts

4. **Cleans up only after successful merge**
   - Worktree is not removed if merge fails
   - Branch is deleted only after successful merge

5. **Auto-staging with git-add.sh**
   - Never lose uncommitted changes
   - Everything is staged before operations

## 📖 Typical Development Cycle

```
1. User: "在 feature/xxx worktree 上实现功能"
   ↓
2. Claude: Creates worktree, starts coding
   ↓
3. Claude: Makes changes, tests, commits
   ↓
4. User: "完成这个 worktree"
   ↓
5. Claude: Automatically:
   - Commits remaining changes
   - Pushes to remote
   - Switches to main worktree
   - Merges with --no-ff
   - Pushes merged changes
   - Removes worktree
   - Deletes branch
   ↓
6. Done! Feature merged and cleaned up
```

## 🎓 Advanced Usage

### Working on Multiple Features
```bash
# Start multiple worktrees
/worktree-flow start feature/ui main "UI improvements"
/worktree-flow start feature/api main "API updates"

# List all active worktrees
/worktree list

# Switch between them
cd ../handbox-feature/ui
cd ../handbox-feature/api

# Complete them independently
/worktree-flow complete
```

### Custom Merge Targets
```bash
# Merge into develop instead of main
/worktree-flow complete develop "feat: new feature"

# Or with /worktree
/worktree merge feature/xxx develop
```

### Emergency Cleanup
```bash
# If something goes wrong, manually clean up
/worktree list
/worktree remove feature/broken-branch
git branch -D feature/broken-branch
```

## 🔍 Troubleshooting

**Problem: "Not in a linked worktree" error**
- You're trying to complete from the main worktree
- Solution: Make sure you're in the feature worktree

**Problem: Merge conflicts**
- The workflow will stop and show conflict messages
- Solution: Resolve conflicts manually, then continue

**Problem: Worktree not found**
- The worktree may have been manually deleted
- Solution: Use `git worktree prune` to clean up

**Problem: Can't delete branch**
- Branch might not be fully merged
- Solution: Use `git branch -D` (force delete) if you're sure

## 📚 References

- Git Worktree Documentation: https://git-scm.com/docs/git-worktree
- Claude Code Hooks: `.claude/settings.json`
- Command Implementation: `.claude/commands/`

---

See `.claude/settings.json` for hook configuration.
See individual command files for implementation details.
