/**
 * Open/closed state of the `render_app` preview panel, keyed by session: it
 * mounts only while `openSessionId` is the current session, so switching
 * sessions hides it. Panel content lives outside this store — the page derives
 * the artifact by replaying the transcript (`reconstructAppArtifact`); this
 * store answers only "which session's panel is open".
 */

class AgentAppPanelStore {
  /** Session whose panel is open; null = closed. */
  openSessionId = $state<string | null>(null);

  open(sessionId: string): void {
    this.openSessionId = sessionId;
  }

  close(): void {
    this.openSessionId = null;
  }
}

export const agentAppPanel = new AgentAppPanelStore();
