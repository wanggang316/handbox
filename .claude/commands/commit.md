---
description: Intelligent git commit assistant for HandBox project
when: User says "提交代码", "commit this", "完成了功能" or similar phrases indicating readiness to commit
---

$ARGUMENTS

Please create a git commit following these steps:

1. **Analyze Changes**:
   - Run `git status` to see what's changed
   - Run `git diff --cached` to review staged changes
   - Check recent commits with `git log -3 --oneline` to understand commit style

2. **Generate Commit Message**:
   - Use Conventional Commits format: `type(scope): message`
   - Types: feat, fix, refactor, perf, test, docs, style, chore
   - Scopes: backend, frontend, database, api, mcp, security, config
   - Keep message concise and descriptive (focus on "why" not "what")

3. **Create Commit**:
   - Add co-authorship footer:
     ```
     🤖 Generated with [Claude Code](https://claude.com/claude-code)
     via [Happy](https://happy.engineering)

     Co-Authored-By: Claude <noreply@anthropic.com>
     Co-Authored-By: Happy <yesreply@happy.engineering>
     ```
   - Use heredoc format for proper formatting:
     ```bash
     git commit -m "$(cat <<'EOF'
     <type>(<scope>): <message>

     🤖 Generated with [Claude Code](https://claude.com/claude-code)
     via [Happy](https://happy.engineering)

     Co-Authored-By: Claude <noreply@anthropic.com>
     Co-Authored-By: Happy <yesreply@happy.engineering>
     EOF
     )"
     ```

4. **Verify**: Run `git status` after commit to confirm success

**Important**:
- Only commit if there are staged changes
- Analyze the actual code changes to write accurate messages
- Follow the project's commit message style from recent commits
