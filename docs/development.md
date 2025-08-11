# HandBox 开发指南

## 1. 开发环境配置

### 1.1 系统要求

#### 1.1.1 操作系统支持
- **Windows**: Windows 10 Version 1903 或更高版本
- **macOS**: macOS 10.15 或更高版本  
- **Linux**: 现代发行版 (Ubuntu 18.04+, Debian 10+, Arch Linux 等)

#### 1.1.2 硬件要求
- **CPU**: x64 或 ARM64 架构
- **内存**: 最少 4GB RAM，推荐 8GB+
- **存储**: 至少 2GB 可用空间

### 1.2 开发工具安装

#### 1.2.1 Node.js 环境
```bash
# 使用 nvm 安装 (推荐)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
nvm use 18

# 或直接下载安装
# https://nodejs.org/ (选择 LTS 版本)
```

#### 1.2.2 Rust 环境
```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 验证安装
rustc --version
cargo --version
```

#### 1.2.3 Tauri CLI
```bash
# 安装 Tauri CLI
npm install -g @tauri-apps/cli@latest

# 或使用 Cargo 安装
cargo install tauri-cli
```

#### 1.2.4 系统依赖

##### Windows
```bash
# 需要安装 Visual Studio Build Tools 或 Visual Studio Community
# https://visualstudio.microsoft.com/downloads/

# 安装 WebView2 (通常已预装)
# https://developer.microsoft.com/en-us/microsoft-edge/webview2/
```

##### macOS
```bash
# 安装 Xcode Command Line Tools
xcode-select --install
```

##### Linux (Ubuntu/Debian)
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
```

### 1.3 IDE 配置

#### 1.3.1 推荐 IDE
- **Visual Studio Code** (推荐)
- **WebStorm** 
- **Neovim** (高级用户)

#### 1.3.2 VSCode 插件
```json
{
  "recommendations": [
    "svelte.svelte-vscode",           // Svelte 支持
    "tauri-apps.tauri-vscode",        // Tauri 支持  
    "rust-lang.rust-analyzer",        // Rust 支持
    "bradlc.vscode-tailwindcss",      // Tailwind CSS 支持
    "ms-vscode.vscode-typescript-next", // TypeScript 支持
    "esbenp.prettier-vscode",         // 代码格式化
    "ms-vscode.vscode-json",          // JSON 支持
    "redhat.vscode-yaml"              // YAML 支持
  ]
}
```

#### 1.3.3 VSCode 设置
```json
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.cargo.features": "all",
  "svelte.enable-ts-plugin": true,
  "typescript.preferences.importModuleSpecifier": "relative",
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": {
    "source.fixAll": true
  }
}
```

## 2. 项目结构详解

### 2.1 根目录结构
```
handbox/
├── .github/              # GitHub Actions 工作流
├── .vscode/              # VSCode 配置文件
├── docs/                 # 项目文档
├── src/                  # Svelte 前端源码
├── src-tauri/            # Tauri 后端源码
├── static/               # 静态资源文件
├── tests/                # 测试文件
├── package.json          # Node.js 依赖配置
├── svelte.config.js      # Svelte 配置
├── tsconfig.json         # TypeScript 配置
├── vite.config.js        # Vite 构建配置
├── tailwind.config.js    # Tailwind CSS 配置
└── README.md             # 项目说明
```

### 2.2 前端结构 (src/)
```
src/
├── lib/                  # 共享组件和工具
│   ├── components/       # UI 组件
│   │   ├── ui/          # 基础 UI 组件
│   │   ├── chat/        # 对话相关组件
│   │   ├── prompt/      # Prompt 管理组件
│   │   └── agent/       # Agent 管理组件
│   ├── stores/          # Svelte stores 状态管理
│   ├── types/           # TypeScript 类型定义
│   ├── utils/           # 工具函数
│   ├── api/             # API 调用封装
│   └── styles/          # 全局样式文件
├── routes/              # SvelteKit 路由
│   ├── +layout.svelte   # 根布局
│   ├── +layout.ts       # 布局逻辑
│   ├── +page.svelte     # 首页
│   ├── chat/            # 对话页面
│   ├── prompts/         # Prompt 管理页面
│   ├── agents/          # Agent 管理页面
│   └── settings/        # 设置页面
└── app.html             # HTML 入口模板
```

### 2.3 后端结构 (src-tauri/src/)
```
src-tauri/src/
├── commands/            # Tauri 命令处理器
│   ├── mod.rs          # 模块导出
│   ├── chat.rs         # 对话相关命令
│   ├── prompt.rs       # Prompt 管理命令
│   ├── agent.rs        # Agent 管理命令
│   ├── llm.rs          # LLM API 命令
│   └── file.rs         # 文件操作命令
├── services/            # 业务服务层
│   ├── mod.rs          # 模块导出
│   ├── llm_service.rs  # LLM 集成服务
│   ├── memory_service.rs # 记忆系统服务
│   ├── tool_service.rs # 工具集成服务
│   └── database_service.rs # 数据库服务
├── models/              # 数据模型
│   ├── mod.rs          # 模块导出
│   ├── chat.rs         # 对话模型
│   ├── prompt.rs       # Prompt 模型
│   ├── agent.rs        # Agent 模型
│   └── user.rs         # 用户模型
├── providers/           # LLM 提供商实现
│   ├── mod.rs          # 模块导出
│   ├── openai.rs       # OpenAI 实现
│   ├── claude.rs       # Claude 实现
│   └── base.rs         # 基础接口
├── utils/               # 工具函数
│   ├── mod.rs          # 模块导出
│   ├── crypto.rs       # 加密工具
│   ├── validation.rs   # 输入验证
│   └── logger.rs       # 日志工具
├── config/              # 配置管理
│   ├── mod.rs          # 模块导出
│   └── app_config.rs   # 应用配置
├── main.rs             # 应用入口
└── lib.rs              # 库入口
```

## 3. 开发流程

### 3.1 克隆项目
```bash
git clone https://github.com/your-username/handbox.git
cd handbox
```

### 3.2 安装依赖
```bash
# 安装前端依赖
npm install

# 安装 Rust 依赖 (自动处理)
cd src-tauri
cargo fetch
cd ..
```

### 3.3 开发模式启动
```bash
# 启动开发服务器
npm run tauri dev

# 或使用 Tauri CLI
tauri dev
```

### 3.4 构建打包
```bash
# 构建生产版本
npm run tauri build

# 构建特定平台 (如果支持交叉编译)
npm run tauri build -- --target x86_64-pc-windows-msvc
```

## 4. 代码规范

### 4.1 Git 提交规范

#### 4.1.1 提交消息格式
```
<type>(<scope>): <subject>

<body>

<footer>
```

#### 4.1.2 类型说明
- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式调整
- `refactor`: 代码重构
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建过程或辅助工具变动

#### 4.1.3 示例
```bash
git commit -m "feat(chat): add streaming message support"
git commit -m "fix(llm): handle API timeout errors"
git commit -m "docs(api): update API documentation"
```

### 4.2 分支管理策略

```
main              # 主分支，用于生产发布
├── develop       # 开发分支，集成各个功能
├── feature/*     # 功能分支
├── hotfix/*      # 紧急修复分支
└── release/*     # 发布分支
```

### 4.3 代码风格

#### 4.3.1 TypeScript/JavaScript 规范
```typescript
// 使用 Prettier 格式化
// .prettierrc
{
  "semi": true,
  "trailingComma": "es5",
  "singleQuote": true,
  "printWidth": 80,
  "tabWidth": 2,
  "useTabs": false
}
```

#### 4.3.2 Rust 代码规范
```toml
# .rustfmt.toml
edition = "2021"
max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Unix"
```

#### 4.3.3 Svelte 组件规范
```svelte
<script lang="ts">
  // 1. 导入声明
  import { onMount } from 'svelte';
  import type { ChatSession } from '$lib/types';
  
  // 2. 组件属性
  export let session: ChatSession;
  export let disabled = false;
  
  // 3. 响应式变量
  let loading = false;
  $: canSend = !loading && !disabled;
  
  // 4. 函数定义
  async function handleSubmit() {
    // 实现逻辑
  }
  
  // 5. 生命周期
  onMount(() => {
    // 初始化逻辑
  });
</script>

<!-- 6. HTML 模板 -->
<div class="container">
  <!-- 组件内容 -->
</div>

<!-- 7. 样式 -->
<style>
  .container {
    /* 样式定义 */
  }
</style>
```

## 5. 测试指南

### 5.1 前端测试

#### 5.1.1 安装测试依赖
```bash
npm install -D @playwright/test vitest @testing-library/svelte
```

#### 5.1.2 单元测试配置
```typescript
// vitest.config.ts
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: !process.env.VITEST })],
  test: {
    environment: 'jsdom',
    setupFiles: ['src/setupTest.ts']
  }
});
```

#### 5.1.3 组件测试示例
```typescript
// src/lib/components/ChatInput.test.ts
import { render, screen, fireEvent } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import ChatInput from './ChatInput.svelte';

test('should send message when enter is pressed', async () => {
  const { component } = render(ChatInput);
  
  const input = screen.getByRole('textbox');
  await fireEvent.input(input, { target: { value: 'Hello' } });
  await fireEvent.keyDown(input, { key: 'Enter' });
  
  // 验证消息发送
});
```

### 5.2 后端测试

#### 5.2.1 测试依赖
```toml
# Cargo.toml [dev-dependencies]
tokio-test = "0.4"
mockall = "0.11"
tempfile = "3.0"
```

#### 5.2.2 单元测试示例
```rust
// src-tauri/src/services/llm_service.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_chat_completion() {
        let service = LLMService::new();
        let request = ChatRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![ChatMessage::user("Hello")],
            ..Default::default()
        };

        let response = service.chat_completion(request).await;
        assert!(response.is_ok());
    }
}
```

### 5.3 集成测试

#### 5.3.1 端到端测试
```typescript
// tests/e2e/chat.spec.ts
import { test, expect } from '@playwright/test';

test('should create and use chat session', async ({ page }) => {
  await page.goto('/');
  
  // 创建新对话
  await page.click('[data-testid="new-chat"]');
  await page.fill('[data-testid="session-name"]', 'Test Chat');
  await page.click('[data-testid="create-session"]');
  
  // 发送消息
  await page.fill('[data-testid="chat-input"]', 'Hello AI!');
  await page.press('[data-testid="chat-input"]', 'Enter');
  
  // 验证消息发送和接收
  await expect(page.locator('[data-testid="user-message"]')).toContainText('Hello AI!');
  await expect(page.locator('[data-testid="ai-message"]')).toBeVisible();
});
```

## 6. 调试技巧

### 6.1 前端调试

#### 6.1.1 浏览器开发者工具
```typescript
// 在组件中添加调试点
console.log('Debug info:', { variable });
debugger; // 设置断点
```

#### 6.1.2 Svelte DevTools
```bash
# 安装 Svelte DevTools 浏览器扩展
# Chrome: https://chrome.google.com/webstore/detail/svelte-devtools/
# Firefox: https://addons.mozilla.org/en-US/firefox/addon/svelte-devtools/
```

### 6.2 后端调试

#### 6.2.1 Rust 日志
```rust
use tracing::{info, warn, error, debug};

#[tauri::command]
async fn my_command() -> Result<(), String> {
    info!("Command executed");
    debug!("Debug information: {:?}", data);
    Ok(())
}
```

#### 6.2.2 Tauri 调试配置
```rust
// main.rs
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 6.3 数据库调试

#### 6.3.1 SQLite 调试
```bash
# 安装 sqlite3 命令行工具
# 查看数据库结构
sqlite3 app.db ".schema"

# 查询数据
sqlite3 app.db "SELECT * FROM chat_sessions;"
```

## 7. 性能优化

### 7.1 前端性能

#### 7.1.1 代码分割
```typescript
// 动态导入组件
const LazyComponent = lazy(() => import('./LazyComponent.svelte'));
```

#### 7.1.2 虚拟滚动
```svelte
<!-- 大列表虚拟化 -->
<VirtualList items={messages} itemHeight={60} let:item>
  <MessageItem message={item} />
</VirtualList>
```

### 7.2 后端性能

#### 7.2.1 异步操作
```rust
// 使用异步操作提高并发性能
use tokio::task;

async fn process_multiple_requests(requests: Vec<Request>) -> Vec<Response> {
    let futures = requests.into_iter().map(|req| {
        task::spawn(async move { process_request(req).await })
    });
    
    futures_util::future::join_all(futures).await
        .into_iter()
        .map(|result| result.unwrap())
        .collect()
}
```

#### 7.2.2 数据库优化
```rust
// 使用连接池
let pool = SqlitePool::connect_with(
    SqliteConnectOptions::new()
        .filename("database.db")
        .create_if_missing(true)
).await?;

// 批量操作
sqlx::query("INSERT INTO messages (content) VALUES (?)")
    .bind_all(messages)
    .execute(&pool)
    .await?;
```

## 8. 部署指南

### 8.1 构建配置

#### 8.1.1 Tauri 配置
```json
// tauri.conf.json
{
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev",
    "frontendDist": "../build"
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "identifier": "com.gumpw.handbox",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

### 8.2 GitHub Actions CI/CD

#### 8.2.1 构建工作流
```yaml
# .github/workflows/build.yml
name: Build and Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]

    steps:
    - uses: actions/checkout@v3
    - uses: actions/setup-node@v3
      with:
        node-version: 18
    - uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    
    - run: npm install
    - run: npm run tauri build
    
    - uses: actions/upload-artifact@v3
      with:
        name: ${{ matrix.os }}-build
        path: src-tauri/target/release/bundle/
```

### 8.3 发布流程

1. **版本标记**: 创建新的 git tag
2. **自动构建**: GitHub Actions 自动构建各平台版本
3. **发布**: 上传构建产物到 GitHub Releases

```bash
# 创建新版本
git tag -a v1.0.0 -m "Release version 1.0.0"
git push origin v1.0.0
```

## 9. 故障排除

### 9.1 常见问题

#### 9.1.1 Tauri 构建失败
```bash
# 清理缓存
cargo clean
rm -rf node_modules
npm install

# 更新 Rust 工具链
rustup update stable
```

#### 9.1.2 前端依赖冲突
```bash
# 删除 package-lock.json 和 node_modules
rm package-lock.json
rm -rf node_modules
npm install
```

### 9.2 日志分析

#### 9.2.1 应用日志位置
- **Windows**: `%APPDATA%/com.gumpw.handbox/logs/`
- **macOS**: `~/Library/Application Support/com.gumpw.handbox/logs/`
- **Linux**: `~/.config/com.gumpw.handbox/logs/`

#### 9.2.2 启用详细日志
```rust
// 在开发模式下启用详细日志
#[cfg(debug_assertions)]
tracing_subscriber::fmt::init();
```

## 10. 贡献指南

### 10.1 贡献流程

1. Fork 项目到个人仓库
2. 创建功能分支: `git checkout -b feature/new-feature`
3. 提交更改: `git commit -m 'feat: add new feature'`
4. 推送分支: `git push origin feature/new-feature`
5. 创建 Pull Request

### 10.2 代码审查标准

- 代码符合项目规范
- 包含必要的测试
- 文档更新完整
- 无明显性能问题
- 向后兼容性考虑

### 10.3 问题报告

提交 Issue 时请包含：
- 操作系统和版本
- 应用版本号
- 详细的错误描述
- 复现步骤
- 相关日志信息

这份开发指南将帮助开发团队快速上手项目开发，确保代码质量和开发效率。