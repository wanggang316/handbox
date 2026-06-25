/**
 * quickaction 命名空间文案（Quick Action 浮层；zh 为权威）。
 */
export const quickactionZh = {
  "quickaction.placeholder": "输入你想做的事…",
  // 浮层第一步：输入即过滤 Agent 列表的搜索占位符。
  "quickaction.searchPlaceholder": "搜索 Agent…",
  // 浮层第二步：选中 Agent 后的消息输入占位符（{name} 为 Agent 名）。
  "quickaction.messagePlaceholder": "给 {name} 发消息…",
  "quickaction.send": "发送",
  "quickaction.continueInChat": "在对话中继续",
  "quickaction.stop": "停止",
  "quickaction.newClear": "新建",
  // 键位提示：选择高亮的 Agent / 在列表中上下切换。
  "quickaction.select": "选择",
  "quickaction.navigate": "切换",
  // 发送失败的兜底提示（错误无 message 时）。
  "quickaction.runFailed": "发送失败，请重试。",
  // 浮层会话的默认名（一次性 sandbox 会话）。
  "quickaction.sessionName": "快捷动作",
  // 尚无 Agent 时的引导空状态（去应用里创建）。
  "quickaction.noAgents.title": "尚无可用 Agent",
  "quickaction.noAgents.description": "在应用的「Agents」中创建一个后即可在此对话。",
  // 有 Agent 但搜索无匹配时的空状态。
  "quickaction.noMatch": "没有匹配的 Agent",
  // 无可用模型时的配置引导空状态。
  "quickaction.noModel.title": "尚未配置可用模型",
  "quickaction.noModel.description": "在设置中启用一个供应商并选择默认模型后即可使用。",
  "quickaction.noModel.openSettings": "前往设置",

  // 设置页 · 快捷键录制器
  "quickaction.shortcut.title": "唤起快捷动作",
  "quickaction.shortcut.label": "全局快捷键",
  "quickaction.shortcut.hint": "在任意应用中按下此组合即可唤起快捷动作浮层。",
  "quickaction.shortcut.recording": "请按下快捷键…",
  "quickaction.shortcut.record": "录制",
  "quickaction.shortcut.reset": "恢复默认",
  // 校验类引导（与纯 helper 的 invalid reason 对应）。
  "quickaction.shortcut.invalid.modifierOnly": "请至少包含一个普通按键，例如 ⌘⇧Space。",
  "quickaction.shortcut.invalid.noModifier": "请至少包含一个修饰键（⌘ / ⌃ / ⌥ / ⇧）。",
  "quickaction.shortcut.invalid.unsupportedKey": "该按键不受支持，请换一个组合。",
  // 注册失败兜底提示（错误无 message 时）。
  "quickaction.shortcut.registerFailed": "快捷键注册失败，请更换组合后重试。",

  // 设置页 · 默认模型选择
  "quickaction.model.title": "默认模型",
  "quickaction.model.label": "默认模型",
  "quickaction.model.hint": "唤起快捷动作浮层后默认使用的模型，可在浮层内临时切换。",
  // 尚未选择默认模型（catalog 非空，但未指定）。
  "quickaction.model.none": "未选择",
  // 已选模型的供应商已被删除或停用，引用悬空：提示重新选择，但保留磁盘上的旧值。
  "quickaction.model.unavailable": "所选模型已不可用，请重新选择",
  // catalog 为空（无已启用的供应商/模型）：引导先去配置供应商。
  "quickaction.model.emptyCatalog": "请先在「模型」中启用一个供应商并添加模型。",
  "quickaction.model.openModels": "前往模型设置",
};
