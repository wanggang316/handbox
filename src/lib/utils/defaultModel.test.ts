/**
 * Unit tests for the shared default-model helpers.
 *
 * The resolver's empty/dangling/resolved matrix is exercised in depth by
 * `quickaction/resolveModel.test.ts` (which now runs through this module); the
 * cases here cover the pieces the agent-session path adds: a null preference
 * and the override-merging rules of {@link applyDefaultModel}.
 */

import { describe, it, expect } from "vitest";
import { applyDefaultModel, resolveDefaultModel } from "./defaultModel";
import type { ModelWithProvider } from "../types/provider";

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

describe("resolveDefaultModel", () => {
  it("resolves a stored pair against the catalog", () => {
    const result = resolveDefaultModel(
      { modelId: "claude-3", providerId: "anthropic-provider" },
      catalog,
    );

    expect(result.available).toBe(true);
    if (result.available) {
      expect(result.model).toBe(catalog[1]);
    }
  });

  it("treats a null preference (settings unloaded) as no-default", () => {
    expect(resolveDefaultModel(null, catalog)).toEqual({
      available: false,
      reason: "no-default",
    });
  });

  it("treats a null half of the pair as no-default", () => {
    expect(
      resolveDefaultModel({ modelId: "gpt-4o", providerId: null }, catalog),
    ).toEqual({ available: false, reason: "no-default" });
  });
});

describe("applyDefaultModel", () => {
  const resolved = resolveDefaultModel(
    { modelId: "gpt-4o", providerId: "openai-provider" },
    catalog,
  );
  const dangling = resolveDefaultModel(
    { modelId: "removed", providerId: "openai-provider" },
    catalog,
  );

  it("stamps the resolved default onto overrides that pin no model", () => {
    expect(applyDefaultModel({ projectId: "p1" }, resolved)).toEqual({
      projectId: "p1",
      modelId: "gpt-4o",
      providerId: "openai-provider",
    });
  });

  it("stamps the default when there are no overrides at all", () => {
    expect(applyDefaultModel(undefined, resolved)).toEqual({
      modelId: "gpt-4o",
      providerId: "openai-provider",
    });
  });

  it("leaves an explicitly pinned model untouched", () => {
    const pinned = {
      modelId: "claude-3",
      providerId: "anthropic-provider",
    };

    expect(applyDefaultModel(pinned, resolved)).toBe(pinned);
  });

  it("completes a half-set pair from the default (both halves are required)", () => {
    expect(applyDefaultModel({ modelId: "claude-3" }, resolved)).toEqual({
      modelId: "gpt-4o",
      providerId: "openai-provider",
    });
  });

  it("leaves overrides alone when the default cannot be resolved", () => {
    const overrides = { projectId: "p1" };

    expect(applyDefaultModel(overrides, dangling)).toBe(overrides);
    expect(applyDefaultModel(undefined, dangling)).toBeUndefined();
  });
});
