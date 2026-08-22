/**
 * Pure selectors grouping agent sessions into Project → Session.
 *
 * Hierarchy: the top level is the project (`session.projectId`); every project
 * is a bucket, and sessions with no or dangling project fall into a trailing
 * ungrouped bucket. The source agent is not a level — it rides along on the
 * session row as an icon — so a project shows all of its work in one place
 * regardless of which agent produced it.
 *
 * Sort contract:
 *  - Session activity key = coalesce(lastMessageAt, createdAt) — never
 *    updatedAt (rename/config writes bump updatedAt but are not "activity").
 *  - Pinned outranks activity at every level: a pinned session sorts ahead of
 *    its unpinned siblings, and a project holding one sorts ahead of projects
 *    that do not. Without that lift, pinning a session inside a low-activity
 *    project would leave the pin invisible.
 *  - A project's sort key is its latest session's activity, falling back to the
 *    project's own createdAt while it holds none — a project created just now
 *    must land at the top, not below every populated one.
 *  - Empty projects are kept: a project the user just created has to be visible
 *    (and clickable) before it holds anything.
 *  - The ungrouped bucket always comes last — even when it holds a pin, so the
 *    hierarchy's shape never depends on pin state — and is absent when empty.
 *  - Archived sessions are the caller's business: pass only the sessions that
 *    belong in the tree (see [`partitionArchivedSessions`]).
 *  - Pure: inputs are not mutated; output order is independent of input order.
 */

import type { Timestamp } from "../types";
import type { AgentSession } from "../types/agentSession";
import type { AgentProject } from "../types/agentProject";

/** Reserved key for the ungrouped bucket (never collides with a UUID). */
export const UNGROUPED_BUCKET_KEY = "__ungrouped__";

export interface AgentProjectBucket {
  /** Stable key for collapse state / keyed each: project.id or `UNGROUPED_BUCKET_KEY`. */
  key: string;
  /** Owning project; null means the ungrouped bucket. */
  project: AgentProject | null;
  /** Members, pinned first then activity desc. */
  sessions: AgentSession[];
}

/**
 * Activity key = coalesce(lastMessageAt, createdAt). Deliberately not
 * updatedAt: rename/config writes are not "activity" and must not affect order.
 */
export function sessionActivityKey(session: AgentSession): Timestamp {
  return session.lastMessageAt ?? session.createdAt;
}

/** Activity desc; ties by createdAt desc then id asc for a deterministic total order. */
export function compareSessionsByActivityDesc(
  a: AgentSession,
  b: AgentSession,
): number {
  const activityDelta = sessionActivityKey(b) - sessionActivityKey(a);
  if (activityDelta !== 0) return activityDelta;
  const createdDelta = b.createdAt - a.createdAt;
  if (createdDelta !== 0) return createdDelta;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/** Sidebar order: pinned first, then activity desc. */
function compareSessionsForTree(a: AgentSession, b: AgentSession): number {
  if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
  return compareSessionsByActivityDesc(a, b);
}

/**
 * Splits sessions into the ones the sidebar tree renders and the ones the
 * "Archived" group does, the latter already in activity-desc order. Archived
 * rows ignore the pin: a pin is about where a session sits in the tree, and
 * inside the archive there is no tree to sit in.
 */
export function partitionArchivedSessions(sessions: AgentSession[]): {
  active: AgentSession[];
  archived: AgentSession[];
} {
  const active: AgentSession[] = [];
  const archived: AgentSession[] = [];
  for (const session of sessions) {
    (session.archived ? archived : active).push(session);
  }
  return { active, archived: archived.sort(compareSessionsByActivityDesc) };
}

/** Latest member activity; an empty project falls back to its own createdAt. */
function bucketActivityKey(bucket: AgentProjectBucket): Timestamp {
  if (bucket.sessions.length > 0) return sessionActivityKey(bucket.sessions[0]);
  return bucket.project?.createdAt ?? 0;
}

/**
 * Group sessions into sorted Project → Session buckets.
 *
 *  - Every project gets a bucket, sessions or not.
 *  - Sessions with an empty or dangling projectId go to the ungrouped bucket,
 *    which is appended last and omitted when it would be empty.
 *  - Buckets holding a pinned session sort first, then by latest activity desc,
 *    ties by project name asc.
 *
 * Pure: inputs are not mutated; output order is independent of input order.
 */
export function groupSessionsByProject(
  projects: AgentProject[],
  sessions: AgentSession[],
): AgentProjectBucket[] {
  const projectBuckets = new Map<string, AgentProjectBucket>();
  for (const project of projects) {
    projectBuckets.set(project.id, { key: project.id, project, sessions: [] });
  }

  const ungrouped: AgentSession[] = [];
  for (const session of sessions) {
    const bucket = session.projectId
      ? projectBuckets.get(session.projectId)
      : undefined;
    if (bucket) bucket.sessions.push(session);
    else ungrouped.push(session);
  }

  const buckets = [...projectBuckets.values()];
  for (const bucket of buckets) bucket.sessions.sort(compareSessionsForTree);

  // sessions are sorted desc and pinned members sort first, so the first one
  // carries both the bucket's latest activity and its pin state.
  const bucketHasPinned = (bucket: AgentProjectBucket): boolean =>
    bucket.sessions.length > 0 && bucket.sessions[0].pinned;

  buckets.sort((a, b) => {
    const pinnedA = bucketHasPinned(a);
    const pinnedB = bucketHasPinned(b);
    if (pinnedA !== pinnedB) return pinnedA ? -1 : 1;
    const delta = bucketActivityKey(b) - bucketActivityKey(a);
    if (delta !== 0) return delta;
    const nameA = a.project?.name ?? "";
    const nameB = b.project?.name ?? "";
    if (nameA !== nameB) return nameA < nameB ? -1 : 1;
    return a.key < b.key ? -1 : a.key > b.key ? 1 : 0;
  });

  if (ungrouped.length === 0) return buckets;
  return [
    ...buckets,
    {
      key: UNGROUPED_BUCKET_KEY,
      project: null,
      sessions: ungrouped.sort(compareSessionsForTree),
    },
  ];
}
