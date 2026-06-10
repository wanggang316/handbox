# HandBox 开发指南

本指南定义了 HandBox 项目的完整开发工作流，基于个人开发者与 AI 协作的最佳实践，旨在通过人机协作实现高质量、高效率的软件开发。

## 🚀 环境准备

### 系统要求

- Node.js 18+
- Rust stable
- Git 2.30+

### 平台依赖

- **macOS**: Xcode Command Line Tools (`xcode-select --install`)
- **Windows**: Visual Studio Build Tools、WebView2
- **Linux**: `libwebkit2gtk-4.0-dev`、`libssl-dev`、`libgtk-3-dev`

### 开发工具安装

```bash
# Tauri CLI
npm i -g @tauri-apps/cli

# 项目依赖
npm install

# 验证环境
npm run check
cargo fmt -- --check
cargo test
```

## 📐 项目结构

```
handbox/
├── docs/                          # 📚 项目文档
│   ├── requirements.md            # 产品需求文档 (人主导)
│   ├── architecture.md            # 架构设计文档
│   ├── development.md             # 本开发指南
│   └── resources/                 # 设计资源 (人主导)
├── src/                           # 🎨 前端代码
│   ├── lib/                       # 核心库
│   │   ├── components/            # UI 组件
│   │   ├── stores/                # 状态管理
│   │   ├── api/                   # IPC 封装
│   │   ├── types/                 # TypeScript 类型
│   │   └── utils/                 # 工具函数
│   └── routes/                    # 页面路由
│       ├── chat/                  # 聊天界面
│       ├── settings/              # 设置页面
│       ├── artifacts/             # Artifact 管理
│       └── search/                # 历史搜索
├── src-tauri/                     # ⚙️ 后端代码
│   ├── src/
│   │   ├── commands/              # IPC 命令 (AI开发)
│   │   ├── services/              # 业务逻辑 (AI开发)
│   │   ├── models/                # 数据模型 (AI开发)
│   │   ├── utils/                 # 工具函数
│   │   └── config/                # 配置管理
│   ├── migrations/                # 数据库迁移
│   └── tests/                     # 单元测试 (AI开发)
├── .github/                       # 🔄 CI/CD 配置
│   └── workflows/                 # GitHub Actions
└── CLAUDE.md                      # 🤖 AI 协作指南
```

## ⚙️ 开发命令参考

### 日常开发

```bash
# 启动开发环境
npm run tauri dev

# 类型检查
npm run check

# 代码格式化
npm run format
cargo fmt

# 运行测试
cargo test
npm test
```

> ⚠️ **rustfmt 注意**：仓库存在大量历史上未按 rustfmt 排版的已提交代码，裸跑 `cargo fmt` 会重排全 crate、污染无关 diff。提交前只对本次触碰的文件做定向格式化/检查：
>
> ```bash
> rustfmt --edition 2021 src-tauri/src/path/to/touched_file.rs
> rustfmt --edition 2021 --check src-tauri/src/path/to/touched_file.rs
> ```
>
> 另注意 stdin 模式（`rustfmt --check < file`）恒返回 exit 0，不能用作门禁；必须以文件路径调用。

### 质量检查

```bash
# 完整质量检查
npm run check && \
cargo fmt -- --check && \
cargo clippy -D warnings && \
cargo test && \
cargo audit

# 构建验证
npm run tauri build
```

## 🔒 安全与隐私规范

### 数据隐私

- ✅ 数据默认本地存储
- ✅ 用户显式授权第三方调用
- ✅ 敏感信息排除导出
- ❌ 未经授权的数据上传

### 代码安全

- ✅ 输入验证和参数校验
- ✅ 最小权限原则
- ✅ 安全的子进程管理
- ❌ SQL 注入或 XSS 漏洞

## 📊 性能基准

### 启动性能

- 冷启动时间: < 3 秒
- 首次安装: < 5 秒
- 热重载响应: < 100ms

### 运行时性能

- 空闲内存使用: < 500MB
- 并发会话支持: ≥ 10 个
- UI 操作响应: < 100ms

### 构建性能

- 增量编译: < 30 秒
- 完整构建: < 2 分钟
- 打包输出: < 5 分钟

## 🚀 持续集成 (CI/CD)

### GitHub Actions 工作流

**PR 检查流程**:

```yaml
# .github/workflows/pr-check.yml
- 代码格式检查
- 静态分析
- 单元测试
- 构建验证
- 依赖安全审计
```

**发布流程**:

```yaml
# .github/workflows/release.yml
- 版本标签检查
- 完整测试套件
- 多平台构建
- 自动发布
```

### 本地预检查

```bash
# 提交前检查 (推荐配置为 git hook)
npm run check && \
cargo fmt -- --check && \
cargo clippy -D warnings && \
cargo test
```

---

## 📖 相关文档

- [产品需求文档](./requirements.md) - 功能需求和用户故事
- [架构设计文档](./architecture.md) - 技术架构和设计决策
- [Anthropic Claude Code 最佳实践](https://www.anthropic.com/engineering/claude-code-best-practices)
