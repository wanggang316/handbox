import { describe, it, expect } from "vitest";
import {
  groupSessionsByProject,
  partitionArchivedSessions,
  UNGROUPED_BUCKET_KEY,
  type AgentProjectBucket,
} from "./agentGrouping";
import type { AgentProject } from "../types/agentProject";
import type { AgentSession } from "../types/agentSession";

function project(id: string, createdAt = 0, name = id): AgentProject {
  return { id, path: `/p/${id}`, name, createdAt, updatedAt: createdAt };
}

function session(
  id: string,
  activity: number,
  opts: {
    projectId?: string;
    pinned?: boolean;
    archived?: boolean;
  } = {},
): AgentSession {
  return {
    id,
    name: id,
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

/** Compact signature of a bucket's members for order/shape assertions. */
function sessionIds(bucket: AgentProjectBucket): string[] {
  return bucket.sessions.map((s) => s.id);
}

describe("groupSessionsByProject", () => {
  it("returns no buckets for empty input", () => {
    expect(groupSessionsByProject([], [])).toEqual([]);
  });

  it("groups sessions under their project", () => {
    const buckets = groupSessionsByProject(
      [project("P")],
      [
        session("s-old", 100, { projectId: "P" }),
        session("s-new", 200, { projectId: "P" }),
      ],
    );
    expect(buckets).toHaveLength(1);
    expect(buckets[0].key).toBe("P");
    expect(buckets[0].project?.id).toBe("P");
    expect(sessionIds(buckets[0])).toEqual(["s-new", "s-old"]);
  });

  it("puts sessions with no/dangling project into Ungrouped, always last", () => {
    const buckets = groupSessionsByProject(
      [project("P")],
      [
        session("in-p", 100, { projectId: "P" }),
        session("noproj", 200), // never attached
        session("dangling", 300, { projectId: "ghost" }), // deleted project
      ],
    );
    // Ungrouped stays last even with newer activity (300).
    expect(buckets.map((b) => b.key)).toEqual(["P", UNGROUPED_BUCKET_KEY]);
    const ungrouped = buckets[1];
    expect(ungrouped.project).toBeNull();
    expect(sessionIds(ungrouped)).toEqual(["dangling", "noproj"]);
  });

  it("omits the Ungrouped bucket when every session has a project", () => {
    const buckets = groupSessionsByProject(
      [project("P")],
      [session("in-p", 100, { projectId: "P" })],
    );
    expect(buckets.map((b) => b.key)).toEqual(["P"]);
  });

  it("keeps a project with no sessions, ordered by its own createdAt", () => {
    const buckets = groupSessionsByProject(
      [project("old-empty", 50), project("busy", 10), project("new-empty", 500)],
      [session("s", 100, { projectId: "busy" })],
    );
    // new-empty(500) > busy(latest session 100) > old-empty(50).
    expect(buckets.map((b) => b.key)).toEqual([
      "new-empty",
      "busy",
      "old-empty",
    ]);
    expect(buckets[0].sessions).toEqual([]);
  });

  it("orders projects by latest activity desc", () => {
    const buckets = groupSessionsByProject(
      [project("A"), project("B")],
      [
        session("a1", 100, { projectId: "A" }),
        session("b1", 200, { projectId: "B" }),
      ],
    );
    expect(buckets.map((b) => b.key)).toEqual(["B", "A"]);
  });

  it("floats a pinned session above newer siblings", () => {
    const buckets = groupSessionsByProject(
      [project("P")],
      [
        session("newer", 300, { projectId: "P" }),
        session("pinned-old", 100, { projectId: "P", pinned: true }),
        session("older", 200, { projectId: "P" }),
      ],
    );
    expect(sessionIds(buckets[0])).toEqual([
      "pinned-old",
      "newer",
      "older",
    ]);
  });

  it("lifts the project holding the pin above a busier one", () => {
    const buckets = groupSessionsByProject(
      [project("A"), project("B")],
      [
        session("b1", 500, { projectId: "B" }),
        session("a-pinned", 100, { projectId: "A", pinned: true }),
      ],
    );
    expect(buckets.map((b) => b.key)).toEqual(["A", "B"]);
  });

  it("keeps Ungrouped last even when it holds the only pin", () => {
    const buckets = groupSessionsByProject(
      [project("P")],
      [
        session("in-p", 100, { projectId: "P" }),
        session("pinned-chat", 50, { pinned: true }),
      ],
    );
    expect(buckets.map((b) => b.key)).toEqual(["P", UNGROUPED_BUCKET_KEY]);
  });

  it("breaks activity ties by project name asc", () => {
    const buckets = groupSessionsByProject(
      [project("p2", 0, "beta"), project("p1", 0, "alpha")],
      [
        session("s2", 100, { projectId: "p2" }),
        session("s1", 100, { projectId: "p1" }),
      ],
    );
    expect(buckets.map((b) => b.project?.name)).toEqual(["alpha", "beta"]);
  });

  it("does not mutate its inputs", () => {
    const projects = [project("B", 0), project("A", 10)];
    const sessions = [
      session("s-old", 100, { projectId: "A" }),
      session("s-new", 200, { projectId: "A" }),
    ];
    groupSessionsByProject(projects, sessions);
    expect(projects.map((p) => p.id)).toEqual(["B", "A"]);
    expect(sessions.map((s) => s.id)).toEqual(["s-old", "s-new"]);
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
