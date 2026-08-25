/**
 * Pending reply quote per session: the transcript text a reader selected and
 * handed to the composer through the floating reply affordance.
 *
 * Keyed by session so a quote never leaks into another session's composer, and
 * so it survives the composer remount that a session switch performs. The
 * composer owns the lifetime: it clears the entry once the message carrying the
 * quote is sent (or when the user dismisses it).
 */

class AgentQuoteStore {
  // Reassigned rather than mutated on write: entries are tiny, and a new object
  // keeps reads of not-yet-present keys reactive without relying on proxy
  // bookkeeping for added properties.
  private quotes = $state<Record<string, string>>({});

  /** Quoted text pending for a session; null when nothing is quoted. */
  quoteFor(sessionId: string): string | null {
    return this.quotes[sessionId] ?? null;
  }

  /** Replace the session's pending quote (only one quote at a time). */
  set(sessionId: string, text: string): void {
    this.quotes = { ...this.quotes, [sessionId]: text };
  }

  clear(sessionId: string): void {
    if (!(sessionId in this.quotes)) {
      return;
    }
    const next = { ...this.quotes };
    delete next[sessionId];
    this.quotes = next;
  }
}

export const agentQuoteStore = new AgentQuoteStore();
