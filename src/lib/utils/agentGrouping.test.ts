import { describe, it, expect } from "vitest";
import {
  groupSessionsByAgent,
  partitionArchivedSessions,
  CHATS_BUCKET_KEY,
  type AgentSessionBucket,
} from "./agentGrouping";
import type { Agent } from "../types/agent";
import type { AgentProject } from "../types/agentProject";
import type { AgentSession } from "../types/agentSession";

function agent(id: string, name = id): Agent {
  return {
    id,
    name,
    createdAt: 0,
    updatedAt: 0,
    mcpServers: [],
    skills: [],
    builtin: false,
    builtinTools: [],
    starters: [],
  };
}

function project(id: string): AgentProject {
  return { id, path: `/p/${id}`, name: id, createdAt: 0, updatedAt: 0 };
}

function session(
  id: string,
  activity: number,
  opts: {
    agentId?: string;
    projectId?: string;
    pinned?: boolean;
    archived?: boolean;
  } = {},
): AgentSession {
  return {
    id,
    name: id,
    agentDefinitionId: opts.agentId,
    projectId: opts.projectId,
    enabledTools: [],
    mcpServers: [],
    messageCount: 0,
    lastMessageAt: activity,
    pinned: opts.pinned ?? false,
    archived: opts.archived ?? false,
    createdAt: activity,
    updatedAt: activity,
  };
}

/** Compact signature of a bucket's children for order/shape assertions. */
function childKeys(bucket: AgentSessionBucket): string[] {
  return bucket.children.map((c) =>
    c.kind === "project"
      ? `project:${c.project.id}[${c.sessions.map((s) => s.id).join(",")}]`
      : `session:${c.session.id}`,
  );
}

describe("groupSessionsByAgent", () => {
  it("returns no buckets for empty input", () => {
    expect(groupSessionsByAgent([], [], [])).toEqual([]);
  });

  it("groups under the source agent; projects nest, loose sessions stay direct", () => {
    const buckets = groupSessionsByAgent(
      [agent("A")],
      [project("P")],
      [
        session("s-proj", 100, { agentId: "A", projectId: "P" }),
        session("s-loose", 200, { agentId: "A" }),
      ],
    );
    expect(buckets).toHaveLength(1);
    expect(buckets[0].key).toBe("A");
    expect(buckets[0].agent?.id).toBe("A");
    // Activity desc: loose(200) precedes project(100).
    expect(childKeys(buckets[0])).toEqual([
      "session:s-loose",
      "project:P[s-proj]",
    ]);
  });

  it("puts sessions with no/dangling agent into the Chats bucket, always last", () => {
    const buckets = groupSessionsByAgent(
      [agent("A")],
      [],
      [
        session("a1", 100, { agentId: "A" }),
        session("noagent", 200), // no source agent
        session("dangling", 300, { agentId: "ghost" }), // dangling reference
      ],
    );
    // Chats stays last even with newer activity (300).
    expect(buckets.map((b) => b.key)).toEqual(["A", CHATS_BUCKET_KEY]);
    const chats = buckets[1];
    expect(chats.agent).toBeNull();
    expect(childKeys(chats)).toEqual(["session:dangling", "session:noagent"]);
  });

  it("orders agent buckets by latest activity desc", () => {
    const buckets = groupSessionsByAgent(
      [agent("A"), agent("B")],
      [],
      [
        session("a1", 100, { agentId: "A" }),
        session("b1", 200, { agentId: "B" }),
      ],
    );
    expect(buckets.map((b) => b.key)).toEqual(["B", "A"]);
  });

  it("interleaves project groups and loose sessions by activity; project uses its latest member", () => {
    const buckets = groupSessionsByAgent(
      [agent("A")],
      [project("P")],
      [
        session("loose-hi", 350, { agentId: "A" }),
        session("p-old", 100, { agentId: "A", projectId: "P" }),
        session("p-new", 300, { agentId: "A", projectId: "P" }),
        session("loose-mid", 250, { agentId: "A" }),
      ],
    );
    expect(childKeys(buckets[0])).toEqual([
      "session:loose-hi", // 350
      "project:P[p-new,p-old]", // latest 300, members desc
      "session:loose-mid", // 250
    ]);
  });

  it("omits agents that have no sessions", () => {
    const buckets = groupSessionsByAgent(
      [agent("A"), agent("B")],
      [],
      [session("a1", 100, { agentId: "A" })],
    );
    expect(buckets.map((b) => b.key)).toEqual(["A"]);
  });

  it("floats a pinned session above newer siblings", () => {
    const buckets = groupSessionsByAgent(
      [agent("A")],
      [],
      [
        session("newer", 300, { agentId: "A" }),
        session("pinned-old", 100, { agentId: "A", pinned: true }),
        session("older", 200, { agentId: "A" }),
      ],
    );
    expect(childKeys(buckets[0])).toEqual([
      "session:pinned-old",
      "session:newer",
      "session:older",
    ]);
  });

  it("lifts the project group and the agent bucket holding the pin", () => {
    const buckets = groupSessionsByAgent(
      [agent("A"), agent("B")],
      [project("P")],
      [
        // Bucket B is newer, and inside A the loose session is newer than the
        // project — both must yield to the pin.
        session("b1", 500, { agentId: "B" }),
        session("a-loose", 400, { agentId: "A" }),
        session("a-p-pinned", 100, {
          agentId: "A",
          projectId: "P",
          pinned: true,
        }),
      ],
    );
    expect(buckets.map((b) => b.key)).toEqual(["A", "B"]);
    expect(childKeys(buckets[0])).toEqual([
      "project:P[a-p-pinned]",
      "session:a-loose",
    ]);
  });

  it("keeps Chats last even when it holds the only pin", () => {
    const buckets = groupSessionsByAgent(
      [agent("A")],
      [],
      [
        session("a1", 100, { agentId: "A" }),
        session("pinned-chat", 50, { pinned: true }),
      ],
    );
    expect(buckets.map((b) => b.key)).toEqual(["A", CHATS_BUCKET_KEY]);
  });
});

describe("partitionArchivedSessions", () => {
  it("splits archived out and orders them by activity desc, ignoring the pin", () => {
    const { active, archived } = partitionArchivedSessions([
      session("live", 100),
      session("arch-old", 10, { archived: true, pinned: true }),
      session("arch-new", 20, { archived: true }),
    ]);
    expect(active.map((s) => s.id)).toEqual(["live"]);
    expect(archived.map((s) => s.id)).toEqual(["arch-new", "arch-old"]);
  });

  it("returns empty halves rather than undefined", () => {
    expect(partitionArchivedSessions([])).toEqual({ active: [], archived: [] });
  });
});
