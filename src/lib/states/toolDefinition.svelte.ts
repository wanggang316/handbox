/**
 * User-defined tool ("dynamic tool") state — Svelte 5 runes.
 *
 * Two very different readers share this store:
 *  - the settings surfaces, which mutate through the actions below;
 *  - the chat timeline, which only ever asks "is this tool name one of mine,
 *    and what does it look like" while rendering a transcript.
 *
 * The second is why {@link toolDefinitionActions.ensureLoaded} exists: the
 * timeline must not re-fetch on every mount, and must not stall a paint waiting
 * for tools it may not need. It loads once per session, de-duplicates
 * concurrent callers, and leaves the list empty on failure — an unresolved tool
 * name simply renders as an ordinary tool call.
 */

import type {
  CreateToolDefinitionRequest,
  ToolDefinition,
  UpdateToolDefinitionRequest,
} from "../types";
import * as toolApi from "../api/toolDefinition";

export const toolDefinitionState = $state({
  tools: [] as ToolDefinition[],
  isLoading: false,
  error: null as string | null,
  /** True once a load has succeeded; `ensureLoaded` is a no-op afterwards. */
  loaded: false,
});

/** In-flight load, so concurrent `ensureLoaded` callers share one round trip. */
let loadInFlight: Promise<void> | null = null;

function replace(tool: ToolDefinition): void {
  const index = toolDefinitionState.tools.findIndex((t) => t.id === tool.id);
  if (index !== -1) {
    toolDefinitionState.tools[index] = tool;
  }
}

function message(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

export const toolDefinitionActions = {
  async loadTools(): Promise<void> {
    try {
      toolDefinitionState.isLoading = true;
      toolDefinitionState.error = null;
      toolDefinitionState.tools = await toolApi.listToolDefinitions();
      toolDefinitionState.loaded = true;
    } catch (error) {
      toolDefinitionState.error = message(error, "加载自定义工具失败");
      throw error;
    } finally {
      toolDefinitionState.isLoading = false;
    }
  },

  /**
   * Load once per session for read-only consumers (the timeline). Never throws:
   * a transcript must render whether or not the tool catalog resolved.
   */
  async ensureLoaded(): Promise<void> {
    if (toolDefinitionState.loaded) return;
    loadInFlight ??= this.loadTools()
      .catch((error) => {
        console.error("Failed to load tool definitions:", error);
      })
      .finally(() => {
        loadInFlight = null;
      });
    return loadInFlight;
  },

  async createTool(
    request: CreateToolDefinitionRequest,
  ): Promise<ToolDefinition> {
    const created = await toolApi.createToolDefinition(request);
    toolDefinitionState.tools.push(created);
    return created;
  },

  async updateTool(
    toolId: string,
    request: UpdateToolDefinitionRequest,
  ): Promise<ToolDefinition> {
    const updated = await toolApi.updateToolDefinition(toolId, request);
    replace(updated);
    return updated;
  },

  async deleteTool(toolId: string): Promise<void> {
    await toolApi.deleteToolDefinition(toolId);
    toolDefinitionState.tools = toolDefinitionState.tools.filter(
      (tool) => tool.id !== toolId,
    );
  },
};

/**
 * The definition registered under `toolName`, or undefined for a built-in, an
 * MCP tool, or a name from a definition that has since been deleted.
 *
 * A linear scan over a handful of rows, and reactive because it reads the
 * `$state` array: a `$derived` map would buy nothing at this size.
 */
export function findToolByName(toolName: string): ToolDefinition | undefined {
  return toolDefinitionState.tools.find((tool) => tool.name === toolName);
}
