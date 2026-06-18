/**
 * provider 命名空间文案（由迁移子代理填充；zh 为权威）。
 */
export const providerZh = {
  // 供应商列表页
  "provider.loadingProviders": "正在加载供应商...",
  "provider.emptyHint": "添加 AI 供应商开始使用各种模型",
  "provider.addProvider": "添加供应商",
  "provider.addOtherProvider": "添加其它供应商",

  // 供应商详情页
  "provider.modelList": "模型列表",
  "provider.addModel": "添加模型",
  "provider.addModelPlaceholder": "输入 model id，例如 llama-3.1-8b",
  "provider.adding": "添加中…",
  "provider.add": "添加",
  "provider.addModelFailed": "添加模型失败",
  "provider.refreshModels": "刷新模型列表",
  "provider.backAria": "返回",
  "provider.supportImage": "支持图片生成",
  "provider.removeFromFavorites": "从收藏中移除",
  "provider.addToFavorites": "添加到收藏",
  "provider.viewModelInfo": "查看模型信息",
  "provider.emptyCustomModels": "该自定义供应商暂无模型，点击「添加模型」手动添加端点支持的 model id",
  "provider.emptyModels": "暂无模型数据，请检查供应商配置或网络连接",

  // 删除/禁用确认弹窗
  "provider.deleteProviderTitle": "删除供应商",
  "provider.deleteProviderMessage": "确认要删除 <span class='font-medium'>{name}</span> 吗？",
  "provider.disableProviderTitle": "关闭供应商",
  "provider.disableProviderWithChats":
    "检测到有 <span class='font-medium'>{count}</span> 个会话正在使用 <span class='font-medium'>{name}</span>。<br/><br/>关闭此供应商后，这些会话将无法使用该供应商的模型。<br/><br/>确定要关闭吗？",
  "provider.disableProviderConfirm": "确认关闭 <span class='font-medium'>{name}</span> 吗？",
  "provider.disableModelTitle": "禁用模型",
  "provider.disableModelWithChats":
    "检测到有 <span class='font-medium'>{count}</span> 个会话正在使用模型 <span class='font-medium'>{name}</span>。<br/><br/>禁用此模型后，这些会话将无法使用该模型。<br/><br/>确定要禁用吗？",
  "provider.disableModelConfirm": "确认禁用模型 <span class='font-medium'>{name}</span> 吗？",
  "provider.closeAction": "关闭",
  "provider.disableAction": "禁用",

  // 添加/编辑供应商弹窗
  "provider.editProviderTitle": "编辑供应商",
  "provider.addProviderTitle": "添加供应商",
  "provider.providerType": "供应商类型",
  "provider.providerName": "供应商名称",
  "provider.confirm": "确认",
  "provider.configErrorTitle": "供应商配置错误",
  "provider.operationFailed": "操作失败，请稍后重试",
  "provider.validateName": "请输入供应商名称",
  "provider.validateBaseUrl": "请输入 Base URL",
  "provider.validateApiKey": "请输入 API Key",
  "provider.updateSuccess": "供应商更新成功",
  "provider.createSuccess": "供应商创建成功",

  // 模型信息弹窗
  "provider.modelInfo": "模型信息",
  "provider.emptyModelInfo": "暂无模型信息",
  "provider.viewModelDetail": "查看模型详情",
  "provider.copyModelId": "复制模型 ID",
  "provider.modelId": "模型 ID",
  "provider.contextLength": "上下文长度",
  "provider.maxOutputLength": "最大输出长度",
  "provider.inputPrice": "输入价格",
  "provider.outputPrice": "输出价格",
  "provider.supportedFeatures": "支持特性",
  "provider.inputModalities": "输入模态",
  "provider.outputModalities": "输出模态",
  "provider.supportedMethods": "支持方法",
  "provider.supportedParameters": "支持参数",

  // MCP 列表页
  "provider.loadingMcpServers": "正在加载 MCP 服务器...",
  "provider.mcpEmptyHint": "添加 MCP 服务器来扩展 AI 能力",
  "provider.addMcpServer": "添加 MCP 服务器",
  "provider.mcpToolsSummary": "{total} 个工具，已启用 {enabled} 个",
  "provider.editAria": "编辑",

  // MCP 关闭确认弹窗
  "provider.disableMcpTitle": "关闭 MCP 服务器",
  "provider.disableMcpWithChats":
    "检测到有 <span class='font-medium'>{count}</span> 个会话正在使用 <span class='font-medium'>{name}</span>。<br/><br/>请选择要执行的操作：",
  "provider.disableMcpConfirm": "确认关闭 <span class='font-medium'>{name}</span> 吗？",
  "provider.disableAndRemove": "解除关联后关闭",
  "provider.disableMcpOnly": "仅关闭 MCP",

  // MCP 详情页
  "provider.deleteMcpTitle": "删除 MCP 服务器",
  "provider.deleteMcpMessage":
    "确认要删除 <span class='font-medium'>{name}</span> 吗？<br/><br/>此操作无法撤销。",
  "provider.lastSync": "最后同步: {time}",
  "provider.tabTools": "工具",
  "provider.tabPrompts": "提示",
  "provider.tabResources": "资源",
  "provider.emptyTools": "暂无工具数据",
  "provider.emptyPrompts": "暂无提示数据",
  "provider.emptyResources": "暂无资源数据",
  "provider.params": "参数",
  "provider.paramsWithCount": "参数 ({count})",
  "provider.detail": "详情",

  // MCP 表单弹窗
  "provider.editMcpTitle": "编辑 MCP 服务器",
  "provider.addMcpTitle": "添加 MCP 服务器",
  "provider.mcpName": "名称",
  "provider.mcpNamePlaceholder": "唯一名称，如 filesystem",
  "provider.mcpDisplayName": "显示名称",
  "provider.mcpDisplayNamePlaceholder": "可选的用户可读名称",
  "provider.connectionType": "连接类型",
  "provider.connectionStdio": "标准输入输出 (stdio)",
  "provider.connectionSse": "服务器发送事件 (SSE)",
  "provider.connectionHttp": "流式传输 HTTP",
  "provider.mcpCommand": "命令",
  "provider.mcpCommandPlaceholder": "如 npx 或 uvx",
  "provider.mcpArgs": "参数",
  "provider.mcpArgsPlaceholder": "一行一个，或使用逗号分隔",
  "provider.mcpWorkingDir": "工作目录",
  "provider.optional": "可选",
  "provider.mcpEndpoint": "端点 URL",
  "provider.mcpEndpointPlaceholder": "如 http://localhost:3000/mcp 或 ws://localhost:8080",
  "provider.mcpTimeout": "超时时间 (毫秒)",
  "provider.mcpTimeoutPlaceholder": "可选，默认无超时",
  "provider.envVars": "环境变量",
  "provider.addEntry": "新增",
  "provider.envKeyPlaceholder": "键",
  "provider.envValuePlaceholder": "值",
  "provider.httpHeaders": "HTTP 头部",
  "provider.headerKeyPlaceholder": "头部名称",
  "provider.headerValuePlaceholder": "头部值",
  "provider.validateMcpName": "请输入服务器名称",
  "provider.validateCommand": "请输入执行命令",
  "provider.validateEndpoint": "请输入端点 URL",
  "provider.validateTimeout": "超时时间必须是数字",
  "provider.saveFailed": "保存失败，请重试",

  // MCP 文本编辑弹窗
  "provider.editMcpJsonTitle": "编辑 MCP 服务器",
  "provider.mcpJsonPlaceholder": "请输入 MCP 服务器配置...",
  "provider.validateMcpJson": "请输入 MCP 服务器配置",
};
