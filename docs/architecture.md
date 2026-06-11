HandBox 架构设计
------

## 1. 架构总览

基于 Tauri 2 + SvelteKit 5 + TypeScript 的本地优先桌面应用。整体采用前后端同仓的双运行时模型：
- 前端运行在系统 WebView 中（SvelteKit，SPA 模式，`ssr = false`），负责 UI、交互与轻量状态管理。
- 后端运行在 Tauri 原生进程中（Rust），负责安全能力封装、模型与系统资源访问、数据持久化、性能关键路径与隐私保护。
- 前后端通过 Tauri IPC 调用进行通信（`@tauri-apps/api` ↔ Rust `#[tauri::command]`）。

目标对齐 PRD：
- 多模型工作台（模型与供应商管理、探活检测、模型列表获取与过滤）。
- 会话与配置持久化（会话、系统提示词、模型参数、MCP 绑定等）。
- Artifact 复用配置（可落盘、预览、复用）。
- MCP 集成（本地 JSON 配置驱动、可启停）与过程可视化。
- 本地搜索（消息/提示词检索与跳转）。
- 隐私安全（Key 本地加密、数据默认不出本地）。

## 2. 技术栈与运行时

- UI：Svelte 5 + SvelteKit 2（`@sveltejs/adapter-static`，SPA 模式）
- 构建：Vite 6
- 原生容器：Tauri 2
- 语言：TypeScript（前端）、Rust（后端）
- 依赖基线：见 `package.json` 与 `src-tauri/Cargo.toml`
  - 已启用插件：`tauri-plugin-opener`
  - 建议后续引入：`tauri-plugin-shell`（子进程/MCP 启动）、`keyring`（安全凭据存储）、`serde`/`serde_json`（配置与数据序列化）、`rusqlite` 或 `sqlx`（SQLite/FTS）、`tokio`（异步）

运行模式：
- 开发：`tauri dev`，前端 HMR 端口 1420/1421，原生进程热重载。
- 生产：Tauri 打包（Windows/MSI、macOS/DMG、Linux/AppImage 等）。

## 3. 模块划分

### 3.1 前端（SvelteKit + Tailwind CSS 4.x + Lucide Icons）

#### 技术栈
- **框架**: Svelte 5 + SvelteKit 5 (TypeScript)
- **样式系统**: Tailwind CSS 4.x（使用 `@theme` 指令和CSS变量）
- **图标库**: Lucide Svelte (`@lucide/svelte`)
- **构建工具**: Vite 6 + `@tailwindcss/vite` 插件

#### 样式架构
- **主题系统**: 基于CSS变量的双主题支持（浅色/深色）
- **设计tokens**: 在 `src/app.css` 中通过 `@theme` 指令定义
- **组件样式**: Tailwind utility classes + 必要时的组件级CSS
- **响应式设计**: 桌面优先，最小宽度960px

#### 路由与页面
- `routes/+page.svelte`：聊天主页
- `routes/chat/`：会话管理页面
- `routes/settings/`：应用设置页面（已实现models子路由）
- `routes/artifacts/`：Artifact 管理页面  
- `routes/search/`：历史搜索页面

#### UI 组件系统
- **基础组件**: Button、Input、Toggle、Select、Modal等（使用Tailwind + Lucide图标）
- **业务组件**: 消息卡片（类型可扩展）、模型选择弹窗、设置面板、搜索面板、Artifact 卡片
- **布局组件**: Sidebar、ResizableSidebar、TitleBar等

#### 状态管理
- **Svelte stores**: 会话、设置、供应商/模型列表、MCP 选择
- **响应式状态**: 使用Svelte 5的 `$state` 和 `$derived` 语法

#### 前端服务封装
- **IPC客户端**: `@tauri-apps/api` 的 `invoke`/事件流封装
- **API层**: 统一的API调用封装，位于 `src/lib/api/`
- **本地缓存**: 轻量持久化/快照（不过度堆积隐私数据）

### 3.2 后端（Tauri/Rust）
- 命令层（Commands）：`#[tauri::command]` 暴露给前端的 IPC API。
- 服务层（Services）：业务逻辑（模型供应商检测、模型列表获取、消息发送/流式聚合、MCP 生命周期、搜索索引构建与查询、数据快照/导入导出）。
- 资源/系统层：
  - 存储：SQLite + FTS5（或首期 JSON+索引文件），位于应用数据目录（`app_data_dir`）。
  - 凭据：优先 OS Keychain（建议 Rust `keyring`），次选本地加密（密钥来源 OS 安全能力）。
  - 子进程/MCP：通过 `tauri-plugin-shell` 或等价能力进行受控启动、管道通信与超时清理。

### 3.3 前后端通信（IPC）
- 调用模式：
  - 请求-响应：前端 `invoke('command', payload)`，后端同步/异步返回。
  - 流式输出：后端在处理生成式对话时，以事件/分片流（Server-Sent Events 风格）推送，前端订阅并渲染；或使用 Tauri 事件总线进行分段消息 `emit`。
- 错误协议：统一错误结构（错误码、可读消息、建议操作），区分网络/鉴权/速率/用户输入/内部错误。

### 3.4 Agent 模式（独立子系统，自 milestone M1 起）

Agent 模式是接入 hand-ai `hand-agent` agentic-loop 的自主智能体工作台，与现有 Chat 模式**完全独立**（独立的数据结构、存储、输入框选项与会话视图），通过侧栏「单词本」下方的 `Chat | Agent` 段控开关切换。`appMode` 持久化于 localStorage，默认 `chat`。

- **存储**（与 Chat 的 `sessions`/`messages` 表完全隔离）：迁移 `044_create_agent_sessions.sql` + `045_create_agent_session_messages.sql`。每条 hand-agent `Message` 以完整 JSON 整存于 `agent_session_messages.payload`；transcript 按显式 `seq`（非 `created_at`）排序，重载即还原完整时间线（含 tool call / thinking / usage）。
- **后端运行时**（`services/agent_runtime.rs`）：复用 `chat_engine::{shared_client, resolve_model, build_stream_options}`（M1 提为 `pub(crate)`，零逻辑改动）装配裸 `model::Client` + `AgentLoopConfig`，驱动 low-level `run_agent_loop`。事件路径收敛到单一 choke point（`RunSink` + 单一 closed emit site）：每个 `AgentEvent` 发为 Tauri 事件 `agent_stream_event {sessionId, event}`，run-level `Err` 发为 sanitized 的 `agent_stream_error {sessionId, error{code,message,hint}}`（在 closed 之前），回合终结发 `agent_stream_closed {sessionId}` 恰好一次。约束：同一 session 同一时刻只允许一个 run（`runs` map 去重）。
- **错误分型**：① run-level `Err`（如 `ProviderNotFound`）→ sanitized envelope；② in-band `AssistantMessageEvent::Error` → 终结 assistant 消息 `stopReason=error`（走正常持久化路径，不发 envelope）。安全：envelope 永不回显可能携带 API key 的原始 provider 错误文本。
- **持久化时机**：user 消息发送后立即落库（先于 assistant）；assistant/tool 消息逐 `MessageEnd` 增量落库；只写完整可反序列化的 `Message` JSON；重载对损坏行优雅降级（跳过、不白屏）。
- **删除级联**：`agent_session_repository.delete_session` 在事务内显式先删 transcript 行再删 session 行（不依赖 `PRAGMA foreign_keys`，作为防御实现）。
- **命名隔离**：后端 `agent_session_*` / `agent_run_*` 命令、前端 `agentSession` / `agentRun` 状态、路由 `(app)/agent`（单数），与既有 `/agents` 预设（复数）零碰撞。
- **工具**（自 milestone M2 起，`services/agent_tools.rs`）：内置只读 FS（`read_file` / `list_directory`）+ 出站 `web_fetch`，由 `build_tools(enabled, working_dir)` 单点装配。
  - **沙箱**：自建 `working_dir` 解析器 `resolve_in_sandbox`——先 `canonicalize` root 与 target（解析符号链接），再按**路径分量**（非字符串前缀）+ 大小写折叠做 containment 校验，拒绝 `..`/绝对路径越界/前缀同级兄弟/`~` 展开/符号链接逃逸/NFD-NFC 变体/空·`.`·空白·NUL，错误信息无泄漏；`read_file` 经 `symlink_metadata` 在任何阻塞读取**之前**拒绝非常规文件（FIFO/设备），并对大文件/大目录按预算截断并标记。TOCTOU 为按 D11/D25 接受的残留风险（模块头注释记录）。
  - **SSRF 防护**（`web_fetch`，D13/D19）：纯函数化的 scheme 白名单 + host 归一化 + 字面 IP 解析（含十进制/十六进制/八进制·点分变体）+ IPv4/IPv6 分类（loopback/private/link-local 含 `169.254.169.254`/unique-local/IPv4-mapped/broadcast/unspecified）+ 解析后逐地址校验；自定义 redirect policy 对每一跳重新校验，叠加请求超时与响应字节上限。DNS-rebinding 为接受的 v1 残留（模块头记录）。
  - **门禁为结构性**（D8/D9）：未注册即不可运行——禁用工具、无 `working_dir` 的 FS 工具、以及永不注册的 mutating 名（`write_file`/`run_command`）都由 loop 的 `Tool <name> not found`（`isError=true`）兜底，`agent_runtime.rs` 内无独立 allowlist/denylist。`tool_execution_mode` 仅 `sequential`（去空白·大小写不敏感）→ Sequential，其余 → Parallel。
- **运行时 steering**（自 M2 起）：`RunHandle` 持有一个 `Arc<Mutex<Vec<Message>>>` steering 队列，与 `AgentLoopConfig.get_steering_messages` 闭包共享同一 `Arc`；`agent_run_steer` 命令向既有 run 入队 user 消息（保持 one-run-per-session），loop 在 turn 边界 drain；空/空白文本与无活跃 run 均为干净 no-op。
- **图片附件**（自 M2 起）：`build_user_message` 是 attachments → content blocks 的唯一 seam——`image/*` base64 编码为 `UserContentBlock::Image`（图片在前、文本在后，镜像 chat_engine），非图片防御性跳过；前端 `AgentInput` 仅图片选择器 + 缩略图 + 10MiB 软上限（超限跳过不阻塞）。
- **工具卡片 UI**（自 M2 起）：`agentRun.svelte.ts` 把 `tool_execution_start/update/end` 归并为按 `toolCallId` 键的 `ToolCallView`（executing→completed/error 原地翻转），`toolCallViewFor` 协调 live 优先、回退到已提交 `toolResult`（重载路径）；`AgentToolCallCard.svelte` 单 prop 渲染。重载时 `loadTranscript` 在无活跃 run 时清空 live `toolCalls`，纯从存储重建已终结卡片；run 终结/错误时 `settleDanglingToolCalls` 把仍 executing 的卡片落到 error 终态，杜绝中止后卡 spinner。
- **项目分组**（自 agent-projects 计划 M1 起）：Agent 模式 session 按工作目录归组为项目。迁移 `046_create_agent_projects.sql`（`agent_projects` 表，`path` UNIQUE）+ `047_backfill_agent_projects.sql`（纯 SQL 事务回填：每个 distinct 非空 canonical `working_dir` 恰好一行项目，name = basename、空 basename（如根 `/`）回退完整路径，项目时间戳 = 组内最新 `coalesce(last_message_at, created_at)`；既有 `agent_sessions` 列零触碰）。Schema 取「保留 `agent_sessions.working_dir` + 新增可空 `project_id` FK」而非 JOIN 取值：挂靠项目的 session 创建时由 `AgentSessionService` 把 `project.path`（已 canonical）复制进 `working_dir`（覆盖请求值），`agent_runtime.rs` 的 working_dir 消费点零改动；`project_id` **仅在创建时写入**（write-once，`update_session` 显式略过该列）。`AgentProjectRepository.create_project` 为 get-or-create：`INSERT ... ON CONFLICT(path) DO NOTHING` + re-select，按 canonical path 字符串全等去重（symlink 别名在 service 层 `canonicalize` 后命中同一行），并发同 path 创建不暴露约束错误、绝不改写已有行。项目删除 = `AgentProjectService` 先对项目下全部 session 逐个 `runtime.abort`，再由 repo 在单事务内显式三步级联（messages → sessions → project，不依赖 `ON DELETE CASCADE`），`project_id IS NULL` 的未分组 session 与兄弟项目天然不在删除范围。path 校验独立于 session 的 `validate_working_dir`（project path 必须非空 + 绝对 + 存在的目录，存 canonical 值）。IPC：`agent_project_create/list/rename/delete`；`agent_session_list` 默认 limit 自此为无上限（分组侧栏消费完整列表，不静默截断）。
- **分组侧栏与零弹窗创建**（自 agent-projects 计划 M2 起，纯前端）：`AgentProjectList.svelte` 取代平铺的 `AgentSessionList`（已删除，连同 `AgentSessionCreateModal`——创建全程零弹窗，per-session 目录选择不存在）。排序语义单一来源 `src/lib/utils/agentGrouping.ts` 纯函数 selectors：session 活动键 = `coalesce(lastMessageAt, createdAt)`（**绝不读 `updatedAt`**——rename/配置写入不得重排）；组间键 = `max(project.createdAt, 组内最新活动)` 降序、并列按 path 升序；未分组桶（`projectId` 缺失或悬挂）钉底。折叠态走 `states/agentProjectCollapse.svelte.ts`：localStorage key `agentProjectCollapse`、形态 `{ [projectId]: true }`（只存折叠项），未分组保留 key `__ungrouped__`，损坏值 fallback 全展开。直建 session：组头 hover「+」以 `CreateAgentSessionRequest.projectId` 创建，继承组内排序首位 session 的持久化配置全集（model+provider/thinking/工具/systemPrompt/temperature/maxTokens/toolExecutionMode），不继承内容性状态。运行态清理：`agentRunStore.removeSession(id)` = 删 per-session 状态 + 立 tombstone（非响应式 `deletedSessions` Set，三个流事件入口设防），拦截删除后迟到流事件重建条目；tombstone 由该 session 的 `agent_stream_closed` 回收；旧 `clear()` 已删除——删除路径一律用 `removeSession`。system prompt 唯一编辑入口 = `AgentSessionHeader` 设置 popover（`agent_session_update_field`，保存写回打开时刻 capture 的 sessionId）。失效恢复指针（`/agent?id=` 指向已删 session）由 `(app)/agent/+page.svelte` 的 `handleMissingSession` 清指针并 `replaceState` 回落地页。
- **Skills 后端基础**（自 agent-skill-support 计划 M1 起，纯后端逻辑库，无 UI/IPC/网络/DB）：skill = 含 `SKILL.md` 的目录，YAML frontmatter 声明 `name`/`description`/`disable-model-invocation`、body 为注入 system prompt 的指令。两个模块：`utils/frontmatter.rs` 的泛型 `parse_frontmatter::<T: DeserializeOwned>` 拆 `---\n<yaml>\n---\n<body>` 信封（处理 LF/CRLF、前导 BOM、EOF 锚定 closer，并把空块 `Some(Null)` 与无块 `None` 区分为两态）；`services/skills.rs` 的数据模型（`SourceScope`/`SourceInfo`/`SkillMetadata`/`Skill`/`SkillError`）+ 文件系统无关的 `validate()` + `discover_skills(roots)` 发现 + 纯函数 `format_skills_section`。关键不变量：① 长度上限按**字节**（`str::len`），判长用未 trim 原值、判空用 `trim().is_empty()`（与上游一致的刻意不对称）；校验序固定 description-required → description-length → name-mismatch → name-valid，只报首违规；② **dedup-before-validate**——`discover_skills` 用 `BTreeMap<name,_>` 在 load 成功后、validate 前占槽：loader 层坏（frontmatter/IO 错误）不占槽，低优先同名好 skill 顶上；validate 层坏（占槽后校验失败）遮蔽低优先（核心 shadow 语义，勿在后续编辑中重排）；发现非递归（仅直接子目录）、name 取父目录名、缺失 scope 静默跳过、单个坏 skill 不中断且不 panic；③ `SourceScope` serde 字面值 = `"project"`/`"user"`/`"appData"`，`Ord` 使 `Project` 最大供 dedup 取胜者（IPC/UI 契约）；`SkillError` 6 变体（`Io`/`Loader`/`MissingDescription`/`DescriptionTooLong`/`NameMismatch`/`InvalidName`），非 bool `disable-model-invocation` → `Loader(InvalidYaml)`；④ `format_skills_section` **有意分叉上游**：`<skill>` 块只含 `<name>`+`<description>`（无 `<location>`/绝对路径）、引导文案为 "call the skill tool"（不含 "read tool to load"），`escape_xml` 逐字符转义五个 XML 元字符、换行逐字保留、不二次转义；空列表返回 `None`。新增 `serde_yaml` 0.9 依赖（与 hand-ai 移植保持一致，上游已 archived，待后续可迁移）。

## 4. 数据与存储设计

存储位置：平台对应的应用数据目录下，例如：
- macOS: `~/Library/Application Support/com.gumpw.handbox/`
- Windows: `%APPDATA%/com.gumpw.handbox/`
- Linux: `~/.config/com.gumpw.handbox/`

推荐采用 SQLite（含 FTS5）统一管理结构化数据与全文索引，示意表：
- `sessions`：会话元数据（id、名称、创建/更新时间、关联模型/ArtifactId 等）
- `messages`：消息体（id、session_id、role、content、images、tokens、elapsed_ms、metadata json、ts）
- `artifacts`：Artifact（id、name、desc、prompt、model、params json、enabled_mcp[]、created_at、last_used_at）
- `providers`：供应商配置（id、name、type、base_url、status、updated_at）仅非敏感字段
- `settings`：通用设置（主题、语言、自动下滑等）
- FTS：对 `messages(content, prompt)` 建 FTS 虚表与触发器实现增量索引

凭据与机密：
- API Key 不入库，使用 OS Keychain（`service = com.gumpw.handbox`, `account = <provider-id>`）按供应商维度保存。
- 导入导出时，Key 不随数据导出；导入后由用户补充凭据。

导入/导出：
- 会话级 JSON：包含 `session + messages + config`（不含 Key）。
- 全量备份：SQLite 文件 + 配置 JSON 快照（不含 Key）。

MCP 配置 JSON：
```json
{
  "servers": [
    { "name": "local-tools", "command": "node", "args": ["mcp-server.js"], "enabled": true }
  ]
}
```
文件放置在应用数据目录，UI 提供内置 JSON 编辑器与校验（字段必填、数组非空、示例指引）。

## 5. 关键用例与流程

### 5.1 发送文字与图片（含流式）
1) 前端构建消息（文本、最多 10 张图片 meta）。
2) 通过 IPC `chat_send` 触发后端：
   - 参数：`session_id | artifact_id | inline_config`、`messages[]`、`attachments[]`、`stream=true`。
3) 后端：
   - 解析会话配置（模型、参数、系统提示词、MCP 绑定）。
   - 从 Keychain 取密钥，构造供应商 HTTP/SDK 客户端。
   - 流式聚合分片，实时 `emit` 渲染事件；完成后写入 `messages` 与度量（耗时、tokens）。
4) 前端订阅流，增量渲染；失败时提供重试与替换。

### 5.2 模型选择/探活/获取模型列表
- `provider_probe`：使用最小请求验证 Base URL 与 Key；失败返回明确错误。
- `provider_list_models`：拉取模型列表并缓存，支持搜索与禁用过滤。
- 切换模型后将元数据写入会话配置与消息元数据。

### 5.3 聊天配置与 Artifact
- 在聊天设置中编辑系统提示词、参数、启用的 MCP。
- `artifact_save`：将当前配置落盘/入库；`artifact_use`：基于 Artifact 新建会话。
- Artifact 列表支持预览、重命名、删除、二次确认。

### 5.4 历史消息搜索
- 前端输入关键词 → IPC `search_messages`。
- 后端使用 FTS 查询，按时间与相关性排序，返回片段预览与定位信息。

### 5.5 MCP 启停与执行过程展示
- 设置页切换 `enabled` → 写回 `mcp.json`。
- 聊天设置勾选启用的 MCP → 与会话绑定。
- 执行过程：将 MCP 调用步骤与日志以卡片/时间线形式增量展示。

## 6. 对外集成设计

### 6.1 模型供应商（OpenAI/Anthropic/Google/DeepSeek/OpenRouter/自定义）
- 类型 1（特定主流）：字段为 `api_key`, `base_url`（可选）。
- 类型 2（自定义，兼容 OpenAI/Anthropic API）：字段为 `name`, `type`, `base_url`, `api_key`。
- 统一 Provider 接口：`listModels()`、`chatCompletion(stream)`。
- 速率/错误处理：暴露错误码与建议（等待、降速、检查 Key、重试）。

### 6.2 MCP（Model Context Protocol）
- 存量通过 JSON 注册；原生启动子进程并管理生命周期（超时、日志、退出码）。
- 与 LLM 对话流程集成：按需调用 MCP 以获取/处理上下文数据；UI 展示执行过程。

## 7. 安全与隐私

- 本地优先：用户数据默认不出本地；所有第三方调用需用户显式配置。
- 代码执行/MCP：沙箱隔离、超时控制、内存上限；禁用危险系统调用。
- CSP：生产环境启用严格 CSP（当前 `tauri.conf.json` 为 `null`，建议上线前收紧）。
- 备份与恢复：仅手动导入/导出，本地存储加校验。

## 8. 性能与可靠性

- 启动：冷启动 < 3s（首次 < 5s），懒加载非关键模块与页面。
- 对话：发起到首包 < 2s（不含 LLM 响应），采用流式渲染与增量写入。
- 并发：≥ 10 会话并行稳定；后端异步化（Tokio）。
- 内存：空闲 < 500MB；长会话分页与虚拟列表；消息归档/快照。
- 重试与幂等：模型列表拉取、消息发送、索引构建具备重试与断点续建。
- 崩溃恢复：关键状态落盘；异常退出后重启可恢复。

## 9. 可扩展性与扩展点

- UI 卡片类型可扩展（思考过程、工具调用、代码执行）。
- Provider 可插拔（新增实现 `Provider` 接口并注册）。
- MCP 通过 JSON 扩展（无需改代码即可上/下线）。
- 数据 schema 版本化与迁移脚本（SQLite `PRAGMA user_version` + 迁移程序）。

## 10. 配置与环境

- 应用配置：`tauri.conf.json`（窗口、打包、CSP、构建前后命令）。
- 前端构建：`svelte.config.js`（static adapter，fallback `index.html`）。
- DevServer：`vite.config.js`（端口 1420/1421，忽略 `src-tauri` 监听）。
- 运行时变量：`TAURI_DEV_HOST`（HMR 场景）。

## 11. 目录结构与演进计划

当前：
- 前端：`src/routes/+page.svelte` 示例应用。
- 后端：`src-tauri/src/lib.rs` 提供示例命令 `greet`。

目标重构（逐步落地）：
```
src/
  lib/
    components/{ui,chat,prompt,agent}/
    stores/
    api/
    types/
    utils/
  routes/
    +layout.svelte
    chat/
    settings/
    artifacts/
    search/

src-tauri/src/
  commands/{chat.rs,provider.rs,artifact.rs,search.rs,settings.rs,mcp.rs}
  services/{llm_service.rs,provider_service.rs,mcp_service.rs,search_service.rs,storage_service.rs}
  models/{chat.rs,artifact.rs,provider.rs,settings.rs}
  utils/{crypto.rs,validation.rs,logger.rs}
  config/{app_config.rs}
  lib.rs
  main.rs
```

迁移分期：
- M1 基础框架：页面骨架、会话基本模型、Provider 探活、消息发送（非流式）。
- M2 流式与渲染：消息流式、度量、错误处理与重试。
- M3 存储与搜索：SQLite/FTS、导入导出、历史搜索页。
- M4 Artifact 与 MCP：Artifact 全流程、MCP 配置与执行过程可视化。
- M5 设置与本地化：主题、语言、快捷键、关于页与更新检查。

## 12. API 与类型（示例）

前端调用示例：
```ts
import { invoke } from '@tauri-apps/api/core';

type ChatParams = {
  sessionId?: string;
  artifactId?: string;
  inlineConfig?: {
    systemPrompt?: string;
    model: string;
    temperature?: number;
    topP?: number;
    maxTokens?: number;
    stream?: boolean;
    mcpServers?: string[];
  };
  messages: Array<{ role: 'user' | 'assistant' | 'system'; content: string }>;
  attachments?: Array<{ name: string; mime: string; path: string }>;
};

await invoke('chat_send', params satisfies ChatParams);
```

错误返回（统一结构）：
```json
{ "code": "RATE_LIMIT", "message": "请求过于频繁，请稍后重试", "hint": "降低并发或更换模型" }
```

## 13. 测试与度量

- 前端：Vitest/Testing Library，Playwright E2E（关键流程：新建会话、发送消息、切换模型、保存 Artifact、搜索跳转）。
- 后端：Rust 单元/集成测试，Mock Provider，离线样例回放。
- 性能日志：本地开关，默认关闭；不上传云端。

## 14. 风险与决策

- 供应商差异与速率限制：通过抽象 Provider 接口与重试/退避策略降低耦合。
- MCP 执行安全：默认禁用危险指令；仅显式启用的 MCP 可用；提供可视化与可中断能力。
- 凭据管理：优先 OS Keychain；若不可用则提示降级并给出风险提示。
- 数据迁移：版本化 schema，提供迁移脚本与回滚策略。

—— 本文档将随实现演进持续更新，确保与 PRD 与代码保持一致。


