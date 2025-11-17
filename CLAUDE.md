# HandBox AI 协作指南

这是 HandBox 项目的 AI 协作配置文件，为 Claude 提供项目上下文和开发指导。

## 🎯 项目概述

HandBox 是一款基于 Tauri 2 + SvelteKit 5 的本地优先、隐私可控的多模型 AI 工作台。

**核心特性**:

- 多模型供应商管理（OpenAI、Anthropic、Google AI 等）
- 可复用聊天配置（Artifact 系统）
- MCP (Model Context Protocol) 集成
- 本地数据存储与全文搜索

## 🔧 技术约束与规范

### 前端 (SvelteKit + TypeScript + Tailwind CSS 4.x + Lucide Icons)

- ✅ **框架**: Svelte 5 + SvelteKit 5，严格 TypeScript，禁用 `any` 类型
- ✅ **样式**: Tailwind CSS 4.x，使用 `@theme` 指令和 CSS 变量，双主题支持
- ✅ **图标**: Lucide Svelte (`@lucide/svelte`)，语义化图标选择
- ✅ **组件结构**: 导入 → props/逻辑 → 模板 → Tailwind 样式类
- ✅ **Svelte 5 语法**: 优先使用 `$state`, `$derived` 等新语法
- ✅ **代码质量**: 使用 Prettier 格式化，通过 `npm run check` 类型检查

### 后端 (Tauri + Rust)

- ✅ 所有 `pub` 函数必须有单元测试
- ✅ 使用 `cargo fmt` 和 `cargo clippy` 保持代码质量
- ✅ IPC 命令遵循 `domain_action` 命名规范
- ✅ 错误处理使用统一格式：`{ code, message, hint }`
- ✅ 优先使用 `tokio` 异步编程

### 安全与隐私

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

## 🚨 注意事项

### 绝对禁止

- ❌ 硬编码 API Key 或敏感信息
- ❌ 跳过单元测试直接实现
- ❌ 忽略 TypeScript 类型检查
- ❌ 提交未格式化的代码
- ❌ 未经验证的用户输入处理
- ❌ **未经用户允许创建文档文件** (README.md, RECOMMENDATIONS.md 等)

### 特别关注

- ⚠️ 所有网络请求需要错误处理和重试机制
- ⚠️ 数据库操作需要事务管理
- ⚠️ 文件系统访问需要权限检查
- ⚠️ 子进程启动需要超时和资源限制

---

## 📖 相关资源

- [产品需求文档](./docs/requirements.md)
- [架构设计文档](./docs/architecture.md)
- [开发指南](./docs/development.md)
- [任务管理清单](./docs/tasks.md)
- [Tauri 官方文档](https://tauri.app/)
- [SvelteKit 官方文档](https://kit.svelte.dev/)

通过遵循本指南，确保 AI 与人类开发者的高效协作，构建高质量的 HandBox 应用。
