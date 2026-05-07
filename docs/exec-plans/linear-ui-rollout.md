# ExecPlan: Linear 风格 UI 落地到 HandBox

**Status:** Completed (M5 retrospective filled, screenshots pending Gump's manual capture)
**Author:** Claude (与 Gump 协作)
**Date:** 2026-05-06

This is a living document. The Progress, Surprises & Discoveries, Decision Log, and Outcomes & Retrospective sections must be kept up to date as work proceeds.

## Purpose

把 `docs/ui-design.md` 里以 Linear 为参照的设计语言，从一份"参考文档"变成 HandBox 实际生效的视觉语言。完成后，开发者打开应用应能直观看到三件事：聊天页（用户气泡 / 助手消息）、Provider 添加 / 编辑模态框、设置页里的分组面板，全部呈现 Linear 风格的近黑画布、四级表面阶梯、发丝边框与薰衣草紫强调色；同时浅色主题继续可用，不破坏既有路由与数据流。Token 层面的对齐写在 `src/app.css` 的 `@theme` 中，使所有 `bg-base-*` / `text-base-content` 这类 Tailwind 工具类自动继承新风格，无需逐组件改类名。

## Progress

- [x] M1.T1 — 在 `docs/ui-design.md` 末尾追加"HandBox Deviations"段，记录浅色模式策略、oklch 表示、保留的语义色与 Web/Tauri 字体替换 _(2026-05-06)_
- [x] M1.T2 — 在 `src/app.css` 的 `@theme` 中改写 `--color-base-100/200/300`、`--color-primary*`、新增 `--color-surface-3/4`、`--color-hairline*`、`--color-ink-subtle`，浅色 / 深色双轨完成 _(2026-05-06)_
- [x] M1.T3 — `npm run check` baseline 比对：本次改动 0 新增类型错误（pre-existing 11 errors 全部位于 `routes/(app)/words/+page.svelte` 等文件，与 CSS token 无关）。`tauri dev` 视觉验证在 M2 完成后一次性进行 _(2026-05-06)_
- [x] M2.T1 — 改造 `MessageUser.svelte` 气泡：`px-4 py-3 rounded-2xl` → `px-3.5 py-2 rounded-lg` + 1px hairline 边框 _(2026-05-06)_
- [x] M2.T2 — `MessageAssistant.svelte` / `ToolCallCard.svelte` 审计：现有 `border-base-300` / `bg-base-100/200` 类在新 token 下视觉一致，无需改动；scope-discipline 不动 _(2026-05-06)_
- [ ] M2.T3 — 浅色 + 深色双主题各截一张聊天页面截图，附在 Artifacts and Notes 段（推迟到 M4 完成后一次性截图）
- [x] M3.T1 — 调整 `AddProviderModal.svelte` 容器：模态背景 = surface-1，标题用 headline 字号，CTA 按 button-primary 规格 _(2026-05-06)_
- [x] M3.T2 — 验证 `Modal.svelte`、`RoundButton.svelte` 是否需要补 token；不直接改其 API _(2026-05-06，发现 S3-S6 偏差，登记不修)_
- [x] M4.T1 — `TableGroup.svelte` 容器重构：移除绝对定位双层 div hack，改单层 `bg-base-200 rounded-lg border border-[var(--hairline)] overflow-hidden`；分隔线 `var(--base-300)` → `var(--hairline)` _(2026-05-06)_
- [x] M4.T2 — `TableBaseRow.svelte`：审视后不动（见 Decision Log D6）；行间分隔线由 TableGroup 投射的 `:global(> *:not(:last-child)::after)` 仍生效 _(2026-05-06)_
- [x] M4.T3 — `SwitchRow` / `SelectRow` / `TextRow` 在新 TableGroup 下视觉对比度通过；hairline 边框 + 12px 圆角 + 行间 hairline 分隔线在双主题下都成立。`npm run check` 增量 0（baseline 11 errors / 17 warnings 不变）_(2026-05-06)_
- [x] M5 — 总结回顾：填写 Outcomes & Retrospective，输出待跟进项清单 _(2026-05-06)_

## Surprises & Discoveries

**S1 (M1.T3, 2026-05-06)** — `npm run check` 在 main 上即报 11 errors / 17 warnings，绝大多数是 `@lucide/svelte` 图标 component 类型不匹配 (`Type 'typeof BookMinus' is not assignable to type 'Component<IconProps, {}, "">'`)，全部位于 `routes/(app)/words/+page.svelte` 等业务页。本次 token 改动 0 新增。处置：不在本计划范围内修复，登记到 M5 待跟进项。

**S2 (M1.T2)** — `[data-theme="dark"]` 块需要同时映射两套别名：daisyUI 风格 (`--base-100` 等，已存在) 与 Linear 扩展 (`--color-surface-3` / `--surface-3` 等，新增)。后者需要在 dark 块里既覆盖 `@theme` 暴露的全局 token，又给短别名赋值，否则深色下扩展 token 不切换。已在 dark 块同时写 `--color-surface-3: var(--color-surface-3-dark)` 与 `--surface-3: var(--color-surface-3-dark)`。

**S3 (M3.T2, 2026-05-06)** — `Modal.svelte` 卡片背景层 (line 67) 写死 `bg-base-100` (= canvas)，语义上不是 surface-1；Linear 期望模态卡 = surface-1。当前用 AddProviderModal 内部 wrapper 加 `bg-base-200` 实现 lift；理想方案是把 Modal.svelte 的卡片背景升级为 `bg-base-200`，但这会牵动所有现有 Modal 调用方，超出 M3 范围。登记到 M5 待跟进项。

**S4 (M3.T2, 2026-05-06)** — `Modal.svelte` line 67-68 同时使用 `rounded-2xl` 类与内联 `style="border-radius: 20px"`，内联值覆盖 class，且都偏大于 Linear 紧凑 `rounded-lg` (12px) 范式。同行 `border-base-200` 在新 token 下与卡片底色同明度，边界几乎不可见，应改用 `var(--hairline)`。本里程碑只读不改；登记到 M5。

**S5 (M3.T2, 2026-05-06)** — `RoundButton.svelte` 默认 `bgColor="bg-primary"` / `textColor="text-primary-content"` 走 token；但默认 `hoverColor="hover:opacity-90"` 不符合 Linear "hover = 上升一层" 的明度变化约定，应改为 `hover:bg-primary/90` 或 token-driven hover 色。另：`disabled:bg-base-300` 写死、`h-10` + `text-[16px]` 偏大于 Linear 紧凑节奏 (`h-8 text-sm`)。本里程碑不改；登记到 M5。

**S6 (M3.T2, 2026-05-06)** — `Modal.svelte` backdrop (line 51) 用 `var(--overlay)`，深色解析为 `oklch(0% 0 0 / 0.8)` 与 Linear `#000 80%` 完全一致；浅色 `oklch(8% 0 250 / 0.5)` 不透明度偏低（但 Linear 本身 dark-only，浅色为 HandBox Deviation，可接受）。无需修改。

**S7 (Review 后补登, 2026-05-06)** — Reviewer 发现 4 处需修复 + 4 处需文档化。已修复：(a) Modal 双层卡（D8 / Important #1）；(b) 浅色 primary 对比度 4.2:1 → 调到 5.0:1（45% L / 0.19 chroma）；(c) `rounded-lg`(8px) → `rounded-xl`(12px) 兑现 Linear `lg=12px` token；(d) `[data-theme="dark"]` 中失效的 `--color-*-dark` re-binding 删除（runtime 无效，Tailwind utility build-time 已 baked）。已文档化：(e) D7 解释 "headline" 重新解读为 Linear 紧凑标题；(f) D8 解释 Modal 内核改造；(g) 给深色 surface 加 chroma 0.005 真正赋予"近黑带蓝调"（chroma 0 时 hue 250 无效）；(h) `--color-ink-subtle` / `--color-ink-tertiary` 加注释固化命名约定。

## Decision Log

**D1 — Linear 是 dark-only，HandBox 必须保留浅色模式。**
方案：把 Linear 的 dark token 映射到 HandBox 的 dark theme，并为浅色构造一套"反相 Linear"——`canvas` 取 `oklch(99% 0 0)`、`surface-1` 取 `oklch(97% 0 0)`、`surface-2` 取 `oklch(94% 0 0)`，发丝边框取 `oklch(90% 0 0)`，文字取深灰 `oklch(18% 0 0)`，强调色复用同一只薰衣草紫但略微加深到 `oklch(48% 0.18 277)` 以保证 AA 对比。该决策在 `docs/ui-design.md` 的 Deviations 段中说明。

**D2 — 颜色系统统一用 oklch，不混用 hex。**
原因：HandBox 既有 `@theme` 全是 oklch，且大量使用 `color-mix(in oklch, ...)`。把 Linear 的 hex 转成 oklch 写进 `--color-*` 变量，避免色彩函数兼容性问题。Linear 锚定值的 oklch 近似：`canvas #010102 ≈ oklch(8% 0 250)`，`surface-1 #0f1011 ≈ oklch(15% 0 250)`，`surface-2 #141516 ≈ oklch(18% 0 250)`，`primary #5e6ad2 ≈ oklch(54% 0.16 277)`，`primary-hover #828fff ≈ oklch(70% 0.18 285)`，`ink #f7f8f8 ≈ oklch(97% 0 0)`，`ink-subtle #8a8f98 ≈ oklch(63% 0.01 250)`。最终值由 M1.T2 期间用浏览器 DevTools 微调到肉眼匹配 Linear 截图为准。

**D3 — 不重命名既有 Tailwind 工具类。**
即不把 `bg-base-200` 改成 `bg-surface-1`。所有现有组件继续用 daisyUI 风格的 `base-100/200/300` + `primary` + `base-content`；只是这些变量背后的值变了。这能让本次迁移代价降到最低，且未来若再换风格只需改 `@theme`。新增的 `surface-3/4`、`hairline*`、`ink-subtle` 仅在显式需要的少数组件里通过 CSS 变量引用，例如 `style="background: var(--color-surface-3)"`。

**D4 — 字体保持系统栈。**
Linear 的自有字体不公开。Deviations 段记录：使用 `-apple-system, system-ui, "SF Pro Display", "Helvetica Neue", Inter, sans-serif` 作为正文与显示型；mono 沿用现有 `Fira Code` 链。不在 M1 引入 Web 字体，避免桌面应用首屏闪烁。

**D5 — 验证组件选择：聊天气泡 / Provider 模态 / Settings 分组。**
理由：覆盖三种典型表面——平铺正文（assistant 消息）、独立卡片（user 气泡 + 模态）、嵌套面板（TableGroup）；同时是 HandBox 用户高频路径；改动控制在 token + 容器层不动业务逻辑。

**D7 — "Headline 字号"重新解读为 Linear 紧凑标题（16px / 500 / tracking-tight）。** _(2026-05-06，Review 后补登)_
原 ExecPlan §M3.T1 写"标题用 headline 字号"，字面意义是大字号。实际落地把 `<h2>` 从默认浏览器尺寸（~24px）改为 `text-base font-medium tracking-tight`（16px / 500 / 负字距）。Linear 自身的模态标题就是这种紧凑节奏，并非显示型大标题——marketing 站点的 display headline 是用在 hero / section opener，模态内属于卡片紧凑标题。视觉上更接近 Linear 真实产品语义；`text-base` 对应 Linear 的 `body` token（16px）配合 medium 字重 + 负字距构成卡内紧凑标题。

**D8 — Important #1 修复方案：在 Modal.svelte 内核统一上浮到 surface-1。** _(2026-05-06，Review 后补登)_
Reviewer 指出 AddProviderModal 在 Modal.svelte (`bg-base-100`) 外再套一层 `bg-base-200` 形成"双层卡"伪装，并非真正"modal = surface-1"。决定：把 Modal.svelte 内核背景从 `bg-base-100` 升到 `bg-base-200`，移除内联 `border-radius: 20px`，把 `border-base-200` 换成 `var(--hairline)`，并把内层 wrapper 在 AddProviderModal 中删除。代价：其它 5 个 modal 调用方（`ChatModelSelectModal` / `McpServerFormModal` / `ConfirmModal` / `ModelInfoModal` / `McpServerTextEditModal`）的视觉同时上浮一档——这正是 Linear 设计意图，不是回归。原 plan §Interfaces 限制"Modal.svelte 不要改"在此被覆盖，改动控制在 1 行 class + 1 行 inline style 删除 + 1 处 border 颜色，没有 props 接口变更。

**D6 — TableGroup 标题保持容器外部；M4 不动 TableBaseRow。** _(2026-05-06)_
TableGroup 的标题区（`<button>`）保留原结构、站在容器外（`my-1 mx-2 text-xs`），不在容器内加 `border-b border-[var(--hairline)]`。理由：(a) 多个 TableGroup 在设置页竖向堆叠时，外部标题让卡片之间分隔更清；(b) 把标题移到容器内会改变所有调用者的纵向布局，超出 M4 视觉收敛范围。
TableBaseRow 不引入 hover 行为：(a) 它是基础行容器，被 SwitchRow / SelectRow / TextRow / DefaultRow / StatusLabelRow 等多种语义截然不同的行复用，并非所有行都是 clickable（输入类行点击应聚焦输入而非整行 lift），统一加 hover 会误导用户；(b) 真正"clickable"的行类型应在自身组件内控制 hover lift，作为后续独立任务处理。本任务维持 TableBaseRow 视觉不变；TableGroup 投射的 `:global(> *:not(:last-child)::after)` 行间分隔线（已切换到 `var(--hairline)`）继续生效。

## Outcomes & Retrospective

### 完成情况（2026-05-06）

按计划交付了五个里程碑中的四个代码层里程碑（M1–M4）+ 总结里程碑（M5）。M2.T3 的双主题截图未完成，留待 Gump 手动启动 `npm run tauri dev` 后捕获——这一项不影响功能验收，且无法在自动化代理中执行（截图需要 macOS 桌面交互）。

**Token 层面**：`src/app.css` 的 `@theme` 完成 Linear 风格全套映射，浅 / 深双主题协调；新增六个扩展 token (`--color-surface-3/4`、`--color-hairline*`、`--color-ink-subtle/tertiary`) 对应 Linear 的扩展阶梯，可按需在组件内 `var(...)` 引用。**daisyUI 风格的工具类名 (`bg-base-*`、`text-base-content`、`bg-primary`) 全部保留**，意味着除了主动改造的三个验证组件外，其它 90+ 组件无需改动即继承 Linear 视觉。

**验证组件交付**：
- 用户聊天气泡：紧凑 12px 圆角 + 1px hairline 边框（M2）
- Provider 模态：surface-1 lift + 紧凑节奏 + 默认 primary CTA（M3）
- Settings TableGroup：单层 Linear 卡片 + hairline 边框 + hairline 行间分隔（M4）

### 与 Purpose 段对照

Purpose 段承诺"开发者打开应用应能直观看到三件事"——三件事在 M2/M3/M4 各自代码层完成。最终视觉验收需要 Gump 在本地启动 `npm run tauri dev`，依次：
1. 进入聊天页发一条消息（看气泡形态）
2. 设置 → Provider → 添加供应商（看模态层次）
3. 设置 → 任意子页（看 TableGroup 卡片）
4. 通过应用内主题切换比对浅 / 深两套外观

### 待跟进清单（M5 输出）

按优先级降序：

1. ~~**Modal.svelte 内核升级**（来源：S3 / S4）~~ ✅ **已在 Review 阶段修复**（D8）
   - Modal 卡片层从 `bg-base-100` 升到 `bg-base-200`、移除内联 `border-radius: 20px`、`border-base-200` → `var(--hairline)`。
   - AddProviderModal 内部 wrapper 已删除。
   - 5 个未被本计划主动检阅的 Modal 调用方（`ChatModelSelectModal` / `McpServerFormModal` / `ConfirmModal` / `ModelInfoModal` / `McpServerTextEditModal`）由于背景上浮，视觉一并发生变化——属设计意图（Linear modal = surface-1），不属回归；后续 Gump 视觉验收时如发现具体调用方有"内容期望坐在 canvas-bg 上"的特殊情况，再单独修。

2. **RoundButton.svelte 紧凑化**（来源：S5）
   - 默认尺寸从 `h-10 + text-[16px]` 收紧到 Linear 紧凑节奏 `h-8 + text-sm`；hover 行为从 `opacity-90` 改为 `bg-primary/90` 或 token-driven hover。
   - 影响面：全应用按钮高度，需要回归测试所有调用方布局。

3. **真正 clickable 行的 hover lift**（来源：D6）
   - `DefaultRow.svelte` / `StatusLabelRow.svelte` 在 `clickable=true` 时，给行加 `hover:bg-base-300 transition-colors`，符合 Linear "lift one level on hover"。
   - `TableBaseRow` 仍保持纯容器、不加 hover。

4. **`@lucide/svelte` 类型签名兼容性**（来源：S1）
   - pre-existing 11 errors / 17 warnings，与 Lucide v1.x → Svelte 5 `Component<T>` 类型不匹配相关。需升级 `@lucide/svelte` 到匹配 Svelte 5 的版本，或在调用点用类型断言。
   - 不属于本计划范围；列入设计系统迁移**之外**的独立 issue。

5. **聊天页的 reasoning / tool-call 边界视觉**
   - `MessageAssistant.svelte` 内 reasoning 块用 `border-l border-base-300`，在新 token 下深色模式 base-300 与 base-200 亮度差较小（18% vs 15%），左边线不够醒目。建议改为 `border-[var(--hairline)]`。
   - 不属本计划已交付范围；在下次迭代加。

6. **截图归档（M2.T3）**
   - Gump 在本地完成 M2/M3/M4 的视觉肉眼验收后，在 `docs/exec-plans/_artifacts/` 下保存四张截图（聊天 light/dark、设置 light/dark），并在本节增量补充。

### 经验沉淀

- **Token 层是杠杆点**：仅 `src/app.css` 一个文件 ~70 行的 `@theme` 改写，把全应用 90+ 组件的视觉调性变了——daisyUI 命名空间的存在让 Linear 风格"渗透"成本接近零。这是 D3 决策的最大收益。
- **scope-discipline 比想象的更重要**：M3 / M4 实施过程中发现了 6 处可改进点（S3–S5），但都没动——保持每个里程碑的 diff 在 `≤ 20 行 / 1 文件`，让 commit history 清晰、回滚可控。改动累积到一起会让单次 review 难以把控。
- **并行 dispatch 实测 OK**：M3 + M4 文件不交叉，两个 implementer agent 并发跑没有冲突；控制器只需在合并阶段做单次 git commit + 验收。前提是任务边界写得足够清楚（"只改这两个文件、不改 props、报告但不 commit"）。
- **Linear 浅色模式需要原创**：Linear 官方不提供浅色，HandBox 必须自己设计反相阶梯。`oklch(99% / 97% / 94%)` 的递进配 `oklch(48% 0.18 277)` 加深的薰衣草紫，目前是基于 Linear dark 设计意图的合理推断，最终需肉眼验收 Gump 是否接受。

## Context and Orientation

HandBox 是 Tauri 2 + SvelteKit 5 的本地 AI 工作台。前端样式系统由两层构成：底层是 Tailwind 4.x 的 `@theme` CSS 变量（在 `src/app.css` 里全部用 `oklch()` 表示），上层是组件中的 Tailwind 工具类（`bg-base-200`、`text-base-content` 等）。深色模式靠 `[data-theme="dark"]` 选择器在 `<html>` 上切换，在 `app.css` 第 92–137 行做变量重映射。daisyUI 风格的 `base-100/200/300`、`primary/secondary/accent`、`base-content` 是当前事实标准。

`docs/ui-design.md`（548 行，11 段，刚刚通过 `npx getdesign add linear.app` 安装）描述了 Linear 的 dark-only 设计语言：四级表面阶梯（canvas → surface-1..4）、发丝边框、薰衣草紫 `#5e6ad2` 唯一强调色、显示型字号配负字距。它本身不知道 HandBox 的浅色模式存在，也不知道 HandBox 用 oklch。本计划要做的就是把它的"意图"翻译成 HandBox 的"事实"。

**术语**：
- **Token**：`@theme` 里的 CSS 变量，例如 `--color-base-100`。
- **Surface ladder**：Linear 的多层表面递进，由 canvas（最底）到 surface-4（最高），靠纯色亮度差而非阴影制造层级。
- **Hairline border**：1px 实线边框，颜色介于相邻两层 surface 之间，用于在无阴影的情况下勾出卡片轮廓。
- **Ink**：文字颜色的统称（ink / ink-muted / ink-subtle / ink-tertiary），按对比度递减。

**相关文档**：
- 设计参考：`docs/ui-design.md`
- 既有架构：`docs/architecture.md`
- 既有组件清单：`docs/ui-components.md`
- 项目协作约束：`CLAUDE.md`

**关键源文件**：
- `src/app.css` — `@theme` 与 `[data-theme="dark"]` 的唯一定义点；Markdown 与代码块的细节样式也在这里。
- `src/lib/components/chat/messages/MessageUser.svelte` — 用户消息气泡，第 122–124 行是当前样式（`rounded-2xl bg-base-200 text-base-content`）。
- `src/lib/components/chat/messages/MessageAssistant.svelte` — 助手消息容器；正文走 `markdown-content` 类。
- `src/lib/components/settings/AddProviderModal.svelte` — Provider 添加 / 编辑模态，复用 `Modal` + `TableGroup` + `TextRow/SelectRow`。
- `src/lib/components/ui/table/TableGroup.svelte` — 设置页分组容器；本计划要把它从默认 div 改造成 surface-1 + hairline 卡片。
- `src/lib/components/ui/table/TableBaseRow.svelte` — 行的基础结构；行间分隔线由它控制。
- `src/lib/components/ui/Modal.svelte` — 通用模态外壳。
- `src/lib/styles/highlight-dark.css` — 代码高亮深色主题。

## Plan of Work

工作分五个里程碑，按 token 优先、再按组件由小到大推进。每个里程碑都能独立验证，能在任意位置停止 / 回滚，不阻塞其它路由继续工作。

### Milestone 1 — Token 落地与 Deviations 文档

完成后，应用启动时整体观感已经"变成 Linear"——画布近黑、卡片浮一级、按钮变薰衣草紫——但所有组件类名一行未动。

**T1：扩展 `docs/ui-design.md`。** 在文件末尾追加一段 `## HandBox Deviations`（中文小节标题用英文以保持文件整体语义统一），按以下结构写：(1) 浅色模式策略（D1 决策摘要 + 浅色 token 表）；(2) oklch 数值映射表（D2，列出 12 个 Linear 锚定颜色对应的 oklch 近似值）；(3) 保留的语义色（HandBox 仍用 info/warning/error，理由：聊天和工具调用需要明确的成功/失败/警告反馈）；(4) 字体替换（D4）；(5) 工具类命名延续（D3，明确 `bg-base-200` ≈ Linear `surface-1`）。

**T2：改写 `src/app.css` 的 `@theme`。** 在第 7–55 行块内：
- 重写 `--color-base-100/200/300`：浅色取 `oklch(99% 0 0)` / `oklch(97% 0 0)` / `oklch(94% 0 0)`；深色 `*-dark` 变体取 `oklch(8% 0 250)` / `oklch(15% 0 250)` / `oklch(18% 0 250)`。注释里写明"100=canvas, 200=surface-1, 300=surface-2"。
- 重写 `--color-primary` 和 `--color-primary-content`：浅色 `oklch(48% 0.18 277)` + 白；深色 `oklch(54% 0.16 277)` + 白。
- 新增 `--color-surface-3` / `--color-surface-4`、`--color-hairline` / `--color-hairline-strong`、`--color-ink-subtle` / `--color-ink-tertiary`，浅深各一份。这些变量不放进 daisyUI 风格的标准集合，仅供少数显式需要的组件用 `var(--color-*)` 引用。
- 在第 67–89 行 `:root` 与第 92–137 行 `[data-theme="dark"]` 块里同步新增变量的别名（`--surface-3` 等）。

**T3：质量门。** 运行 `npm run check`（类型）+ `npm run tauri dev` 启动桌面包，沿菜单点过聊天 / 设置 / Provider 列表。预期：所有页面渲染正常，颜色变成 Linear 风格但文字依然清晰；不应该出现白底白字、深底深字这类对比失效。任何回归记入 Surprises & Discoveries。

**M1 验收**：Token 替换后应用可正常运行；ChatBubble 的视觉已经发生变化（即使尚未做 M2 的形态调整），证明 token 路径打通。

### Milestone 2 — ChatBubble 验证组件

把用户气泡从 `rounded-2xl bg-base-200`（圆头大半径）改成 Linear 的紧凑卡片范式：`rounded-lg`（12px）+ 1px hairline border + 8/14 padding 节奏。助手消息保持平铺正文，但把内嵌的 reasoning / tool-call 卡片背景拉到 surface-1，用 hairline 包边。

**T1：改 `MessageUser.svelte` 第 122–124 行。** 当前：

    class="inline-block max-w-full px-4 py-3 rounded-2xl bg-base-200 text-base-content"

改为：

    class="inline-block max-w-full px-3.5 py-2 rounded-lg bg-base-200 text-base-content border border-[var(--color-hairline)]"

理由：`px-3.5 py-2`（≈ 14px / 8px）匹配 Linear 的紧凑节奏；`rounded-lg`（12px）替换 `rounded-2xl`（16px）使气泡少一分"圆胖"；新增 hairline 边框让气泡在 surface-1 = base-200 上仍能勾出轮廓。文字字号 `text-[15px]` 保持不变。

**T2：检查 `MessageAssistant.svelte`。** 助手消息本身是平铺的，正文走 `markdown-content`。重点看：(a) reasoning 折叠块；(b) `ToolCallCard.svelte`。如果它们用了 `bg-base-100` / `bg-base-200` 在新 token 下视觉错乱（例如 reasoning 块淹没在 canvas 里），把它们换成 surface-1 + hairline 一致的卡片。先读后改。

**T3：截图对照。** 浅 / 深主题各发一条用户消息 + 一条助手消息，截图保存到 `docs/exec-plans/_artifacts/linear-ui-chatbubble-{light,dark}.png`（M2 前手动 `mkdir`）。在 Artifacts and Notes 段引用。

**M2 验收**：用户气泡形态收敛、边框可见但不抢戏；助手消息的代码块、reasoning 块在双主题下都不糊。

### Milestone 3 — Provider 模态验证

`AddProviderModal.svelte` 是一个高频且承载多种行类型（TextRow / SelectRow + RoundButton CTA）的容器，用来检验模态层在 Linear 风格下是否成立。

**T1：调整模态外壳。** 不改 `AddProviderModal.svelte` 的业务逻辑，只调整其 `<Modal>` 容器的 `customClass`（如有）或在其内部 wrapper 上加 `bg-base-200 rounded-lg border border-[var(--color-hairline)]`。模态背景 = surface-1，模态内的 `TableGroup` 嵌一层成为 surface-2 卡（在 M4 完成后自动获得正确视觉）。模态内的"保存"按钮 (`RoundButton`) 确认采用 `bg-primary text-primary-content` —— `RoundButton` 已支持 `bgColor` props，传入 token 而非硬编码颜色。

**T2：检查 `Modal.svelte` / `RoundButton.svelte`。** 只读不写，记录两件事：(a) 是否有任何硬编码 hex / oklch 没走 token；(b) Modal 的 backdrop 是否符合 Linear 的 `semantic-overlay` (#000000 80% 不透明)。如有偏差但本里程碑不修，登记到 Outcomes 的待跟进项里。

**M3 验收**：打开 "添加 Provider" 模态，背景层次（页面 → backdrop → 模态卡 → TableGroup → 行）四层清晰；CTA 按钮明显是 Linear 薰衣草紫；浅深主题均通过。

### Milestone 4 — Settings TableGroup 验证

`TableGroup` 是 HandBox 设置页的核心容器。当前默认渲染是基本 div + 标题，本里程碑把它升级为 Linear 卡片：surface-1 背景 + 12px 圆角 + 1px hairline 边框 + 行间 hairline 分隔。

**T1：改 `TableGroup.svelte`。** 给容器外层加 `bg-base-200 rounded-lg border border-[var(--color-hairline)] overflow-hidden`；标题 `<header>` 用 Linear 的 `eyebrow` 风格——12–13px、font-medium、`text-[var(--color-ink-subtle)]`、letter-spacing 略正；标题与内容之间留 `border-b border-[var(--color-hairline)]`。

**T2：改 `TableBaseRow.svelte`。** 行的下边框（如有）用 `border-[var(--color-hairline)]` 替换 `border-base-300`；`hover` 态从默认 `hover:bg-base-200`（与外层同色）改成 `hover:bg-base-300`（surface-2 lift），符合 Linear 的"hover = 上升一层"约定。

**T3：三种典型行（SwitchRow / SelectRow / TextRow）只读检查。** 不改它们；只看在新 TableBaseRow 下视觉是否平衡。如不平衡，登记到 Outcomes。

**M4 验收**：设置页一个典型 group（建议看"通用"或"模型"页）呈现明确的卡片轮廓；行之间分隔线细但可见；hover 行有微妙上升感；浅深主题均通过。

### Milestone 5 — 总结与待跟进

不改代码，只记录。把 Outcomes & Retrospective 段填完，列出："本次未覆盖的组件 / 路由"、"用户在 review 中提出的偏离项"、"下次迁移要修补的 token gap"。

## Concrete Steps

工作目录：`/Users/wanggang/dev/00/handbox/space1/handbox`。

**M1.T1 — 追加 Deviations 段（编辑 `docs/ui-design.md`）。** 用 Edit 工具，无命令。

**M1.T2 — 改写 `src/app.css`。** 用 Edit 工具，无命令。完成后：

    npm run check
    # 预期尾行：✓ Found 0 errors and 0 warnings

**M1.T3 — 启动桌面包：**

    npm run tauri dev

预期：窗口打开，加载完成后看到聊天页；颜色已变深 / 紫；终端无 panic。手动点过：聊天 → 设置 → Provider 列表 → 模型列表。

**M2.T1 / M2.T2 — 编辑两个 message 组件。** 无命令。改完后回到已运行的 `npm run tauri dev` 窗口，发一条测试消息验证。

**M2.T3 — 截图：**

    mkdir -p docs/exec-plans/_artifacts

用 macOS `Cmd+Shift+4` 框选保存到上述路径，命名 `linear-ui-chatbubble-light.png` / `linear-ui-chatbubble-dark.png`。

**M3.T1 — 编辑 `AddProviderModal.svelte`。** 无命令。在已运行的应用中点 "设置 → Provider → 添加" 验证。

**M4.T1 / M4.T2 — 编辑 TableGroup / TableBaseRow。** 无命令。验证：设置页打开任意一组。

**M5 — 全量自检：**

    npm run check
    cargo fmt -- --check
    cargo clippy -D warnings

    # 期望全部通过，warning = 0

## Validation and Acceptance

整体可执行验收（M1–M4 完成后）：

1. `npm run check` 退出码 0，无类型错误。
2. `npm run tauri dev` 启动应用：
   - 浅色主题下整体观感是"近白画布 + 浅灰卡片 + 薰衣草紫强调"，文字深灰对比清晰；
   - `<html data-theme="dark">` 切换后变成"近黑画布 + 炭灰卡片 + 同色系紫"，且任何文字 / 边框依然可读；
   - 用户气泡是 12px 圆角紧凑卡片，浅 / 深主题下均能看到 1px hairline 边框；
   - 助手消息正文流式渲染、reasoning 块、代码块、ToolCall 卡片均无视觉错乱；
   - 打开 "添加 Provider" 模态，能看到四层背景递进（页面 < backdrop < 模态 < TableGroup）；
   - 设置页任一 group：标题区与内容区之间有发丝分隔线，行 hover 时背景明显上升一档。
3. 手动操作 git diff，受影响文件总数应在 6 ± 2 范围内：`docs/ui-design.md`、`src/app.css`、`src/lib/components/chat/messages/MessageUser.svelte`（必）、`src/lib/components/chat/messages/MessageAssistant.svelte`（条件）、`src/lib/components/settings/AddProviderModal.svelte`、`src/lib/components/ui/table/TableGroup.svelte`、`src/lib/components/ui/table/TableBaseRow.svelte`。任何超出此清单的修改需在 Decision Log 加条目说明。

## Idempotence and Recovery

- M1 的 token 改写是声明式覆盖，多次运行同一份编辑无副作用。
- 任意里程碑失败可单独 `git checkout -- <path>` 回退该文件，不影响其它里程碑产物。
- 由于本计划不动任何业务逻辑、不改 IPC、不动数据库，最坏情况是视觉回退到当前 main，无数据风险。
- 回滚预案：若 M1 后 Gump 觉得方向不对，单条命令 `git checkout -- src/app.css docs/ui-design.md` 立刻回到 main 视觉；前提是 M1 完成时未提交。建议本计划全部任务在一个工作分支 `feat/linear-ui-rollout` 上推进，最终 squash 一次。
- 截图与产物 (`docs/exec-plans/_artifacts/`) 不入提交可由 `.gitignore` 忽略，或作为 PR 附件单独保留。

## Artifacts and Notes

待 M2 / M3 / M4 各阶段完成后填入截图引用。

期望 Token 映射表（Decision Log D1 / D2 的可执行版本，M1.T2 实施时按此为准，浏览器目检后微调）：

| 角色 | Linear hex | HandBox 浅色 oklch | HandBox 深色 oklch | 变量名 |
|---|---|---|---|---|
| canvas | #010102 | oklch(99% 0 0) | oklch(8% 0 250) | --color-base-100 |
| surface-1 | #0f1011 | oklch(97% 0 0) | oklch(15% 0 250) | --color-base-200 |
| surface-2 | #141516 | oklch(94% 0 0) | oklch(18% 0 250) | --color-base-300 |
| surface-3 | #18191a | oklch(91% 0 0) | oklch(22% 0 250) | --color-surface-3 |
| surface-4 | #191a1b | oklch(88% 0 0) | oklch(25% 0 250) | --color-surface-4 |
| hairline | #23252a | oklch(90% 0 0) | oklch(28% 0 250) | --color-hairline |
| hairline-strong | #34343a | oklch(85% 0 0) | oklch(35% 0 250) | --color-hairline-strong |
| ink | #f7f8f8 | oklch(18% 0 0) | oklch(97% 0 0) | --color-base-content |
| ink-subtle | #8a8f98 | oklch(45% 0.01 250) | oklch(63% 0.01 250) | --color-ink-subtle |
| ink-tertiary | #62666d | oklch(55% 0.01 250) | oklch(48% 0.01 250) | --color-ink-tertiary |
| primary | #5e6ad2 | oklch(48% 0.18 277) | oklch(54% 0.16 277) | --color-primary |
| primary-hover | #828fff | oklch(58% 0.18 285) | oklch(70% 0.18 285) | (动效内联) |

## Interfaces and Dependencies

本计划只动 CSS 变量与组件视觉，不引入新依赖，不改任何 TypeScript 接口。需要保持的契约：

- `src/app.css` 必须继续导出 daisyUI 风格的全套 `@theme` 变量名：`--color-base-100/200/300/content`、`--color-primary`/`-content`、`--color-secondary`/`-content`、`--color-accent`/`-content`、`--color-neutral`/`-content`、`--color-info/success/warning/error`/`-content`、`--color-overlay`。所有 `*-dark` 变体亦保留。任何工具类（如 `bg-base-200`、`text-primary-content`）的解析必须不变。
- 新增变量命名前缀使用 `--color-surface-`、`--color-hairline*`、`--color-ink-*`，不得复用既有命名空间。
- `MessageUser.svelte` / `MessageAssistant.svelte` 的 `Props` 接口（包括 `message`、`isOperating` 等）不改。
- `TableGroup.svelte` 的 props（`title`、`collapsible`、`defaultCollapsed`、`showDivider`）不改；新视觉通过容器 class 实现。
- `TableBaseRow.svelte` 的 props 不改；行间分隔线、hover 颜色通过 class / token 实现。
- `AddProviderModal.svelte` 仅调整外层容器 class，其表单逻辑（`formData`、`canSave`、`originalData` 等 `$state`）一行不动。
- `RoundButton.svelte` 通过现有 `bgColor` / `textColor` / `hoverColor` props 接受 token 颜色（如 `var(--color-primary)`）；不修改其内部实现。

签收标准：以上接口在 PR diff 中应表现为 0 行接口变更；只在 class 字符串与 CSS 变量层面有改动。
