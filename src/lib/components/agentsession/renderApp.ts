/**
 * render_app toolcall-argument parsing and artifact reconstruction.
 *
 * The backend `render_app` tool (agent_tools.rs) is presentational, like
 * render_card: app content travels only in toolcall block `arguments`. The
 * app's CURRENT state is not stored anywhere — it is derived by replaying the
 * session's `render_app` toolcall blocks in transcript order (`create` sets
 * title+content, `update` replaces them), so it persists with the transcript
 * and survives reloads for free.
 *
 * Arguments arrive as an already-parsed object on the live path but may be a
 * raw JSON string on restored/streaming paths, so both carriers are accepted.
 * Anything that does not yield a well-formed payload is skipped — streaming
 * partials never corrupt the folded artifact.
 */

import type { AgentMessage } from "$lib/types/agentSession";

/** Tool name matched against assistant toolcall blocks (backend TOOL_RENDER_APP). */
export const RENDER_APP_TOOL_NAME = "render_app";

export interface RenderAppArgs {
  command: "create" | "update";
  title?: string;
  content: string;
}

/** The folded app state shown in the side panel. */
export interface AppArtifact {
  title: string;
  content: string;
  /** Id of the last toolcall that contributed content (panel remount key). */
  toolCallId: string;
}

/**
 * Normalise unknown toolcall arguments into {@link RenderAppArgs}, or `null`
 * when they are not (yet) a well-formed render_app payload. Never throws.
 */
export function parseRenderAppArgs(args: unknown): RenderAppArgs | null {
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
  const command = record["command"];
  if (command !== "create" && command !== "update") {
    return null;
  }

  const content = record["content"];
  if (typeof content !== "string" || content.trim().length === 0) {
    return null;
  }

  const title = record["title"];
  return {
    command,
    content,
    title:
      typeof title === "string" && title.trim().length > 0 ? title : undefined,
  };
}

/**
 * Fold the session's `render_app` toolcall blocks into the current app state.
 *
 * Blocks are visited in transcript order (assistant message order × content
 * source order, mirroring the timeline). A block contributes when its args
 * parse AND its execution did not error: errored calls were rejected by the
 * backend validator and must not clobber a previously valid app. Error status
 * is reconciled like the timeline does — live `toolCalls[id]` first, committed
 * `toolResult` pairing as the restored fallback; a call with neither is still
 * executing and already carries validated-shape args, so it contributes
 * (that's what makes the panel update live during a run).
 *
 * A leading `update` without a prior `create` still yields an artifact
 * (model misuse tolerated — showing the app beats dropping it).
 */
export function reconstructAppArtifact(
  messages: AgentMessage[],
  liveToolCalls: Record<string, { status: string; args?: unknown }>,
): AppArtifact | null {
  // toolResult pairing for the restored path (reload: live state is empty).
  const committedErrors = new Map<string, boolean>();
  for (const message of messages) {
    if (message.role === "toolResult") {
      committedErrors.set(message.toolCallId, message.isError);
    }
  }

  let title = "";
  let content: string | null = null;
  let toolCallId = "";

  for (const message of messages) {
    if (message.role !== "assistant") {
      continue;
    }
    for (const block of message.content) {
      if (block.type !== "toolcall" || block.name !== RENDER_APP_TOOL_NAME) {
        continue;
      }

      const live = liveToolCalls[block.id];
      const errored =
        live?.status === "error" ||
        (live === undefined && committedErrors.get(block.id) === true);
      if (errored) {
        continue;
      }

      const parsed = parseRenderAppArgs(live?.args ?? block.arguments);
      if (!parsed) {
        continue;
      }

      if (parsed.title !== undefined) {
        title = parsed.title;
      }
      content = parsed.content;
      toolCallId = block.id;
    }
  }

  return content === null ? null : { title, content, toolCallId };
}
