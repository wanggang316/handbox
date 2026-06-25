/**
 * Unit tests for {@link buildQuickSessionRequest}.
 *
 * Pure TypeScript — the builder takes the resolver result + default-tools
 * setting as arguments and imports no `.svelte` module, so this suite runs
 * under the plain-Node Vitest environment. It is the strongest non-GUI
 * verification of the model = pick-or-default (VAL-COMMS-007/020),
 * enabledTools = defaultEnabledTools (VAL-COMMS-019) and the no-model
 * empty-state (VAL-COMMS-006) behaviors.
 */

import { describe, it, expect } from "vitest";
import { buildQuickSessionRequest } from "./createSessionRequest";
import { resolveQuickActionModel } from "./resolveModel";
import { BUILTIN_TOOL_IDS } from "../constants/builtinToolIds";
import type { ModelWithProvider } from "../types/provider";
import type { QuickActionModelResolution } from "./resolveModel";

/** Build a catalog item with only the fields the resolver matches on. */
function makeModel(id: string, providerId: string): ModelWithProvider {
  return {
    id,
    provider_id: providerId,
    name: `${providerId}/${id}`,
    support_tools: true,
    support_image: false,
    enabled: true,
    favorite: false,
    created_at: 0,
    updated_at: 0,
    providerName: providerId,
    providerType: providerId,
  };
}

const catalog: ModelWithProvider[] = [
  makeModel("gpt-4o", "openai-provider"),
  makeModel("claude-3", "anthropic-provider"),
];

const resolved: QuickActionModelResolution = {
  available: true,
  modelId: "gpt-4o",
  providerId: "openai-provider",
  model: catalog[0],
};

describe("buildQuickSessionRequest", () => {
  it("uses the resolved model/provider on the request (VAL-COMMS-007/020)", () => {
    const decision = buildQuickSessionRequest({
      resolution: resolved,
      defaultEnabledTools: ["read"],
      name: "Quick Action",
    });

    expect(decision.status).toBe("ready");
    if (decision.status === "ready") {
      expect(decision.request.modelId).toBe("gpt-4o");
      expect(decision.request.providerId).toBe("openai-provider");
      expect(decision.request.name).toBe("Quick Action");
    }
  });

  it("respects an in-panel pick over the configured default", () => {
    // An in-panel pick is just a different resolution fed in by the caller.
    const pick: QuickActionModelResolution = {
      available: true,
      modelId: "claude-3",
      providerId: "anthropic-provider",
      model: catalog[1],
    };

    const decision = buildQuickSessionRequest({
      resolution: pick,
      defaultEnabledTools: BUILTIN_TOOL_IDS,
      name: "Quick Action",
    });

    expect(decision.status).toBe("ready");
    if (decision.status === "ready") {
      expect(decision.request.modelId).toBe("claude-3");
      expect(decision.request.providerId).toBe("anthropic-provider");
    }
  });

  it("sets enabledTools to defaultEnabledTools (VAL-COMMS-019)", () => {
    const tools = ["read", "grep", "ls"];
    const decision = buildQuickSessionRequest({
      resolution: resolved,
      defaultEnabledTools: tools,
      name: "Quick Action",
    });

    expect(decision.status).toBe("ready");
    if (decision.status === "ready") {
      expect(decision.request.enabledTools).toEqual(tools);
      // A copy, not the caller's array (defensive against later mutation).
      expect(decision.request.enabledTools).not.toBe(tools);
    }
  });

  it("falls back to all 7 built-ins when settings have not loaded", () => {
    const fallbacks: (string[] | undefined | null)[] = [undefined, null, []];
    for (const fallback of fallbacks) {
      const decision = buildQuickSessionRequest({
        resolution: resolved,
        defaultEnabledTools: fallback,
        name: "Quick Action",
      });

      expect(decision.status).toBe("ready");
      if (decision.status === "ready") {
        expect(decision.request.enabledTools).toEqual(BUILTIN_TOOL_IDS);
      }
    }
  });

  it("never attaches a projectId (throwaway sandbox session)", () => {
    const decision = buildQuickSessionRequest({
      resolution: resolved,
      defaultEnabledTools: BUILTIN_TOOL_IDS,
      name: "Quick Action",
    });

    expect(decision.status).toBe("ready");
    if (decision.status === "ready") {
      expect(decision.request.projectId).toBeUndefined();
    }
  });

  it("surfaces the empty-state decision and creates no request (VAL-COMMS-006)", () => {
    const decision = buildQuickSessionRequest({
      resolution: { available: false, reason: "no-default" },
      defaultEnabledTools: BUILTIN_TOOL_IDS,
      name: "Quick Action",
    });

    expect(decision).toEqual({ status: "empty", reason: "no-default" });
  });

  it("delegates the empty-state decision to resolveQuickActionModel", () => {
    // Empty catalog → resolver returns empty-catalog → builder is empty too,
    // with the reason carried through verbatim (no session built).
    const decision = buildQuickSessionRequest({
      resolution: resolveQuickActionModel(
        { modelId: "gpt-4o", providerId: "openai-provider" },
        [],
      ),
      defaultEnabledTools: BUILTIN_TOOL_IDS,
      name: "Quick Action",
    });

    expect(decision).toEqual({ status: "empty", reason: "empty-catalog" });

    // Dangling default likewise propagates and builds nothing.
    const dangling = buildQuickSessionRequest({
      resolution: resolveQuickActionModel(
        { modelId: "removed", providerId: "openai-provider" },
        catalog,
      ),
      defaultEnabledTools: BUILTIN_TOOL_IDS,
      name: "Quick Action",
    });

    expect(dangling).toEqual({ status: "empty", reason: "dangling-default" });
  });
});
