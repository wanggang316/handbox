/**
 * Unit tests for the draft-session helpers.
 *
 * Pure TypeScript — the seed builder takes the definition and the model pair as
 * arguments and imports no `.svelte` module, so this suite runs under the
 * plain-Node Vitest environment. It pins the two properties the deferred-create
 * flow depends on: a draft id is never mistaken for a persisted one, and a
 * draft promotes into exactly the session the backend would have instantiated.
 */

import { describe, it, expect } from "vitest";
import {
  buildDraftSession,
  isDraftSessionId,
  newDraftSessionId,
} from "./draftSession";
import type { Agent } from "../types/agent";

function makeDefinition(overrides: Partial<Agent> = {}): Agent {
  return {
    id: "builtin-coding",
    name: "Coding",
    builtin: true,
    builtinTools: ["read", "write", "bash"],
    mcpServers: [],
    skills: [],
    starters: [],
    workingDirMode: "required",
    toolExecutionMode: "manual",
    thinkingLevel: "high",
    systemPrompt: "You are helpful.",
    temperature: 0.5,
    maxTokens: 1024,
    createdAt: 0,
    updatedAt: 0,
    ...overrides,
  };
}

describe("isDraftSessionId", () => {
  it("accepts a minted draft id and rejects a persisted uuid", () => {
    expect(isDraftSessionId(newDraftSessionId())).toBe(true);
    expect(isDraftSessionId("6d1f0a2c-6e0e-4a1e-9a1e-2b3c4d5e6f70")).toBe(false);
  });

  it("rejects the absent id rather than throwing", () => {
    expect(isDraftSessionId(undefined)).toBe(false);
    expect(isDraftSessionId(null)).toBe(false);
    expect(isDraftSessionId("")).toBe(false);
  });

  it("mints a distinct id per draft, so composers do not share state", () => {
    expect(newDraftSessionId()).not.toBe(newDraftSessionId());
  });
});

describe("buildDraftSession", () => {
  it("snapshots the definition's capabilities and defaults", () => {
    const draft = buildDraftSession({
      id: "draft:1",
      definitionId: "builtin-coding",
      definition: makeDefinition(),
      modelId: "gpt-4o",
      providerId: "openai-provider",
      workingDir: "/tmp/project",
      now: 1_700_000_000_000,
    });

    expect(draft.id).toBe("draft:1");
    expect(draft.agentDefinitionId).toBe("builtin-coding");
    expect(draft.name).toBe("Coding");
    expect(draft.modelId).toBe("gpt-4o");
    expect(draft.providerId).toBe("openai-provider");
    expect(draft.enabledTools).toEqual(["read", "write", "bash"]);
    expect(draft.thinkingLevel).toBe("high");
    expect(draft.toolExecutionMode).toBe("manual");
    expect(draft.workingDir).toBe("/tmp/project");
    // Nothing has been sent yet: the empty-state layout keys off this.
    expect(draft.messageCount).toBe(0);
  });

  it("drops the working dir for a definition that takes none", () => {
    const draft = buildDraftSession({
      id: "draft:1",
      definitionId: "builtin-chat",
      definition: makeDefinition({
        id: "builtin-chat",
        name: "通用对话",
        workingDirMode: "none",
        builtinTools: [],
      }),
      workingDir: "/tmp/project",
      now: 0,
    });

    // Matches the backend's working-dir arbitration, so promoting the draft
    // cannot produce a session create would have rejected.
    expect(draft.workingDir).toBeUndefined();
    expect(draft.enabledTools).toEqual([]);
  });

  it("copies the definition's arrays instead of aliasing them", () => {
    const definition = makeDefinition();
    const draft = buildDraftSession({
      id: "draft:1",
      definitionId: definition.id!,
      definition,
      now: 0,
    });

    draft.enabledTools.push("edit");

    expect(definition.builtinTools).toEqual(["read", "write", "bash"]);
  });

  it("keeps the id and the model pair while the definition is still loading", () => {
    const draft = buildDraftSession({
      id: "draft:1",
      definitionId: "builtin-chat",
      definition: null,
      modelId: "gpt-4o",
      providerId: "openai-provider",
      now: 0,
    });

    expect(draft.id).toBe("draft:1");
    expect(draft.modelId).toBe("gpt-4o");
    expect(draft.enabledTools).toEqual([]);
    expect(draft.name).toBe("");
  });
});
