/**
 * Agent 会话按 Project 分组与排序 - 纯函数 selectors
 *
 * 语义来源：M2 分组排序契约（GROUP-023 等）。关键约束：
 *  - session 活动键 = coalesce(lastMessageAt, createdAt) —— 绝不是 updatedAt。
 *    rename / 配置写入会 bump updatedAt，而契约 GROUP-023 规定重命名不得
 *    改变列表顺序，故排序对 updatedAt 完全不敏感。
 *  - 组内排序：活动键降序。
 *  - 组间排序键 = max(project.createdAt, 组内最新活动键) 降序；并列时按
 *    path 升序 tie-break。空项目以自身 createdAt 参与组间排序（新建空
 *    项目自然置顶）。
 *  - 未分组桶（projectId 为空或悬挂引用不存在的项目）固定排最后，
 *    不参与组间活动排序；桶内仍按活动键降序。
 */

import type { Timestamp } from "../types";
import type { AgentSession } from "../types/agentSession";
import type { AgentProject } from "../types/agentProject";

/** 一个 Project 分组：项目实体 + 组内（已排序）会话。 */
export interface AgentProjectGroup {
  project: AgentProject;
  sessions: AgentSession[];
}

/** `groupSessions` 的输出形态。 */
export interface GroupedAgentSessions {
  groups: AgentProjectGroup[];
  ungrouped: AgentSession[];
}

/**
 * session 的活动键：coalesce(lastMessageAt, createdAt)。
 *
 * 故意不使用 updatedAt —— rename / 配置变更会 bump updatedAt，
 * 但不构成"活动"，不得影响排序（GROUP-023）。
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

/**
 * 将会话按 Project 分组并排序。
 *
 *  - `groups`：每个已知项目恰好一组（含空项目），按组间排序键降序；
 *    组间键 = max(project.createdAt, 组内最新活动键)，并列按 path 升序。
 *  - `ungrouped`：projectId 缺失或悬挂（项目不存在）的会话，固定排最后。
 *
 * 纯函数：不修改入参，结果顺序与入参数组顺序无关。
 */
export function groupSessions(
  projects: AgentProject[],
  sessions: AgentSession[],
): GroupedAgentSessions {
  const sessionsByProject = new Map<string, AgentSession[]>();
  for (const project of projects) {
    sessionsByProject.set(project.id, []);
  }

  const ungrouped: AgentSession[] = [];
  for (const session of sessions) {
    const bucket = session.projectId
      ? sessionsByProject.get(session.projectId)
      : undefined;
    if (bucket) {
      bucket.push(session);
    } else {
      // projectId 为空，或悬挂引用了不存在的项目：进未分组桶。
      ungrouped.push(session);
    }
  }

  const groups: AgentProjectGroup[] = projects.map((project) => ({
    project,
    sessions: (sessionsByProject.get(project.id) ?? []).sort(
      compareSessionsByActivityDesc,
    ),
  }));

  // 组间排序键 = max(project.createdAt, 组内最新活动键)。
  // 组内已按活动键降序，首个元素即组内最新活动。
  const groupSortKey = (group: AgentProjectGroup): Timestamp => {
    const latest = group.sessions.length
      ? sessionActivityKey(group.sessions[0])
      : group.project.createdAt;
    return Math.max(group.project.createdAt, latest);
  };

  groups.sort((a, b) => {
    const keyDelta = groupSortKey(b) - groupSortKey(a);
    if (keyDelta !== 0) return keyDelta;
    // 并列时按 path 升序 tie-break（字节序比较，平台无关确定性）。
    const pathA = a.project.path;
    const pathB = b.project.path;
    return pathA < pathB ? -1 : pathA > pathB ? 1 : 0;
  });

  ungrouped.sort(compareSessionsByActivityDesc);

  return { groups, ungrouped };
}
