# HandBox 开发指南

本指南帮助你在本地搭建、开发、调试与构建 HandBox。更高层次的系统架构请参考 `docs/architecture.md`，产品能力参见 `docs/requirements.md`。

## 1. 环境准备

- Node.js 18+
- Rust stable
- 平台依赖：
  - macOS：Xcode Command Line Tools（`xcode-select --install`）
  - Windows：Visual Studio Build Tools、WebView2
  - Linux：`libwebkit2gtk-4.0-dev`、`libssl-dev`、`libgtk-3-dev` 等

安装 CLI：
```bash
npm i -g @tauri-apps/cli
```

## 2. 安装与启动

```bash
# 安装依赖
npm install

# 开发模式（前端 HMR + Tauri）
npm run tauri dev

# 类型检查
npm run check

# 生产构建
npm run tauri build
```

## 3. 项目结构（当前）

```
handbox/
├── docs/
│   ├── architecture.md
│   ├── requirements.md
│   └── resources/
├── src/
│   ├── app.html
│   └── routes/
│       ├── +layout.ts
│       └── +page.svelte
├── src-tauri/
│   ├── build.rs
│   ├── capabilities/
│   │   └── default.json
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   └── main.rs
│   └── tauri.conf.json
├── static/
│   ├── favicon.png
│   ├── svelte.svg
│   ├── tauri.svg
│   └── vite.svg
├── package.json
├── package-lock.json
├── svelte.config.js
├── tsconfig.json
└── vite.config.js
```

规划中的子目录（逐步落地）：
- 前端：`src/lib/{components,stores,api,types,utils}`、`routes/{chat,settings,artifacts,search}`
- 后端：`src-tauri/src/{commands,services,models,utils,config}`

## 4. 常用命令

```bash
npm run tauri dev     # 启动开发
npm run check         # 类型与 Svelte 校验
npm run tauri build   # 生产构建
```

## 5. 前后端交互（示例）

前端：
```ts
import { invoke } from '@tauri-apps/api/core';

const msg = await invoke<string>('greet', { name: 'HandBox' });
```

后端（Rust，`src-tauri/src/lib.rs`）：
```rust
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
```

## 6. 调试建议

- DevTools：开发模式下打开 WebView 开发者工具
- 日志：建议引入 `tracing`，按需在 `debug_assertions` 下初始化
- 热重载：前端使用 Vite HMR（1420/1421），后端由 Tauri 管理

## 7. 项目规范（统一标准）

### 7.1 原则（强制）

- 质量左移：尽可能通过 lint 与单元测试提前发现问题。
- 前端与后端都必须通过 lint 检查；后端必须编写并通过单元测试；前端暂不要求单元测试。
- 合并前至少需通过：
  - 前端：`npm run check`
  - 后端：`cargo fmt -- --check`、`cargo clippy -D warnings`、`cargo test`

### 7.2 版本、分支与提交

- 版本：SemVer（主.次.补），预发布：`-alpha/-beta/-rc`
- 分支：`main`（稳定）、`develop`（集成）、`feature/*`、`hotfix/*`、`release/*`
- 提交规范：Conventional Commits，格式：`<type>(<scope>): <subject>`
  - 例：`feat(chat): add streaming support`

### 7.3 代码风格

- TypeScript/Svelte：严格模式开启；组件组织建议“导入/props/逻辑 → 模板 → 样式”。
- Rust：面向接口抽象 Provider/MCP；保持 `cargo fmt` 与 `clippy` 零告警。
- 格式化：TS/Svelte/JSON 使用 Prettier（建议）；Rust 使用 rustfmt。
- 命名：语义化与完整单词；函数用动词，变量用名词；避免 1–2 字母名。

### 7.4 IPC 与错误结构

- 命令命名：领域前缀 + 动词，下划线分隔，例如：
  - `chat_send`、`provider_probe`、`provider_list_models`、`artifact_save`、`search_messages`
- 参数约束：`sessionId | artifactId | inlineConfig` 三选一，附件带 `mime/path`。
- 错误统一结构：
  ```json
  { "code": "RATE_LIMIT|AUTH_ERROR|NETWORK_ERROR|VALIDATION_ERROR|INTERNAL_ERROR", "message": "可读提示", "hint": "操作建议" }
  ```
- 流式事件：命名 `<domain>.<topic>`，如 `chat.delta`, `chat.done`, `chat.error`。

### 7.5 数据与迁移

- 存储：推荐 SQLite + FTS5；统一服务封装访问；文件位于系统应用数据目录。
- Schema 版本：使用 `PRAGMA user_version`；迁移脚本位于 `src-tauri/migrations/`；禁止破坏性更改不迁移。
- 导入/导出：会话级 JSON 与全量备份；敏感信息（API Key）永不导出。

### 7.6 安全与隐私

- API Key：优先使用 OS Keychain 保存；若降级为本地加密，需明确风险提示。
- 最小权限：仅开放必要插件/能力；文件系统访问白名单。
- 日志：不得输出密钥/用户数据；调试日志仅在开发环境开启。
- MCP/子进程：参数白名单、超时/内存上限、退出码校验与强制回收。

### 7.7 UI/UX 基线

- 主题/语言：浅色/深色与系统跟随；中英文即时切换。
- 聊天卡片：类型可扩展（思考过程、工具调用、代码执行）；长文折叠与一键复制。
- 可访问性：键盘可达；最小窗口宽度 960px。

### 7.8 完成定义（DoD）

- 本地构建通过，关键路径无控制台错误与未处理异常。
- 前端 `npm run check` 通过；后端 `fmt`/`clippy`/`test` 全绿。
- 错误与空态可见、可重试；必要文档更新（README/开发/架构）。

### 7.9 本地质量检查命令

```bash
# 前端 Lint（必需）
npm run check

# 后端格式与静态检查（必需）
cargo fmt -- --check
cargo clippy -D warnings

# 后端单元测试（必需）
cargo test
```

## 8. 构建产物

- 各平台安装包由 Tauri 生成，产物路径位于 `src-tauri/target/release/bundle/`

## 9. 进一步阅读

- 产品需求（PRD）：`docs/requirements.md`
- 架构设计：`docs/architecture.md`


