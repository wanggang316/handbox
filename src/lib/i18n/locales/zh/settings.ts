/**
 * Settings page strings.
 */
export const settingsZh = {
  "settings.general.appearance": "外观样式",
  "settings.general.theme.system": "跟随系统",
  "settings.general.theme.light": "浅色主题",
  "settings.general.theme.dark": "深色主题",
  "settings.general.language": "语言",
  "settings.general.autoScroll": "聊天界面自动下滑",
  "settings.general.section": "通用",
  "settings.general.appearanceDesc": "选择界面的明暗配色方案",
  "settings.general.languageDesc": "界面显示语言",
  "settings.general.autoScrollDesc": "有新消息时自动滚动到底部",
  "settings.general.sidebarVibrancy": "侧边栏毛玻璃效果",
  "settings.general.sidebarVibrancyDesc":
    "侧边栏背景呈现 macOS 原生半透明磨砂质感",

  // Sidebar
  "settings.sidebar.backToApp": "返回应用",
  "settings.sidebar.search": "搜索设置…",
  "settings.sidebar.group.general": "通用",
  "settings.sidebar.group.agents": "Agents",
  "settings.sidebar.group.other": "其他",
  "settings.sidebar.account": "账户",
  "settings.sidebar.general": "通用",
  "settings.sidebar.session": "会话",
  "settings.sidebar.quicktools": "快捷工具",
  "settings.sidebar.models": "模型",
  "settings.sidebar.agentTools": "Agent 工具",
  "settings.sidebar.hooks": "Hooks",
  "settings.sidebar.skills": "技能",
  "settings.sidebar.components": "组件",
  "settings.sidebar.shortcuts": "快捷键",
  "settings.sidebar.about": "关于",

  // About page
  "settings.about.softwareUpdate": "软件更新",
  "settings.about.autoCheck": "自动检查更新",
  "settings.about.autoCheckHint": "启动时自动检查",
  "settings.about.checkUpdate": "检查更新",
  "settings.about.checking": "检查中…",
  "settings.about.updateAvailable": "发现新版本 v{version}",
  "settings.about.currentVersion": "当前版本 v{version}",
  "settings.about.title": "关于",
  "settings.about.changelog": "更新日志",
  "settings.about.officialSite": "官方网站",

  // Quick Tools page
  "settings.quicktools.selectionToolbarGroup": "选中文本工具栏",
  "settings.quicktools.showToolbarOnSelection": "选中文本显示工具栏",
  "settings.quicktools.translationAgent": "划词翻译 Agent",
  "settings.quicktools.translationAgentDesc":
    "工具栏「翻译」使用的 Agent；未选择时使用内置翻译",
  "settings.quicktools.translationAgentDefault": "内置翻译",
  "settings.quicktools.quickActionGroup": "Quick Action",
  "settings.quicktools.enableQuickAction": "启用 Quick Action",
  "settings.quicktools.enableQuickActionDesc":
    "通过全局快捷键 {shortcut} 唤起快捷动作浮层",
  "settings.quicktools.permissionRequired": "需要辅助功能权限",
  "settings.quicktools.disabledApps": "禁用的应用",
  "settings.quicktools.disabledAppsEmpty":
    "禁止使用划词工具的应用将显示在这里。",
  "settings.quicktools.permissionGuide":
    '启用此功能需要授予辅助功能权限。请前往"系统设置 > 隐私与安全性 > 辅助功能"，并启用 HandBox 的权限。',
  "settings.quicktools.openSystemSettings": "打开系统设置",
  "settings.quicktools.refreshPermission": "刷新权限状态",

  // Session page
  "settings.session.section": "会话标题",
  "settings.session.titleGeneration": "标题生成",
  "settings.session.titleGeneration.firstMessage": "首次发送消息后",
  "settings.session.titleGeneration.everyMessage": "每次发送消息后",
  "settings.session.titleGeneration.off": "关闭",

  // Agent Tools page
  "settings.agentTools.title": "Agent 工具",
  "settings.agentTools.description":
    "新建 Agent 会话默认启用的工具。已存在的会话不受影响。",
  "settings.agentTools.webSearch.title": "网络搜索",
  "settings.agentTools.webSearch.provider": "搜索服务商",
  "settings.agentTools.webSearch.apiKey": "API Key",
  "settings.agentTools.webSearch.apiKeyPlaceholder": "tvly-...",
  "settings.agentTools.system.title": "System",
  "settings.agentTools.uiExtensions.title": "UI",
  "settings.agentTools.skill.title": "Skill",
  "settings.agentTools.renderCardDesc": "在会话中内联渲染交互式 HTML 卡片",
  "settings.agentTools.renderAppDesc":
    "在侧边面板生成完整的 HTML 应用（预览 + 源码）",
  "settings.agentTools.askQuestionDesc": "让模型在需要你拍板时弹出提问面板，等你作答后再继续",
  "settings.agentTools.skillDesc": "允许模型发现并按需加载 Skill",

  // Skills page
  "settings.skills.title": "技能",
  "settings.skills.description":
    "将 SKILL.md 放入技能目录后会在此处展示，可启停有效的技能",
  "settings.skills.loading": "正在加载技能...",
  "settings.skills.scope.user": "用户",
  "settings.skills.scope.project": "项目",
  "settings.skills.scope.appData": "应用",
  "settings.skills.openDir": "打开目录",
  "settings.skills.collapseBody": "收起内容",
  "settings.skills.expandBody": "查看内容",
  "settings.skills.empty": "暂无技能",
  "settings.skills.emptyHint":
    "在技能目录中放入 SKILL.md 文件，然后点击「刷新」即可在此处看到。",

  // Hook rules page
  "settings.hooks.description":
    "在 Agent 生命周期的关键节点（调用工具、提交提示词、回复结束、等待审批）自动执行动作，按顺序匹配，命中的第一条生效",
  "settings.hooks.add": "添加规则",
  "settings.hooks.addTitle": "添加规则",
  "settings.hooks.editTitle": "编辑规则",
  "settings.hooks.empty": "暂无规则",
  "settings.hooks.emptyHint":
    "添加规则可在工具调用前后自动执行命令（格式化、提交、注入上下文等），或在命中特定操作时收到提醒。",
  "settings.hooks.anyArgument": "任意参数",
  "settings.hooks.promptSubject": "提示词",
  "settings.hooks.replySubject": "回复",
  "settings.hooks.event.before": "工具调用前",
  "settings.hooks.event.after": "工具调用后",
  "settings.hooks.event.prompt": "提交提示词时",
  "settings.hooks.event.turnEnd": "回复结束时",
  "settings.hooks.event.approval": "等待审批时",
  "settings.hooks.action.notify": "提醒",
  "settings.hooks.action.runCommand": "执行命令",
  "settings.hooks.field.name": "名称",
  "settings.hooks.field.namePlaceholder": "例如：写入后自动格式化",
  "settings.hooks.field.event": "触发时机",
  "settings.hooks.field.action": "动作",
  "settings.hooks.field.toolPattern": "工具名",
  "settings.hooks.field.argField": "参数名（可选）",
  "settings.hooks.field.argContains": "参数包含（可选）",
  "settings.hooks.field.promptContains": "提示词包含（可选）",
  "settings.hooks.field.promptContainsPlaceholder": "留空则每次提交都触发",
  "settings.hooks.field.replyContains": "回复包含（可选）",
  "settings.hooks.field.replyContainsPlaceholder": "留空则每次回复结束都触发",
  "settings.hooks.field.command": "命令",
  "settings.hooks.field.commandHint":
    '脚本通过 /bin/sh 在会话工作目录执行，事件 JSON 从标准输入传入，默认 10 秒超时。\n\n可用环境变量\n· $HANDBOX_HOOK_EVENT / $HANDBOX_TOOL_NAME\n· $HANDBOX_SESSION_ID / $HANDBOX_RULE_NAME\n\n输出的含义\n· 普通输出：提交提示词时作为上下文交给模型\n· {"decision":"deny","reason":"…"}：拦截本次调用\n· {"updatedInput":{…}}：改写参数；调用后则改写结果（可脱敏）\n· 非零退出码：视为拦截\n\n各时机的差异\n· 回复结束时：deny 会让 Agent 带着 reason 继续工作（最多续 3 轮）；命令失败只提示\n· 等待审批时：仅执行副作用（如响铃、推送），输出不影响审批',
  "settings.hooks.field.message": "说明（可选）",
  "settings.hooks.field.messagePlaceholder": "命中时随提醒一起展示",
  "settings.hooks.field.hint":
    "填写要匹配的工具名，* 是通配符：\n· write — 精确匹配\n· mcp__* — 前缀匹配\n· * — 匹配全部工具\n\n参数名留空：在全部参数中查找\n参数包含留空：只按工具名匹配",
  "settings.hooks.deleteTitle": "删除规则",
  "settings.hooks.deleteMessage": "确定要删除「{name}」吗？",

  // Account page
  "settings.account.editProfile": "编辑资料",
  "settings.account.loggingOut": "退出中...",
  "settings.account.logout": "退出登录",
  "settings.account.updateFailed": "更新失败，请重试",
  "settings.account.logoutFailed": "退出失败，请重试",
  "settings.account.notLoggedIn": "未登录",
  "settings.account.defaultUsername": "用户",
  "settings.account.username": "用户名",
  "settings.account.usernamePlaceholder": "请输入",
};
