/**
 * quickaction 命名空间文案（Quick Action 浮层；zh 为权威）。
 */
export const quickactionZh = {
  "quickaction.placeholder": "输入你想做的事…",
  "quickaction.send": "发送",
  "quickaction.stop": "停止",
  "quickaction.newClear": "新建",
  // run 启动失败的兜底提示（错误无 message 时）。
  "quickaction.runFailed": "启动失败，请重试。",
  // 浮层会话的默认名（一次性 sandbox 会话）。
  "quickaction.sessionName": "快捷动作",
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
};
