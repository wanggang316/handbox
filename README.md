# HandBox

本地优先、隐私可控的一站式多模型 AI 工作台。基于 Tauri 2 + SvelteKit 5 + TypeScript 构建，提供多模型会话、可复用聊天配置（Artifact）、MCP 集成、历史搜索与本地持久化能力。

## 特性

- 多模型与供应商管理：探活检测、获取模型列表、禁用/删除与自定义兼容供应商
- 聊天与配置：系统提示词、模型参数（Temperature、Top-P、最大 Token、上下文数）、流式输出
- Artifact：将聊天配置落盘复用，列表预览、重命名、删除与基于 Artifact 新建会话
- MCP：本地 JSON 管理、开关启停、在聊天中勾选关联与执行过程展示
- 历史搜索：本地全文索引，按关键词搜索消息正文与系统提示词，结果预览与定位
- 本地存储：会话与配置本地化、导入/导出 JSON、异常退出恢复
- 安全：API Key 使用 OS Keychain 加密存储，默认不出本地

完整产品需求参见：[docs/requirements.md](docs/requirements.md)

## 技术栈

- 桌面容器：Tauri 2（Rust）
- 前端：Svelte 5 + SvelteKit 2（SPA 模式）
- 构建：Vite 6
- 语言：TypeScript（前端）与 Rust（后端）

## 环境要求

- Node.js 18+
- Rust stable（含各平台 Tauri 构建依赖）
- 平台依赖：
  - macOS：Xcode Command Line Tools（`xcode-select --install`）
  - Windows：Visual Studio Build Tools 与 WebView2
  - Linux：`libwebkit2gtk-4.0-dev`、`libssl-dev`、`libgtk-3-dev` 等

## 快速开始

```bash
# 克隆与安装
git clone <repo>
cd handbox
npm install

# 开发模式（前端 HMR + Tauri）
npm run tauri dev

# 类型检查
npm run check

# 生产构建
npm run tauri build
```

## 目录结构（当前）

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

## 文档

- 产品需求（PRD）：[docs/requirements.md](docs/requirements.md)
- 架构设计：[docs/architecture.md](docs/architecture.md)
- 开发指南：[docs/development.md](docs/development.md)

## 贡献

欢迎通过 Pull Request 参与：

1. Fork 仓库并创建分支（`feature/*`）
2. 提交前执行 `npm run check`
3. 补充必要说明与文档链接

提交规范与更多细节，参见：[docs/development.md](docs/development.md)

## 许可

MIT


