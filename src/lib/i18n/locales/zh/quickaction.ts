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

  // The overlay runs on the app-wide default model; unset, dangling or an
  // empty catalog all land here — the fix is the same: go pick a default.
  "quickaction.model.unavailable":
    "默认模型不可用，请先在设置 →「Agent 工具」中选择默认模型",
};
