/**
 * Agent 会话按 Agent → Project → Session 三级分组与排序 - 纯函数 selectors
 *
 * 层级：顶层按来源 Agent（session.agentDefinitionId）分桶；桶内若 session
 * 挂了 Project 则再下沉一层（Project → Session），未挂 Project 的 session 直接
 * 作为桶的子节点。无来源 Agent（agentDefinitionId 为空或悬挂引用已删 Agent）
 * 的 session 归入固定垫底的「Chats」桶，桶内同样应用 Project 下沉规则。
 *
 * 排序约束（沿用 M2 分组契约）：
 *  - session 活动键 = coalesce(lastMessageAt, createdAt) —— 绝不是 updatedAt。
 *    rename / 配置写入 bump updatedAt 但不构成「活动」，不得影响顺序。
 *  - 桶内子节点（Project 子组与散 session 混排）按活动键降序；Project 子组的
 *    活动键 = 组内最新 session 活动键。
 *  - Agent 桶按桶内最新活动键降序，并列按 Agent 名升序 tie-break；「Chats」桶
 *    恒定排在所有 Agent 桶之后。
 *  - 纯函数：不修改入参，输出顺序与入参数组顺序无关。
 */

import type { Timestamp } from "../types";
import type { Agent } from "../types/agent";
import type { AgentSession } from "../types/agentSession";
import type { AgentProject } from "../types/agentProject";

/** 无来源 Agent 会话的「Chats」桶保留 key（不会与 UUID 冲突）。 */
export const CHATS_BUCKET_KEY = "__chats__";

/** 一个 Project 子组：项目实体 + 组内（已排序）会话。 */
export interface AgentProjectGroup {
  project: AgentProject;
  sessions: AgentSession[];
}

/** 桶内的一个子节点：Project 子组，或未挂项目的散 session。 */
export type AgentBucketChild =
  | { kind: "project"; project: AgentProject; sessions: AgentSession[] }
  | { kind: "session"; session: AgentSession };

/** 一个顶层桶：某 Agent（`agent` 非空）或「Chats」（`agent` 为 null）。 */
export interface AgentSessionBucket {
  /** 折叠 / keyed-each 用的稳定 key：agent.id 或 `CHATS_BUCKET_KEY`。 */
  key: string;
  /** 桶归属的 Agent；null 表示「Chats」桶（无来源 Agent）。 */
  agent: Agent | null;
  /** 已按活动键降序排好的子节点（Project 子组与散 session 混排）。 */
  children: AgentBucketChild[];
}

/**
 * session 的活动键：coalesce(lastMessageAt, createdAt)。
 * 故意不使用 updatedAt（rename / 配置变更不构成活动，不得影响排序）。
 */
export function sessionActivityKey(session: AgentSession): Timestamp {
  return session.lastMessageAt ?? session.createdAt;
}

/**
 * 组内会话比较器：活动键降序；并列时 createdAt 降序、再按 id 升序，
 * 保证输出全序确定（与输入顺序无关）。
 */
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

/** 子节点的活动键：session 取自身；Project 子组取组内最新（已排序，取首个）。 */
function childActivityKey(child: AgentBucketChild): Timestamp {
  return child.kind === "session"
    ? sessionActivityKey(child.session)
    : sessionActivityKey(child.sessions[0]);
}

/** 子节点稳定 tie-break key：跨类型确定（project 前缀恒排在 session 前缀之前）。 */
function childTiebreakKey(child: AgentBucketChild): string {
  return child.kind === "project"
    ? `0:${child.project.id}`
    : `1:${child.session.id}`;
}

/** 桶内子节点排序：活动键降序，并列按稳定 tie-break key 升序。 */
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
 * 把一桶 session 按 Project 下沉为子节点列表：挂了（存在的）Project 的进 Project
 * 子组，未挂或悬挂 Project 的作为散 session；子组内 / 整体均按活动键降序。
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
 * 将会话按 Agent → Project → Session 分桶并排序。
 *
 *  - 每个「至少有一个 session」的 Agent 生成一个桶（无 session 的 Agent 不渲染）。
 *  - session 的 agentDefinitionId 为空、或悬挂引用了不存在的 Agent，归入「Chats」桶。
 *  - 桶按桶内最新活动键降序；并列按 Agent 名升序。「Chats」桶恒定垫底。
 *
 * 纯函数：不修改入参，结果顺序与入参数组顺序无关。
 */
export function groupSessionsByAgent(
  agents: Agent[],
  projects: AgentProject[],
  sessions: AgentSession[],
): AgentSessionBucket[] {
  const agentById = new Map<string, Agent>();
  // 仅索引已持久化（有 id）的 Agent；未保存的 Agent 无法作为归属 key。
  for (const agent of agents) {
    if (agent.id) agentById.set(agent.id, agent);
  }
  const projectById = new Map<string, AgentProject>();
  for (const project of projects) projectById.set(project.id, project);

  // 按解析后的 Agent 归属分桶：无 / 悬挂 Agent 落入 CHATS_BUCKET_KEY。
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

  // 桶排序键 = 桶内首个子节点的活动键（children 已按活动键降序）。
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

  // 「Chats」桶恒定垫底。
  return chatsBucket ? [...agentBuckets, chatsBucket] : agentBuckets;
}
