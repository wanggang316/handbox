/**
 * quickaction namespace strings (Quick Action overlay; zh is canonical).
 */
export const quickactionZh = {
  "quickaction.placeholder": "输入你想做的事…",
  // Overlay step 1: search placeholder that filters the agent list as you type.
  "quickaction.searchPlaceholder": "搜索 Agent…",
  // Overlay step 2: message placeholder once an agent is selected ({name} = agent name).
  "quickaction.messagePlaceholder": "给 {name} 发消息…",
  "quickaction.send": "发送",
  "quickaction.continueInChat": "在对话中继续",
  "quickaction.stop": "停止",
  "quickaction.newClear": "新建",
  // Key hints: pick the highlighted agent / move through the list.
  "quickaction.select": "选择",
  "quickaction.navigate": "切换",
  // Fallback error when sending fails without an error message.
  "quickaction.runFailed": "发送失败，请重试。",
  // Default name for the overlay's throwaway sandbox session.
  "quickaction.sessionName": "快捷动作",
  // Empty state when no agents exist yet (points to the app to create one).
  "quickaction.noAgents.title": "尚无可用 Agent",
  "quickaction.noAgents.description": "在应用的「Agents」中创建一个后即可在此对话。",
  // Empty state when agents exist but the search has no match.
  "quickaction.noMatch": "没有匹配的 Agent",
  // Empty state guiding model setup when none is available.
  "quickaction.noModel.title": "尚未配置可用模型",
  "quickaction.noModel.description": "在设置中启用一个供应商并选择默认模型后即可使用。",
  "quickaction.noModel.openSettings": "前往设置",

  // Settings page · shortcut recorder
  "quickaction.shortcut.title": "唤起快捷动作",
  "quickaction.shortcut.label": "全局快捷键",
  "quickaction.shortcut.hint": "在任意应用中按下此组合即可唤起快捷动作浮层。",
  "quickaction.shortcut.recording": "请按下快捷键…",
  "quickaction.shortcut.record": "录制",
  "quickaction.shortcut.reset": "恢复默认",
  // Validation guidance (maps to the pure helper's invalid reasons).
  "quickaction.shortcut.invalid.modifierOnly": "请至少包含一个普通按键，例如 ⌘⇧Space。",
  "quickaction.shortcut.invalid.noModifier": "请至少包含一个修饰键（⌘ / ⌃ / ⌥ / ⇧）。",
  "quickaction.shortcut.invalid.unsupportedKey": "该按键不受支持，请换一个组合。",
  // Fallback error when registration fails without an error message.
  "quickaction.shortcut.registerFailed": "快捷键注册失败，请更换组合后重试。",

  // Settings page · default-model selector
  "quickaction.model.title": "默认模型",
  "quickaction.model.label": "默认模型",
  "quickaction.model.hint": "唤起快捷动作浮层后默认使用的模型，可在浮层内临时切换。",
  // No default model chosen yet (catalog non-empty, nothing selected).
  "quickaction.model.none": "未选择",
  // Chosen model's provider was removed or disabled (dangling reference): prompt to re-pick while keeping the stale value on disk.
  "quickaction.model.unavailable": "所选模型已不可用，请重新选择",
  // Empty catalog (no enabled providers/models): guide to configure a provider first.
  "quickaction.model.emptyCatalog": "请先在「模型」中启用一个供应商并添加模型。",
  "quickaction.model.openModels": "前往模型设置",
};
