/**
 * Unit tests for {@link resolveQuickActionModel}.
 *
 * Pure TypeScript — the resolver takes the catalog + settings as arguments and
 * imports no `.svelte` module, so this suite runs under the plain-Node Vitest
 * environment.
 */

import { describe, it, expect } from "vitest";
import { resolveQuickActionModel } from "./resolveModel";
import type { ModelWithProvider } from "../types/provider";
import type { QuickActionSettings } from "../types/settings";

/** Build a catalog item with only the fields the resolver matches on. */
function makeModel(
  id: string,
  providerId: string,
): ModelWithProvider {
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

describe("resolveQuickActionModel", () => {
  it("resolves a configured default that exists in the catalog", () => {
    const settings: QuickActionSettings = {
      modelId: "gpt-4o",
      providerId: "openai-provider",
    };

    const result = resolveQuickActionModel(settings, catalog);

    expect(result.available).toBe(true);
    if (result.available) {
      expect(result.modelId).toBe("gpt-4o");
      expect(result.providerId).toBe("openai-provider");
      expect(result.model).toBe(catalog[0]);
    }
  });

  it("reports no-default when there is no configured default", () => {
    expect(resolveQuickActionModel(undefined, catalog)).toEqual({
      available: false,
      reason: "no-default",
    });

    expect(resolveQuickActionModel({}, catalog)).toEqual({
      available: false,
      reason: "no-default",
    });

    // A model id without a provider id is not a complete default.
    expect(
      resolveQuickActionModel({ modelId: "gpt-4o" }, catalog),
    ).toEqual({ available: false, reason: "no-default" });
  });

  it("reports dangling-default when the configured default is not in the catalog", () => {
    const settings: QuickActionSettings = {
      modelId: "removed-model",
      providerId: "openai-provider",
    };

    expect(resolveQuickActionModel(settings, catalog)).toEqual({
      available: false,
      reason: "dangling-default",
    });

    // Right model id but wrong provider id is also dangling.
    expect(
      resolveQuickActionModel(
        { modelId: "gpt-4o", providerId: "anthropic-provider" },
        catalog,
      ),
    ).toEqual({ available: false, reason: "dangling-default" });
  });

  it("reports empty-catalog when no enabled provider+model exists", () => {
    const settings: QuickActionSettings = {
      modelId: "gpt-4o",
      providerId: "openai-provider",
    };

    expect(resolveQuickActionModel(settings, [])).toEqual({
      available: false,
      reason: "empty-catalog",
    });

    // Empty catalog wins even when no default is configured.
    expect(resolveQuickActionModel(undefined, [])).toEqual({
      available: false,
      reason: "empty-catalog",
    });
  });
});
