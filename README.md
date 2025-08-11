# HandBox - 个人AI工具平台

<div align="center">
  <img src="static/tauri.svg" alt="HandBox Logo" width="120" height="120">
  
  <p align="center">
    <strong>一款功能强大的原生化个人AI工具平台</strong>
    <br />
    基于 Tauri + Svelte + TypeScript 构建
  </p>

  <p align="center">
    <img src="https://img.shields.io/badge/Tauri-2.0-blue.svg" alt="Tauri">
    <img src="https://img.shields.io/badge/Svelte-5.0-ff3e00.svg" alt="Svelte">
    <img src="https://img.shields.io/badge/TypeScript-5.6-3178c6.svg" alt="TypeScript">
    <img src="https://img.shields.io/badge/Rust-1.70+-000000.svg" alt="Rust">
    <img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License">
  </p>
</div>

## ✨ 功能特性

### 🤖 多模型集成
- 支持 OpenAI GPT 系列、Claude、Gemini 等主流大语言模型
- 统一的 API 管理界面，一键切换不同模型
- 实时费用监控和使用量统计

### 💬 智能对话系统
- 类似 ChatGPT 的直观对话界面
- 多会话管理，支持会话导入导出
- 自定义模型参数（Temperature、Top-p 等）
- 支持 Markdown 渲染和代码高亮

### 📝 Prompt 管理
- 个人 Prompt 模板库，支持分类和标签
- 可视化 Prompt 编辑器，支持变量占位符
- 版本控制和历史记录管理
- Prompt 模板导入导出和分享

### 🎯 Agent 智能助手
- 可视化 Agent 创建和配置平台
- 多层记忆系统：短期记忆 + 长期记忆 + 向量检索
- 内置工具集成：文件操作、代码执行、网络搜索
- MCP (Model Context Protocol) 协议支持
- CoT (Chain-of-Thought) 思维链推理

### 🔧 强大工具集
- 安全的代码执行环境
- 文档解析和知识提取
- 数据分析和可视化
- 自定义工具和插件支持

## 🚀 快速开始

### 环境要求

- **Node.js** 18.0+
- **Rust** 1.70+
- **系统要求**: Windows 10+, macOS 10.15+, Linux (现代发行版)

### 安装步骤

1. **克隆项目**
   ```bash
   git clone https://github.com/your-username/handbox.git
   cd handbox
   ```

2. **安装依赖**
   ```bash
   # 安装前端依赖
   npm install
   
   # 安装 Tauri CLI (如果尚未安装)
   npm install -g @tauri-apps/cli@latest
   ```

3. **开发模式启动**
   ```bash
   npm run tauri dev
   ```

4. **构建生产版本**
   ```bash
   npm run tauri build
   ```

### 系统依赖安装

#### Windows
```bash
# 需要安装 Visual Studio Build Tools
# 下载地址: https://visualstudio.microsoft.com/downloads/
```

#### macOS
```bash
xcode-select --install
```

#### Linux (Ubuntu/Debian)
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

## 📚 文档

- [产品需求文档 (PRD)](docs/PRD.md) - 详细功能需求和用户场景
- [技术架构文档](docs/ARCHITECTURE.md) - 系统架构和技术选型
- [API 设计文档](docs/API.md) - 接口设计和使用说明
- [开发指南](docs/DEVELOPMENT.md) - 开发环境配置和编码规范
- [Claude AI 项目说明](CLAUDE.md) - 给 AI 助手的项目概述

## 🛠️ 技术栈

### 前端
- **Svelte 5** - 现代化前端框架
- **SvelteKit** - 全栈框架，提供路由和构建工具
- **TypeScript** - 类型安全的 JavaScript
- **Tailwind CSS** - 实用优先的 CSS 框架
- **Vite** - 快速的构建工具

### 后端
- **Tauri 2.0** - 轻量级桌面应用框架
- **Rust** - 系统级编程语言，提供安全和性能
- **SQLite** - 嵌入式数据库
- **Tokio** - 异步运行时

### AI 集成
- **OpenAI API** - GPT 系列模型
- **Anthropic Claude API** - Claude 系列模型
- **Google Gemini API** - Gemini 系列模型
- **向量数据库** - 语义检索和记忆系统

## 🏗️ 项目结构

```
handbox/
├── src/                    # Svelte 前端源码
│   ├── lib/               # 共享组件和工具
│   ├── routes/            # SvelteKit 路由页面
│   └── app.html           # 应用入口模板
├── src-tauri/             # Tauri 后端源码
│   ├── src/               # Rust 源代码
│   ├── Cargo.toml         # Rust 依赖配置
│   └── tauri.conf.json    # Tauri 应用配置
├── docs/                  # 项目文档
├── static/                # 静态资源文件
├── package.json           # Node.js 依赖配置
└── README.md              # 项目说明文件
```

## 🔒 安全特性

- **本地存储**: 所有数据存储在本地，保护用户隐私
- **API 密钥加密**: 使用 AES-256 加密存储 API 密钥
- **沙箱执行**: 代码执行在安全的隔离环境中
- **最小权限**: 仅申请必要的系统权限

## 📊 开发状态

- [x] 项目架构设计
- [x] 基础框架搭建  
- [x] 文档编写完成
- [ ] LLM API 集成
- [ ] 对话系统实现
- [ ] Prompt 管理功能
- [ ] Agent 平台开发
- [ ] 记忆系统构建
- [ ] 工具集成实现
- [ ] UI/UX 优化
- [ ] 测试和优化

## 🤝 贡献指南

我们欢迎所有形式的贡献！请查看 [开发指南](docs/DEVELOPMENT.md) 了解如何参与项目开发。

### 贡献流程
1. Fork 本仓库
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'feat: add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建 Pull Request

## 📄 许可证

本项目采用 MIT 许可证。查看 [LICENSE](LICENSE) 文件了解更多详情。

## 🙏 致谢

感谢以下开源项目为 HandBox 提供的基础支持：

- [Tauri](https://tauri.app/) - 构建桌面应用的现代框架
- [Svelte](https://svelte.dev/) - 轻量级前端框架
- [Rust](https://www.rust-lang.org/) - 系统编程语言

## 📞 联系我们

- 项目主页: [GitHub Repository](https://github.com/your-username/handbox)
- 问题反馈: [GitHub Issues](https://github.com/your-username/handbox/issues)
- 邮箱: your-email@example.com

---

<div align="center">
  <sub>使用 ❤️ 和 Rust 构建</sub>
</div>
