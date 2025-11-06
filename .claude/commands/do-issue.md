---
allowed-tools: Bash(gh:*), Bash(git add:*), Bash(git status:*), Bash(git commit:*), Bash(git push:*), Bash(git diff:*), Bash(git log:*), Bash(git branch:*), Read, Edit, Write, Glob, Grep, TodoWrite
description: Solve GitHub issue (project)
---

## Workflow

Solve GitHub issue #$ARGUMENTS by following these steps:

### 1. Read Issue

- Use `gh issue view $ARGUMENTS` to get issue details
- Extract: title, description, labels, assignees
- Understand the problem and requirements

### 2. Plan & Implement

- Use TodoWrite to create implementation tasks
- Locate relevant code files (use Grep/Glob)
- Implement the solution following project guidelines
- Add/update tests as needed
- Run quality checks: `npm run check`, `cargo clippy`, `cargo test`

### 3. Commit & Push

- Use Conventional Commits format: `<type>: <message>`
- Types: feat, fix, refactor, perf, test, docs, style, chore
- Keep message concise and descriptive (focus on "why" not "what")

### 4. Update Issue

- Use `gh issue comment $ARGUMENTS --body "<comment>"` to add a comment
- Include: what was resolved, changes made, testing done
- **DO NOT close the issue** - just comment

## Context

- Issue list: !`gh issue list`
- View issue: !`gh issue view $id`
- Current git status: !`git status`
- Current git diff (staged and unstaged changes): !`git diff HEAD`
- Current branch: !`git branch --show-current`
- Recent commits: !`git log --oneline -10`

## Guidelines

- Ensure all quality checks pass before committing
- Keep commits atomic and well-documented
- Reference issue number in commit message
- Provide clear update comment on issue
