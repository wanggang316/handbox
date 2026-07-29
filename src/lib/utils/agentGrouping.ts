/**
 * Pure selectors grouping agent sessions into Agent → Project → Session.
 *
 * Hierarchy: sessions are bucketed by source agent (session.agentDefinitionId);
 * inside a bucket, sessions attached to an existing project nest under a
 * project subgroup while the rest stay direct children. Sessions with no or
 * dangling agent go into a "Chats" bucket that always sorts last.
 *
 * Sort contract:
 *  - Session activity key = coalesce(lastMessageAt, createdAt) — never
 *    updatedAt (rename/config writes bump updatedAt but are not "activity").
 *  - Bucket children (project subgroups interleaved with loose sessions) sort
 *    by activity key desc; a project subgroup's key is its latest session's.
 *  - Agent buckets sort by latest activity desc, ties broken by agent name
 *    asc; the "Chats" bucket always comes last.
 *  - Pure: inputs are not mutated; output order is independent of input order.
 */

import type { Timestamp } from "../types";
import type { Agent } from "../types/agent";
import type { AgentSession } from "../types/agentSession";
import type { AgentProject } from "../types/agentProject";

/** Reserved key for the "Chats" bucket (never collides with a UUID). */
export const CHATS_BUCKET_KEY = "__chats__";

export interface AgentProjectGroup {
  project: AgentProject;
  sessions: AgentSession[];
}

export type AgentBucketChild =
  | { kind: "project"; project: AgentProject; sessions: AgentSession[] }
  | { kind: "session"; session: AgentSession };

export interface AgentSessionBucket {
  /** Stable key for collapse state / keyed each: agent.id or `CHATS_BUCKET_KEY`. */
  key: string;
  /** Owning agent; null means the "Chats" bucket. */
  agent: Agent | null;
  /** Children sorted by activity key desc (project subgroups interleaved with loose sessions). */
  children: AgentBucketChild[];
}

/**
 * Activity key = coalesce(lastMessageAt, createdAt). Deliberately not
 * updatedAt: rename/config writes are not "activity" and must not affect order.
 */
export function sessionActivityKey(session: AgentSession): Timestamp {
  return session.lastMessageAt ?? session.createdAt;
}

/** Activity desc; ties by createdAt desc then id asc for a deterministic total order. */
function compareSessionsByActivityDesc(
  a: AgentSession,
  b: AgentSession,
): number {
  const activityDelta = sessionActivityKey(b) - sessionActivityKey(a);
  if (activityDelta !== 0) return activityDelta;
  const createdDelta = b.createdAt - a.createdAt;
  if (createdDelta !== 0) return createdDelta;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/** Session: its own key; project subgroup: latest member (sessions pre-sorted desc). */
function childActivityKey(child: AgentBucketChild): Timestamp {
  return child.kind === "session"
    ? sessionActivityKey(child.session)
    : sessionActivityKey(child.sessions[0]);
}

/** Stable cross-kind tie-break key (project prefix sorts before session prefix). */
function childTiebreakKey(child: AgentBucketChild): string {
  return child.kind === "project"
    ? `0:${child.project.id}`
    : `1:${child.session.id}`;
}

function compareChildrenByActivityDesc(
  a: AgentBucketChild,
  b: AgentBucketChild,
): number {
  const delta = childActivityKey(b) - childActivityKey(a);
  if (delta !== 0) return delta;
  const ka = childTiebreakKey(a);
  const kb = childTiebreakKey(b);
  return ka < kb ? -1 : ka > kb ? 1 : 0;
}

/**
 * Split a bucket's sessions into project subgroups and loose sessions; a
 * dangling projectId falls back to loose. Everything sorts by activity desc.
 */
function buildBucketChildren(
  bucketSessions: AgentSession[],
  projectById: Map<string, AgentProject>,
): AgentBucketChild[] {
  const byProject = new Map<string, AgentSession[]>();
  const loose: AgentSession[] = [];
  for (const session of bucketSessions) {
    const project = session.projectId
      ? projectById.get(session.projectId)
      : undefined;
    if (project) {
      const list = byProject.get(project.id);
      if (list) list.push(session);
      else byProject.set(project.id, [session]);
    } else {
      loose.push(session);
    }
  }

  const children: AgentBucketChild[] = [];
  for (const [projectId, list] of byProject) {
    children.push({
      kind: "project",
      project: projectById.get(projectId)!,
      sessions: list.sort(compareSessionsByActivityDesc),
    });
  }
  for (const session of loose) {
    children.push({ kind: "session", session });
  }
  children.sort(compareChildrenByActivityDesc);
  return children;
}

/**
 * Group sessions into sorted Agent → Project → Session buckets.
 *
 *  - Only agents with at least one session get a bucket.
 *  - Sessions with an empty or dangling agentDefinitionId go to "Chats".
 *  - Buckets sort by latest activity desc, ties by agent name asc; the
 *    "Chats" bucket always comes last.
 *
 * Pure: inputs are not mutated; output order is independent of input order.
 */
export function groupSessionsByAgent(
  agents: Agent[],
  projects: AgentProject[],
  sessions: AgentSession[],
): AgentSessionBucket[] {
  const agentById = new Map<string, Agent>();
  // Only persisted agents (with an id) can serve as an ownership key.
  for (const agent of agents) {
    if (agent.id) agentById.set(agent.id, agent);
  }
  const projectById = new Map<string, AgentProject>();
  for (const project of projects) projectById.set(project.id, project);

  // Bucket by resolved agent; missing/dangling agents fall into CHATS_BUCKET_KEY.
  const sessionsByBucket = new Map<string, AgentSession[]>();
  for (const session of sessions) {
    const agentId =
      session.agentDefinitionId && agentById.has(session.agentDefinitionId)
        ? session.agentDefinitionId
        : CHATS_BUCKET_KEY;
    const list = sessionsByBucket.get(agentId);
    if (list) list.push(session);
    else sessionsByBucket.set(agentId, [session]);
  }

  const agentBuckets: AgentSessionBucket[] = [];
  let chatsBucket: AgentSessionBucket | null = null;
  for (const [key, bucketSessions] of sessionsByBucket) {
    const bucket: AgentSessionBucket = {
      key,
      agent: key === CHATS_BUCKET_KEY ? null : (agentById.get(key) ?? null),
      children: buildBucketChildren(bucketSessions, projectById),
    };
    if (key === CHATS_BUCKET_KEY) chatsBucket = bucket;
    else agentBuckets.push(bucket);
  }

  // children are sorted desc, so the first child carries the bucket's latest activity.
  const bucketSortKey = (bucket: AgentSessionBucket): Timestamp =>
    bucket.children.length ? childActivityKey(bucket.children[0]) : 0;

  agentBuckets.sort((a, b) => {
    const delta = bucketSortKey(b) - bucketSortKey(a);
    if (delta !== 0) return delta;
    const nameA = a.agent?.name ?? "";
    const nameB = b.agent?.name ?? "";
    if (nameA !== nameB) return nameA < nameB ? -1 : 1;
    return a.key < b.key ? -1 : a.key > b.key ? 1 : 0;
  });

  return chatsBucket ? [...agentBuckets, chatsBucket] : agentBuckets;
}
