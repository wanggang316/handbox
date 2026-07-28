/**
 * render_card toolcall-argument parsing.
 *
 * The backend `render_card` tool (agent_tools.rs) is presentational: the card
 * content travels only in the toolcall block's `arguments`, which this module
 * normalises for the timeline. Arguments arrive as an already-parsed object on
 * the live path but may be a raw JSON string on restored/streaming paths, so
 * both carriers are accepted. Anything that does not yield a non-empty `html`
 * string is `null` — the caller falls back to an error presentation, never a
 * broken iframe.
 */

/** Tool name matched against assistant toolcall blocks (backend TOOL_RENDER_CARD). */
export const RENDER_CARD_TOOL_NAME = "render_card";

export interface RenderCardArgs {
  html: string;
  title?: string;
}

/**
 * Normalise unknown toolcall arguments into {@link RenderCardArgs}, or `null`
 * when they are not (yet) a well-formed render_card payload. Never throws.
 */
export function parseRenderCardArgs(args: unknown): RenderCardArgs | null {
  let candidate: unknown = args;

  if (typeof candidate === "string") {
    try {
      candidate = JSON.parse(candidate);
    } catch {
      return null;
    }
  }

  if (
    typeof candidate !== "object" ||
    candidate === null ||
    Array.isArray(candidate)
  ) {
    return null;
  }

  const record = candidate as Record<string, unknown>;
  const html = record["html"];
  if (typeof html !== "string" || html.trim().length === 0) {
    return null;
  }

  const title = record["title"];
  return {
    html,
    title:
      typeof title === "string" && title.trim().length > 0 ? title : undefined,
  };
}
