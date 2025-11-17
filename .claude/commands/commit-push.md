---
allowed-tools: Bash(git add:*), Bash(git status:*), Bash(git commit:*), Bash(git push:*)
description: Create a git commit then push
---

## Context

- Current git status: !`git status`
- Current git diff (staged and unstaged changes): !`git diff HEAD`
- Current branch: !`git branch --show-current`
- Recent commits: !`git log --oneline -10`
- Push to remote: !`git push`

## Commit Message Guidelines

- Use Conventional Commits format: `<type>: <message>`
- Types: feat, fix, refactor, perf, test, docs, style, chore
- Keep message concise and descriptive (focus on "why" not "what")

## Your task

Based on the above changes, create a single git commit, then push to remote.

## Important

- **Preserve work**: Never force push or perform destructive operations
- **Handle errors**: If a git command fails, explain the issue and suggest solutions
