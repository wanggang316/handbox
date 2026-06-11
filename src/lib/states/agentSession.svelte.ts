/**
 * Agent Session 状态管理 - Svelte 5 runes
 *
 * 镜像 `states/chat.svelte.ts` 的约定：模块级 `$state` 变量 + getter/setter 暴露的
 * 状态对象 + 一个动作对象。仅负责 session 的 CRUD 与列表交互，
 * run / timeline 由后续 feature 承担。
 */

import type { UUID, AgentSession, CreateAgentSessionRequest } from "../types";
import type { AgentSessionField } from "../api/agentSession";
import * as agentSessionApi from "../api/agentSession";
import { normalizeError } from "../utils/error";

// ============================================
// Agent Session 状态 - 使用 Svelte 5 runes
// ============================================
let sessions = $state<AgentSession[]>([]);
let currentSession = $state<AgentSession | null>(null);
let isLoading = $state(false);

export const agentSessionState = {
  get sessions() {
    return sessions;
  },
  set sessions(value) {
    sessions = value;
  },

  get currentSession() {
    return currentSession;
  },
  set currentSession(value) {
    currentSession = value;
  },

  get isLoading() {
    return isLoading;
  },
  set isLoading(value) {
    isLoading = value;
  },
};

export const agentSessionActions = {
  /**
   * 加载 Agent Session 列表（后端已按 updatedAt DESC 返回，原样保留顺序）。
   */
  async loadSessions(): Promise<void> {
    try {
      isLoading = true;
      sessions = await agentSessionApi.getAgentSessions();
    } catch (error) {
      console.error("Failed to load agent sessions:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 创建新的 Agent Session：插入列表顶部并设为当前。
   */
  async createSession(
    config: CreateAgentSessionRequest,
  ): Promise<AgentSession> {
    try {
      isLoading = true;
      const session = await agentSessionApi.createAgentSession(config);
      const existing = Array.isArray(sessions) ? sessions : [];
      sessions = [session, ...existing];
      currentSession = session;
      return session;
    } catch (error) {
      console.error("Failed to create agent session:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 重命名 Agent Session。
   */
  async renameSession(id: UUID, name: string): Promise<void> {
    const updated = await agentSessionApi.renameAgentSession(id, name);
    const index = sessions.findIndex((session) => session.id === id);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === id) {
      currentSession = updated;
    }
  },

  /**
   * 删除 Agent Session：从列表移除；若为当前会话则清空当前。
   */
  async deleteSession(id: UUID): Promise<void> {
    try {
      isLoading = true;
      await agentSessionApi.deleteAgentSession(id);
      sessions = sessions.filter((session) => session.id !== id);
      if (currentSession?.id === id) {
        currentSession = null;
      }
    } catch (error) {
      console.error("Failed to delete agent session:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 更新 Agent Session 的单个字段（同步本地列表与当前会话）。
   */
  async updateField(
    id: UUID,
    field: AgentSessionField,
    value: string | number | string[] | null,
  ): Promise<void> {
    const updated = await agentSessionApi.updateAgentSessionField(
      id,
      field,
      value,
    );
    const index = sessions.findIndex((session) => session.id === id);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === id) {
      currentSession = updated;
    }
  },

  /**
   * 将列表中已存在的某个会话设为当前（不触发网络请求）。
   */
  setCurrentById(id: UUID): AgentSession | null {
    const session = sessions.find((item) => item.id === id) ?? null;
    currentSession = session;
    return session;
  },

  /**
   * 一次 run 结束后刷新某会话的侧栏元数据（VAL-PERSIST-011）。
   *
   * run 期间后端按 message_end 追加 transcript 并 bump 该会话的 `messageCount` /
   * `lastMessageAt` / `updatedAt`，但前端列表持有的是 run 之前的快照。`agent_stream_closed`
   * 抵达时调用本方法重新拉取该会话详情，更新列表内对应项与当前会话，并按
   * `updatedAt DESC` 重排到顶部 —— 使计数 / 最近时间 / 顺序无需手动刷新即更新。
   *
   * 该会话不在列表内（例如已被删除）则为干净 no-op；重拉撞上 NOT_FOUND
   * （abort 收尾先于 delete IPC 回包的竞态）则静默移除该行；其余错误仅记录、
   * 不抛出，避免影响 run 终结的其它收尾。
   */
  async refreshAfterRun(id: UUID): Promise<void> {
    if (!sessions.some((session) => session.id === id)) {
      // 已删除（或从未在列表内）的会话：静默 no-op（GROUP-018 / CROSS-008）。
      // 不发起重拉——对已删 id 的 agent_session_get 必然 NOT_FOUND，
      // 落进下方 catch 会留下无意义的 console.error 噪音。
      return;
    }
    try {
      const updated = await agentSessionApi.getAgentSession(id);
      const others = sessions.filter((session) => session.id !== id);
      if (others.length === sessions.length) {
        // await 期间被删除：不插入幽灵条目。
        return;
      }
      // 置顶 + 刷新元数据（重拉对象携带后端最新 lastMessageAt，
      // groupSessions 据活动键自动把它排到组内第一并上浮该组）。
      sessions = [updated, ...others];
      if (currentSession?.id === id) {
        currentSession = updated;
      }
    } catch (error) {
      if (normalizeError(error).code === "NOT_FOUND") {
        // Session deleted while refreshing (abort-closed raced the delete IPC):
        // drop the stale row silently — no ghost, no console noise (CROSS-008).
        sessions = sessions.filter((session) => session.id !== id);
        return;
      }
      console.error("Failed to refresh agent session after run:", error);
    }
  },
};
