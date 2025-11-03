# HandBox AI 协作指南

这是 HandBox 项目的 AI 协作配置文件，为 Claude 提供项目上下文和开发指导。

## 🎯 项目概述

HandBox 是一款基于 Tauri 2 + SvelteKit 5 的本地优先、隐私可控的多模型 AI 工作台。

**核心特性**:
- 多模型供应商管理（OpenAI、Anthropic、Google AI 等）
- 可复用聊天配置（Artifact 系统）
- MCP (Model Context Protocol) 集成
- 本地数据存储与全文搜索
- API Key 安全存储（OS Keychain）

## 📋 开发工作流

项目采用五阶段 **人+AI 协作** 开发模式：

| 阶段 | 主导者 | AI 职责 |
|------|--------|---------|
| 需求阶段 | 👨‍💻 人 | 无 |
| UI/UX 设计 | 👨‍💻 人 + 🤖 AI | 辅助组件设计、样式实现 |
| 任务管理 | 🤖 AI | 拆分任务，生成清单 |
| 开发阶段 | 🤖 AI | 编码实现，代码审查 |
| 测试阶段 | 👨‍💻 人 + 🤖 AI | 测试生成，质量保证 |

## 🔧 技术约束与规范

### 前端 (SvelteKit + TypeScript + Tailwind CSS 4.x + Lucide Icons)
- ✅ **框架**: Svelte 5 + SvelteKit 5，严格 TypeScript，禁用 `any` 类型
- ✅ **样式**: Tailwind CSS 4.x，使用 `@theme` 指令和CSS变量，双主题支持
- ✅ **图标**: Lucide Svelte (`@lucide/svelte`)，语义化图标选择
- ✅ **组件结构**: 导入 → props/逻辑 → 模板 → Tailwind样式类
- ✅ **Svelte 5语法**: 优先使用 `$state`, `$derived` 等新语法
- ✅ **代码质量**: 使用 Prettier 格式化，通过 `npm run check` 类型检查

### 后端 (Tauri + Rust)
- ✅ 所有 `pub` 函数必须有单元测试
- ✅ 使用 `cargo fmt` 和 `cargo clippy` 保持代码质量
- ✅ IPC 命令遵循 `domain_action` 命名规范
- ✅ 错误处理使用统一格式：`{ code, message, hint }`
- ✅ 优先使用 `tokio` 异步编程

### 安全与隐私
- 🔒 API Key 必须使用 OS Keychain 存储
- 🔒 禁止硬编码敏感信息
- 🔒 数据默认本地存储，不自动上传
- 🔒 输入验证和参数校验必须完备

## 📁 项目结构

```
handbox/
├── docs/                    # 项目文档
│   ├── requirements.md      # 产品需求 (人编写)
│   ├── architecture.md      # 架构设计
│   ├── tasks.md            # 任务清单 (AI生成)
│   └── development.md      # 开发指南
├── src/                    # 前端代码
│   ├── lib/
│   │   ├── components/     # UI 组件
│   │   ├── stores/         # Svelte stores
│   │   ├── api/           # IPC 封装
│   │   ├── types/         # TS 类型定义
│   │   └── utils/         # 工具函数
│   └── routes/            # 页面路由
├── src-tauri/             # 后端代码
│   ├── src/
│   │   ├── commands/      # IPC 命令 (AI开发)
│   │   ├── services/      # 业务逻辑 (AI开发)
│   │   ├── models/        # 数据模型 (AI开发)
│   │   └── utils/         # 工具函数
│   └── tests/             # 单元测试 (AI生成)
└── CLAUDE.md              # 本文件
```

## ⚙️ 常用开发命令

### 环境启动
```bash
# 开发模式
npm run tauri dev

# 类型检查
npm run check

# 后端测试
cargo test
```

### 质量检查 (必须通过)
```bash
# 前端检查
npm run check

# 后端检查
cargo fmt -- --check
cargo clippy -D warnings
cargo test

# 安全审计
cargo audit
npm audit
```

### AI 协作命令示例
```bash
# 任务分析
claude "分析当前任务，创建详细实现计划"

# 功能开发 (TDD)
claude "为 [功能] 先编写单元测试，然后实现功能"

# 代码审查
claude "审查最近的代码变更，检查安全性和代码质量"

# 文档更新
claude "根据代码变更更新相关技术文档"
```

## 🎯 AI 开发指导原则

### 开发流程 (必须遵循)
1. **探索** - 先阅读相关文件，理解现有代码
2. **计划** - 制定详细实现计划，确认技术路径
3. **编码** - 按 TDD 方式开发：测试 → 实现 → 重构
4. **审查** - 自动检查代码质量、安全性、性能
5. **提交** - 使用规范的 commit message

### 代码实现要求
- 🧪 **测试先行**: 先写测试，确保测试失败，再实现功能
- 🔍 **安全优先**: 所有用户输入必须验证，敏感数据必须加密
- 📝 **文档同步**: 重要功能需要更新相关文档
- ♻️ **可维护性**: 代码简洁、职责单一、易于扩展

### 错误处理模式
```rust
// Rust 后端错误结构
#[derive(Debug, serde::Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub hint: Option<String>,
}

// 常用错误码
// - VALIDATION_ERROR: 输入验证失败
// - AUTH_ERROR: 认证/授权错误
// - NETWORK_ERROR: 网络请求失败
// - RATE_LIMIT: 请求频率限制
// - INTERNAL_ERROR: 内部错误
```

### IPC 命令规范
```rust
// 命令命名: domain_action
#[tauri::command]
async fn chat_send(/* params */) -> Result<ChatResponse, AppError> {
    // 实现
}

#[tauri::command] 
async fn provider_probe(/* params */) -> Result<ProviderStatus, AppError> {
    // 实现
}
```

## 📊 代码质量标准

### 测试覆盖率要求
- 后端单元测试覆盖率 ≥ 80%
- 所有 public API 必须有测试
- 关键业务逻辑必须有集成测试

### 性能基准
- 应用冷启动: < 3 秒
- UI 操作响应: < 100ms  
- 空闲内存使用: < 500MB
- 并发会话支持: ≥ 10 个

### 安全检查清单
- [ ] API Key 安全存储
- [ ] 输入验证完备
- [ ] 无硬编码敏感信息
- [ ] 子进程安全管理
- [ ] 最小权限原则

## 🔄 AI 协作最佳实践

### 任务开始前
1. 阅读 `docs/requirements.md` 了解需求
2. 查看 `docs/tasks.md` 确认当前任务
3. 理解相关代码文件和架构

### 开发过程中
1. 遵循 TDD：测试 → 实现 → 重构
2. 频繁提交，每个功能点单独 commit
3. 及时更新文档和注释
4. 保持代码整洁和一致性

### 完成任务后
1. 运行完整质量检查
2. 更新相关文档
3. 标记任务状态为完成
4. 准备演示和说明

## 🚨 注意事项

### 绝对禁止
- ❌ 硬编码 API Key 或敏感信息
- ❌ 跳过单元测试直接实现
- ❌ 忽略 TypeScript 类型检查
- ❌ 提交未格式化的代码
- ❌ 未经验证的用户输入处理

### 特别关注
- ⚠️ 所有网络请求需要错误处理和重试机制
- ⚠️ 数据库操作需要事务管理
- ⚠️ 文件系统访问需要权限检查
- ⚠️ 子进程启动需要超时和资源限制

---

## 🤖 Claude Code 扩展机制

HandBox 配备了完整的 Claude Code 扩展系统（5000+ 行代码），提供专业化的开发工作流自动化。

### 可用的 Subagents（专业化 AI 助手）

Claude 会根据任务类型自动选择合适的 subagent：

**rust-backend** - Rust/Tauri 后端开发专家
- **何时使用**: 后端代码开发、IPC 命令、数据库操作、异步编程
- **专长**: async Rust、SQLx、错误处理、安全性、性能优化

**svelte-frontend** - SvelteKit/Svelte 5 前端专家
- **何时使用**: 前端组件开发、UI 实现、状态管理、样式设计
- **专长**: Svelte 5 runes ($state, $derived)、TypeScript、Tailwind 4.x

**security-reviewer** - 安全审查专家
- **何时使用**: 代码审查、安全检查、漏洞扫描、合规验证
- **专长**: API key 存储、SQL 注入、XSS、输入验证、命令注入

**database-inspector** - 数据库查询专家
- **何时使用**: 数据库问题调试、schema 检查、数据验证、迁移分析
- **专长**: SQLite 查询、migration 分析、外键检查、完整性验证

**rust-test-writer** - 单元测试编写专家
- **何时使用**: 编写单元测试、提升覆盖率、TDD 开发、测试重构
- **专长**: async 测试、mock 数据库、测试模式、覆盖率分析

### 可用的 Commands（快捷命令）

**Git 工作流:**
- `/worktree list` - 列出所有 worktrees
- `/worktree-flow start <branch> "<task>"` - 创建 worktree 开始开发
- `/worktree-flow complete` - 自动提交、合并、清理
- `/commit "message"` - 手动提交（Conventional Commits 格式）
- `/commit feat api "add streaming"` - 带类型和作用域的提交

**开发工具:**
- `/test` - 运行所有测试
- `/test coverage` - 生成 HTML 覆盖率报告（目标 ≥80%）
- `/test watch` - TDD 监视模式（文件变化自动运行）
- `/test mod services::chat` - 测试特定模块

**数据库调试:**
- `/db tables` - 查看所有表
- `/db schema <table>` - 查看表结构和字段
- `/db query <table> [limit]` - 查询数据
- `/db migrations` - 查看迁移历史
- `/db fk-check` - 检查外键约束
- `/db sql "SELECT ..."` - 执行自定义 SQL

### 自动化 Hooks

**PostToolUse（编辑后自动执行）:**
- **auto-format.sh**: 自动格式化代码（Rust: cargo fmt, 前端: prettier）

**UserPromptSubmit（发送消息前自动执行）:**
- **git-add.sh**: 自动暂存所有变更
- **worktree-context.sh**: 自动检测 worktree 环境并提供上下文

### Skills（智能能力）

**handbox-committer** - 智能提交助手
- **触发词**: "提交代码"、"commit this"、"完成了功能"、"可以提交了"
- **功能**: 自动分析变更、生成语义化 commit message、执行提交

### 使用示例

**场景1：开发新功能（完整 TDD 流程）**
```
User: "实现用户认证功能"

Claude:
1. [调用 rust-test-writer] 编写测试用例
2. [调用 rust-backend] 实现功能代码
3. [自动触发 auto-format.sh hook] 格式化代码
4. [使用 /test] 运行测试验证
5. [调用 handbox-committer skill] 生成提交

Result: 完整的 TDD 开发流程，代码质量有保障
```

**场景2：调试数据库问题**
```
User: "为什么会报 foreign key constraint failed？"

Claude: [调用 database-inspector subagent]
1. 运行 /db fk-check 检查外键约束
2. 运行 /db schema models 查看表结构
3. 运行 /db query providers 检查引用数据
4. 分析问题原因并提供修复方案

Result: 快速定位问题，提供具体修复步骤
```

**场景3：安全审查**
```
User: "审查最近的代码变更"

Claude: [调用 security-reviewer subagent]
1. 扫描硬编码的 API keys 或密钥
2. 检查 SQL 注入风险
3. 验证输入处理和验证逻辑
4. 检查 API key 存储方式
5. 生成详细的安全报告

Result: 全面的安全分析，确保代码安全
```

**场景4：Worktree 工作流**
```
User: "在 feature/new-ui worktree 上实现新界面"

Claude:
1. 运行 /worktree-flow start feature/new-ui main "实现新界面"
2. [调用 svelte-frontend] 开发 UI 组件
3. [自动触发 hooks] 暂存和格式化代码

User: "完成这个 worktree"

Claude:
1. [调用 handbox-committer skill] 生成提交信息
2. 运行 /worktree-flow complete
3. 自动合并到 main 分支并清理 worktree

Result: 流畅的 Git worktree 工作流，代码隔离清晰
```

### 配置文件位置

- **Subagents**: `.claude/agents/*.md`
- **Commands**: `.claude/commands/*.md`
- **Hooks**: `.claude/hooks/*.sh`
- **Skills**: `.claude/skills/*/SKILL.md`
- **配置**: `.claude/settings.json`

完整文档见: [.claude/commands/README.md](./.claude/commands/README.md)

---

## 📖 相关资源

- [产品需求文档](./docs/requirements.md)
- [架构设计文档](./docs/architecture.md)
- [开发指南](./docs/development.md)
- [任务管理清单](./docs/tasks.md)
- [Claude Code 扩展建议](./.claude/RECOMMENDATIONS.md)
- [Tauri 官方文档](https://tauri.app/)
- [SvelteKit 官方文档](https://kit.svelte.dev/)

通过遵循本指南，确保 AI 与人类开发者的高效协作，构建高质量的 HandBox 应用。