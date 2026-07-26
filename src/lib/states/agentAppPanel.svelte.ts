/**
 * Agent 应用面板开合状态 - Svelte 5 runes
 *
 * `render_app` 工具的右侧预览面板按 sessionId 分键开合：面板只在
 * `openSessionId === 当前会话` 时挂载，切换会话即自然隐藏（回来后 pill 可再打开）。
 * 面板内容不进本 store —— artifact 由页面从 transcript 重放推导
 * （`reconstructAppArtifact`），本 store 只回答「哪个会话的面板是打开的」。
 *
 * 打开路径有二：timeline 里的 AppPill 点击（用户主动），以及 run 期间新的
 * render_app 调用抵达时页面 effect 的自动打开（live 跟手）。关闭仅经显式
 * `close()`（面板右上角 X）。
 */

class AgentAppPanelStore {
  /** 面板打开的会话 id；null = 关闭。 */
  openSessionId = $state<string | null>(null);

  open(sessionId: string): void {
    this.openSessionId = sessionId;
  }

  close(): void {
    this.openSessionId = null;
  }
}

export const agentAppPanel = new AgentAppPanelStore();
