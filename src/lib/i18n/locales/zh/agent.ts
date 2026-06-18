/**
 * agent 命名空间文案（由迁移子代理填充；zh 为权威）。
 */
export const agentZh = {
  // System Prompt popover (AgentSessionHeader)
  "agent.systemPrompt.editAria": "编辑 System Prompt",
  "agent.systemPrompt.placeholder": "输入系统提示词...",
  "agent.systemPrompt.saveFailed": "保存失败：{error}",

  // Thinking-level selector (AgentInput)
  "agent.thinking.off": "关闭",
  "agent.thinking.low": "低",
  "agent.thinking.medium": "中",
  "agent.thinking.high": "高",

  // Input composer (AgentInput)
  "agent.input.oversizeSkipped": "部分图片超过 10MB 已跳过",
  "agent.input.steerFailed": "发送 steering 消息失败",
  "agent.input.selectModelFirst": "请先选择模型",
  "agent.input.runFailed": "启动 Agent 运行失败",
  "agent.input.removeImage": "移除图片",
  "agent.input.awaitingApprovalPlaceholder": "等待审批中，请在弹窗中允许或拒绝",
  "agent.input.placeholder": "在这里输入消息，按 Enter 发送",
  "agent.input.awaitingApprovalHint": "等待工具审批，对话已暂停",
  "agent.input.addImage": "添加图片",
  "agent.input.uploadImage": "上传图片",
  "agent.input.tools": "工具",
  "agent.input.workingDirRequired": "需设置工作目录后才能启用工具",
  "agent.input.toolNeedsWorkingDir": "{label}（需设置工作目录）",
  "agent.input.stop": "停止",
  "agent.input.send": "发送",

  // Timeline (AgentTimeline)
  "agent.timeline.compacting": "整理上下文中…",
  "agent.timeline.usageInput": "输入 {count}",
  "agent.timeline.usageOutput": "输出 {count}",

  // Thinking block (AgentThinkingBlock)
  "agent.thinkingBlock.streaming": "思考中...",
  "agent.thinkingBlock.title": "思考",

  // Built-in tool labels (constants/agentTools.ts; shared by settings + AgentInput).
  // Kept identical to the coding-agent registration id so the UI reads the same
  // name the backend gates on — no per-language alias to mentally map back.
  "agent.tool.read": "read",
  "agent.tool.write": "write",
  "agent.tool.edit": "edit",
  "agent.tool.bash": "bash",
  "agent.tool.grep": "grep",
  "agent.tool.find": "find",
  "agent.tool.ls": "ls",

  // Tool-call card (AgentToolCallCard)
  "agent.toolCall.executing": "执行中",
  "agent.toolCall.completed": "完成",
  "agent.toolCall.error": "失败",
  "agent.toolCall.fallbackName": "工具",
  "agent.toolCall.resultImageAlt": "工具结果图片",

  // Approval modal (AgentApprovalModal)
  "agent.approval.toolWrite": "写入文件",
  "agent.approval.toolEdit": "编辑文件",
  "agent.approval.toolBash": "执行命令",
  "agent.approval.toolFallback": "工具调用",
  "agent.approval.title": "需要你的确认",
  "agent.approval.intro": "Agent 请求执行以下操作，确认后才会运行。请核对参数。",
  "agent.approval.command": "命令",
  "agent.approval.targetPath": "目标路径",
  "agent.approval.content": "内容",
  "agent.approval.fullArgs": "完整参数",
  "agent.approval.deny": "拒绝",
  "agent.approval.allowOnce": "本次允许",
  "agent.approval.allowAlways": "始终允许",

  // Skill slash popover (SkillSlashPopover)
  "agent.slash.ariaLabel": "Skill 自动补全",
  "agent.slash.noMatch": "无匹配的 skill",

  // Project / session list (AgentProjectList)
  "agent.list.renamePlaceholder": "输入新名称",
  "agent.list.heading": "Agent 会话",
  "agent.list.pickProjectDir": "选择项目目录",
  "agent.list.loadFailed": "列表加载失败",
  "agent.list.emptyHint": "点击 + 选择项目目录开始",
  "agent.list.noChats": "No chats",
  "agent.list.ungrouped": "未分组",
  "agent.list.newSession": "新建会话",
  "agent.list.newSessionInProject": "在项目 {name} 中新建会话",
  "agent.list.copyPath": "复制路径",
  "agent.list.deleteProject": "删除项目",
  "agent.list.copyId": "复制ID",
  "agent.list.untitledSession": "未命名",
  "agent.list.deleteProjectConfirm": "将删除项目“{name}”及其 {count} 个会话，不可恢复。",
  "agent.list.deleteProjectFailed": "删除项目失败",
  "agent.list.createProjectFailed": "创建项目失败",
  "agent.list.createSessionFailed": "创建会话失败",

  // Agent form modal (AgentFormModal)
  "agent.form.nameRequired": "请输入 Agent 名称",
  "agent.form.saveFailed": "保存失败，请重试",
  "agent.form.editTitle": "编辑 Agent",
  "agent.form.createTitle": "新建 Agent",
  "agent.form.nameLabel": "名称",
  "agent.form.namePlaceholder": "输入 Agent 名称",
  "agent.form.modelLabel": "模型",
  "agent.form.modelPlaceholder": "输入模型标识符 (例如: gpt-4, claude-3-5-sonnet-20241022)",
  "agent.form.modelHint": "模型标识符可以是任何字符串，不限于已配置的模型",
  "agent.form.systemPromptTitle": "系统提示词",
  "agent.form.charCount": "{count} 字符",
  "agent.form.skillsTitle": "技能",
  "agent.form.skillsLabel": "技能标签",
  "agent.form.skillsPlaceholder": "例如: coding, writing, translation",
  "agent.form.skillsHint": "用逗号分隔多个技能标签",
  "agent.form.modelParams": "模型参数",
  "agent.form.mcpServers": "MCP 服务器",
  "agent.form.mcpComingSoon": "MCP 服务器配置即将推出，敬请期待...",

  // Agent session landing page (agent/+page.svelte)
  "agent.page.startConversation": "开始与 {name} 对话",
  "agent.page.landingWithProjects": "在左侧选择一个会话，或在项目上点 + 新建",
  "agent.page.landingNoProjects": "先在左侧点 + 选择项目目录",

  // Agents management page (agents/+page.svelte)
  "agent.manage.count": "{count} 个",
  "agent.manage.newAgent": "新建 Agent",
  "agent.manage.searchPlaceholder": "搜索 Agent 名称或技能...",
  "agent.manage.noMatch": "没有找到匹配的 Agent",
  "agent.manage.clearSearch": "清除搜索",
  "agent.manage.empty": "还没有创建任何 Agent",
  "agent.manage.emptyHint": "点击上方按钮创建您的第一个 Agent",
  "agent.manage.use": "使用",
  "agent.manage.modelUnset": "未设置",
  "agent.manage.deleteTitle": "删除 Agent",
  "agent.manage.deleteConfirm": "确定要删除这个 Agent 吗？此操作无法撤销。",
};
