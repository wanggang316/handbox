/**
 * Unit tests for the tool-call presentation helpers.
 *
 * Pure TypeScript — no `.svelte` imports — so this suite runs under the
 * plain-Node Vitest environment. It covers the MCP name split (including the
 * near-misses that must NOT be treated as namespaced) and the argument summary
 * across the built-in tool schemas plus unknown MCP shapes.
 */

import { describe, it, expect } from "vitest";
import { parseMcpToolName, toolArgSummary, toolArgsRecord } from "./toolCall";

describe("parseMcpToolName", () => {
  it("splits a namespaced MCP tool into server id and tool", () => {
    expect(parseMcpToolName("mcp__srv-1__create_issue")).toEqual({
      serverId: "srv-1",
      tool: "create_issue",
    });
  });

  it("keeps separators inside the tool name", () => {
    expect(parseMcpToolName("mcp__srv__a__b")).toEqual({
      serverId: "srv",
      tool: "a__b",
    });
  });

  it("returns null for built-ins and malformed namespaces", () => {
    expect(parseMcpToolName("bash")).toBeNull();
    expect(parseMcpToolName("mcp__srv")).toBeNull();
    expect(parseMcpToolName("mcp____tool")).toBeNull();
    expect(parseMcpToolName("mcp__srv__")).toBeNull();
    expect(parseMcpToolName("")).toBeNull();
  });
});

describe("toolArgSummary", () => {
  it("picks the key that identifies the call, not the bulkiest one", () => {
    expect(toolArgSummary({ path: "src/main.ts", content: "a".repeat(50) })).toBe(
      "src/main.ts",
    );
    expect(toolArgSummary({ file_path: "src/app.ts", new_string: "x" })).toBe(
      "src/app.ts",
    );
    expect(toolArgSummary({ pattern: "TODO", path: "src" })).toBe("TODO");
    expect(toolArgSummary({ query: "svelte 5 runes" })).toBe("svelte 5 runes");
  });

  it("collapses a multi-line command to one line", () => {
    expect(toolArgSummary({ command: "cd src &&\n  ls -la\n" })).toBe(
      "cd src && ls -la",
    );
  });

  it("caps a long value with an ellipsis", () => {
    const summary = toolArgSummary({ command: "x".repeat(500) });
    expect(summary).toHaveLength(201);
    expect(summary.endsWith("…")).toBe(true);
  });

  it("parses arguments delivered as a JSON string", () => {
    expect(toolArgSummary('{"path":"README.md"}')).toBe("README.md");
  });

  it("falls back to the first string value for an unknown MCP shape", () => {
    expect(toolArgSummary({ limit: 10, repo: "handbox", flag: true })).toBe(
      "handbox",
    );
  });

  it("is empty when nothing string-shaped is worth showing", () => {
    expect(toolArgSummary({ questions: [{ header: "a" }] })).toBe("");
    expect(toolArgSummary({ path: "   " })).toBe("");
    expect(toolArgSummary(undefined)).toBe("");
    expect(toolArgSummary(null)).toBe("");
    expect(toolArgSummary([1, 2])).toBe("");
  });

  it("shows a non-JSON string argument as-is", () => {
    expect(toolArgSummary("just text")).toBe("just text");
  });
});

describe("toolArgsRecord", () => {
  it("passes an object through and parses the JSON-string carrier", () => {
    expect(toolArgsRecord({ city: "HZ" })).toEqual({ city: "HZ" });
    expect(toolArgsRecord('{"city":"HZ"}')).toEqual({ city: "HZ" });
  });

  /// The state model a view renders against must never be null: an empty card
  /// is recoverable, a missing state model is not.
  it("falls back to an empty object for anything not object-shaped", () => {
    expect(toolArgsRecord(undefined)).toEqual({});
    expect(toolArgsRecord(null)).toEqual({});
    expect(toolArgsRecord("not json")).toEqual({});
    expect(toolArgsRecord([1, 2])).toEqual({});
  });
});
