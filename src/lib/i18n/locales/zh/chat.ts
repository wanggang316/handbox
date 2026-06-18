/**
 * chat 命名空间文案（由迁移子代理填充；zh 为权威）。
 */
export const chatZh = {
  // 空状态 / 加载
  "chat.loadingMessages": "加载消息中...",
  "chat.startNewConversation": "开始新的对话",

  // 删除 / 重发 / 重新生成 确认对话框
  "chat.deleteConfirmTitle": "确认删除",
  "chat.deleteConfirmMessage": "确定要删除这条消息吗？",
  "chat.resendConfirmTitle": "确认重发",
  "chat.resendConfirmMessage": "重发此消息将删除它之后的所有消息，确定要继续吗？",
  "chat.regenerateConfirmTitle": "确认重新生成",
  "chat.regenerateConfirmMessage": "重新生成此回复将删除该消息及之后的所有消息，确定要继续吗？",
  "chat.resend": "重发",
  "chat.regenerate": "重新生成",

  // 输入框
  "chat.editMessage": "编辑消息",
  "chat.cancelEdit": "取消编辑",
  "chat.editMessagePlaceholder": "编辑消息内容...",
  "chat.inputPlaceholder": "在这里输入消息，按 Enter 发送",
  "chat.addAttachment": "添加附件",
  "chat.uploadImage": "上传图片",
  "chat.removeImage": "移除图片",
  "chat.updateMessage": "更新消息",

  // 模型选择
  "chat.selectModel": "选择模型",
  "chat.searchModelPlaceholder": "搜索模型...",
  "chat.loadingModels": "正在加载模型...",
  "chat.modelCount": "共找到 {count} 个模型",
  "chat.allProviders": "全部供应商",
  "chat.favorites": "收藏",
  "chat.noMatchingModels": "未找到匹配的模型",
  "chat.adjustSearchHint": "尝试调整搜索条件或清除过滤器",
  "chat.supportsImageGeneration": "支持图片生成",
  "chat.contextLength": "上下文长度",
  "chat.maxOutputLength": "最大输出长度",
  "chat.inputPrice": "输入价格",
  "chat.outputPrice": "输出价格",
  "chat.modelProvider": "模型供应商",
  "chat.noModelSelected": "未选择模型",
  "chat.selectModelToStart": "选择一个模型以开始对话",

  // 聊天头部 / 设置
  "chat.settings": "设置",
  "chat.chatSettings": "聊天设置",
  "chat.advanced": "高级",

  // System Prompt
  "chat.noSystemPrompt": "暂无系统提示词",
  "chat.editSystemPrompt": "编辑系统提示词",
  "chat.systemPromptPlaceholder": "输入系统提示词...",
  "chat.characterCount": "字符数: {count}",

  // Reasoning / Thinking
  "chat.followModel": "跟随模型",
  "chat.effort": "难度",
  "chat.summary": "总结",
  "chat.includeReasoning": "包含推理",
  "chat.includeThoughts": "包含过程",
  "chat.budgetMode": "预算模式",

  // Tools / MCP
  "chat.tools": "工具",
  "chat.autoExecution": "自动执行",
  "chat.manualExecution": "手动执行",
  "chat.selectOrCreateChatFirst": "请先选择或创建聊天",
  "chat.serversSelected": "已选中 {count} 个服务器",
  "chat.mcpAssociatedWithChat": "MCP 服务器配置将与聊天关联",
  "chat.noAvailableMcpServers": "暂无可用的 MCP 服务器",
  "chat.configureMcpInSettings": "请在应用设置中配置并开启 MCP 服务器",
  "chat.enabledToolsCount": "{count} enabled tools",
  "chat.disabledServersHeading": "已关闭的服务器 ({count})",
  "chat.serverDisabled": "● 服务器已关闭",
  "chat.serverNotReady": "● 服务器未就绪",
  "chat.serverDeleted": "● 服务器已删除",
  "chat.serverDisabledHint": "此服务器已在全局设置中关闭，请启用后再使用",
  "chat.serverNotReadyHint": "此服务器状态异常，请检查配置",
  "chat.serverDeletedHint": "此服务器已被删除，建议移除此配置",

  // 消息操作
  "chat.copyMessage": "复制消息",
  "chat.editAndResend": "编辑并重发",
  "chat.resendMessage": "重发消息",
  "chat.deleteMessage": "删除消息",
  "chat.openInSystemPreview": "点击在系统预览中打开",
  "chat.favoriteRangeActions": "收藏范围操作",
  "chat.unfavorite": "取消收藏",

  // Assistant 消息
  "chat.reasoningInProgress": "推理中...",
  "chat.reasoningProcess": "推理过程",
  "chat.generatingImage": "图像生成中…",
  "chat.copyImage": "复制图片",
  "chat.saveImage": "保存图片",
  "chat.favoriteImage": "收藏图片",
  "chat.openInFinder": "在 Finder 中打开",

  // 工具调用
  "chat.toolPending": "待执行",
  "chat.toolExecuting": "执行中",
  "chat.toolCompleted": "完成",
  "chat.toolFailed": "失败",
  "chat.toolUnknown": "未知",
  "chat.toolFallbackName": "工具 {index}",
  "chat.execute": "执行",
  "chat.reExecute": "重新执行",

  // 页面级提示 / 标题生成
  "chat.titleGenerationFailed": "自动生成标题失败，可右键会话手动生成",
  "chat.reasonServerDisabled": "服务器已关闭",
  "chat.reasonServerNotReady": "服务器未就绪",
  "chat.reasonServerDeleted": "服务器已删除",
  "chat.disabledMcpDetectedTitle": "检测到已关闭的 MCP 服务器",
  "chat.disabledMcpDetectedMessage":
    "当前聊天配置中有 <span class='font-medium'>{count}</span> 个 MCP 服务器在全局设置中被关闭：<br/>{list}",
};
