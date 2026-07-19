import { describe, it, expect } from "vitest";
import {
  groupSessionsByAgent,
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
  opts: { agentId?: string; projectId?: string } = {},
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
    createdAt: activity,
    updatedAt: activity,
  };
}

/** 桶内子节点的紧凑签名，便于断言顺序与形态。 */
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
    // 子节点按活动键降序：loose(200) 在 project(100) 之前。
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
        session("noagent", 200), // 无来源 Agent
        session("dangling", 300, { agentId: "ghost" }), // 悬挂引用
      ],
    );
    // Chats 桶活动更新（300）也恒定垫底。
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
      "project:P[p-new,p-old]", // latest 300, 组内降序
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
});
