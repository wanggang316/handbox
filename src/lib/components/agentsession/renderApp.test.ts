import { describe, expect, it } from "vitest";
import type { AgentMessage } from "$lib/types/agentSession";
import { parseRenderAppArgs, reconstructAppArtifact } from "./renderApp";

describe("parseRenderAppArgs", () => {
  it("accepts an object carrier", () => {
    expect(
      parseRenderAppArgs({
        command: "create",
        title: "Demo",
        content: "<html></html>",
      }),
    ).toEqual({ command: "create", title: "Demo", content: "<html></html>" });
  });

  it("accepts a JSON-string carrier", () => {
    expect(
      parseRenderAppArgs('{"command":"update","content":"<html>v2</html>"}'),
    ).toEqual({
      command: "update",
      title: undefined,
      content: "<html>v2</html>",
    });
  });

  it("rejects a truncated JSON string (streaming partial)", () => {
    expect(
      parseRenderAppArgs('{"command":"create","content":"<htm'),
    ).toBeNull();
  });

  it("rejects an unknown command", () => {
    expect(
      parseRenderAppArgs({ command: "delete", content: "<html></html>" }),
    ).toBeNull();
  });

  it("rejects missing or empty content", () => {
    expect(parseRenderAppArgs({ command: "create", title: "T" })).toBeNull();
    expect(
      parseRenderAppArgs({ command: "create", title: "T", content: "   " }),
    ).toBeNull();
  });

  it("normalises a blank title to undefined", () => {
    expect(
      parseRenderAppArgs({
        command: "update",
        title: "  ",
        content: "<p>x</p>",
      }),
    ).toEqual({ command: "update", title: undefined, content: "<p>x</p>" });
  });

  it("rejects non-object carriers", () => {
    expect(parseRenderAppArgs(null)).toBeNull();
    expect(parseRenderAppArgs(42)).toBeNull();
    expect(parseRenderAppArgs(["create"])).toBeNull();
  });
});

// --- reconstruction helpers ---

function assistantWithToolCalls(
  calls: Array<{ id: string; args: unknown }>,
): AgentMessage {
  return {
    role: "assistant",
    content: calls.map(({ id, args }) => ({
      type: "toolcall" as const,
      id,
      name: "render_app",
      arguments: args,
    })),
    api: "test",
    provider: "test",
    model: "test",
    usage: {
      input: 0,
      output: 0,
      cacheRead: 0,
      cacheWrite: 0,
      totalTokens: 0,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
    },
    stopReason: "toolUse",
    timestamp: 0,
  } as AgentMessage;
}

function toolResult(toolCallId: string, isError: boolean): AgentMessage {
  return {
    role: "toolResult",
    toolCallId,
    toolName: "render_app",
    content: [{ type: "text", text: isError ? "bad" : "ok" }],
    isError,
    timestamp: 0,
  } as AgentMessage;
}

describe("reconstructAppArtifact", () => {
  it("returns null when no render_app blocks exist", () => {
    expect(reconstructAppArtifact([], {})).toBeNull();
  });

  it("folds create then update, keeping the title", () => {
    const messages = [
      assistantWithToolCalls([
        { id: "t1", args: { command: "create", title: "Game", content: "v1" } },
      ]),
      toolResult("t1", false),
      assistantWithToolCalls([
        { id: "t2", args: { command: "update", content: "v2" } },
      ]),
      toolResult("t2", false),
    ];
    expect(reconstructAppArtifact(messages, {})).toEqual({
      title: "Game",
      content: "v2",
      toolCallId: "t2",
    });
  });

  it("lets update replace the title when provided", () => {
    const messages = [
      assistantWithToolCalls([
        { id: "t1", args: { command: "create", title: "Old", content: "v1" } },
        { id: "t2", args: { command: "update", title: "New", content: "v2" } },
      ]),
    ];
    expect(reconstructAppArtifact(messages, {})).toEqual({
      title: "New",
      content: "v2",
      toolCallId: "t2",
    });
  });

  it("skips errored calls via committed toolResult pairing", () => {
    const messages = [
      assistantWithToolCalls([
        {
          id: "t1",
          args: { command: "create", title: "App", content: "good" },
        },
      ]),
      toolResult("t1", false),
      assistantWithToolCalls([
        { id: "t2", args: { command: "update", content: "rejected" } },
      ]),
      toolResult("t2", true),
    ];
    expect(reconstructAppArtifact(messages, {})).toEqual({
      title: "App",
      content: "good",
      toolCallId: "t1",
    });
  });

  it("skips errored calls via live status, preferring live args", () => {
    const messages = [
      assistantWithToolCalls([
        { id: "t1", args: { command: "create", title: "App", content: "v1" } },
        { id: "t2", args: "" },
      ]),
    ];
    const live = {
      t1: {
        status: "completed",
        args: { command: "create", title: "App", content: "live-v1" },
      },
      t2: { status: "error", args: { command: "update", content: "bad" } },
    };
    expect(reconstructAppArtifact(messages, live)).toEqual({
      title: "App",
      content: "live-v1",
      toolCallId: "t1",
    });
  });

  it("still-executing calls with parseable args contribute (live preview)", () => {
    const messages = [
      assistantWithToolCalls([
        { id: "t1", args: { command: "create", title: "App", content: "v1" } },
      ]),
    ];
    const live = { t1: { status: "executing" } };
    expect(reconstructAppArtifact(messages, live)).toEqual({
      title: "App",
      content: "v1",
      toolCallId: "t1",
    });
  });

  it("skips unparseable args without dropping earlier state", () => {
    const messages = [
      assistantWithToolCalls([
        { id: "t1", args: { command: "create", title: "App", content: "v1" } },
        { id: "t2", args: '{"command":"update","content":"<par' },
      ]),
    ];
    expect(reconstructAppArtifact(messages, {})).toEqual({
      title: "App",
      content: "v1",
      toolCallId: "t1",
    });
  });

  it("tolerates a leading update without create (empty title)", () => {
    const messages = [
      assistantWithToolCalls([
        { id: "t1", args: { command: "update", content: "orphan" } },
      ]),
    ];
    expect(reconstructAppArtifact(messages, {})).toEqual({
      title: "",
      content: "orphan",
      toolCallId: "t1",
    });
  });
});
