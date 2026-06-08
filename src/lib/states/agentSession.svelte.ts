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
};
