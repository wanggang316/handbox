/**
 * Agent Session 状态管理 - Svelte 5 runes
 *
 * 遵循本项目 store 约定：模块级 `$state` 变量 + getter/setter 暴露的
 * 状态对象 + 一个动作对象。仅负责 session 的 CRUD 与列表交互，
 * run / timeline 由后续 feature 承担。
 */

import type {
  UUID,
  AgentSession,
  CreateAgentSessionRequest,
  InstantiateAgentSessionRequest,
} from "../types";
import type { McpServerConfig } from "../types/llm";
import type { AgentSessionField } from "../api/agentSession";
import * as agentSessionApi from "../api/agentSession";
import { normalizeError } from "../utils/error";
import { agentState } from "./agent.svelte";

// ============================================
// Agent Session 状态 - 使用 Svelte 5 runes
// ============================================
let sessions = $state<AgentSession[]>([]);
let currentSession = $state<AgentSession | null>(null);
let isLoading = $state(false);

// 已自动生成过标题的会话 id（每会话仅自动生成一次；失败会移除以允许重试）。
const autoTitledSessions = new Set<string>();

/**
 * 会话首个 run 结束后按需自动生成标题。仅当：这是首个 run、尚未自动生成过、且当前
 * 名称仍等于来源 Agent 的默认名（说明用户未手动命名）时触发一次。后台进行、失败静默
 * ——用户始终可通过右键菜单手动生成。这样绝不覆盖用户手动设置的标题。
 */
async function maybeAutoGenerateTitle(
  id: string,
  wasFirstRun: boolean,
  currentName: string,
  agentDefinitionId?: string,
): Promise<void> {
  if (!wasFirstRun || autoTitledSessions.has(id)) return;
  const agent = agentDefinitionId
    ? agentState.agents.find((a) => a.id === agentDefinitionId)
    : undefined;
  // 无法解析来源 Agent（无 / 悬挂 agentDefinitionId），或名称已非默认名：不自动改名。
  if (!agent || agent.name !== currentName) return;

  autoTitledSessions.add(id);
  try {
    await agentSessionActions.generateTitle(id);
  } catch (error) {
    autoTitledSessions.delete(id);
    console.warn("Auto title generation failed:", error);
  }
}

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
   * 从 AgentDefinition 实例化会话：插入列表顶部并设为当前。
   *
   * 统一的「用此 Agent」入口，取代 chat 侧的 `createSessionFromAgent`。
   * 能力集快照与工作目录策略由后端按 definition 裁决；`overrides` 仅覆盖
   * name/project/workingDir/model/provider。
   */
  async createSessionFromDefinition(
    definitionId: UUID,
    overrides?: InstantiateAgentSessionRequest,
  ): Promise<AgentSession> {
    try {
      isLoading = true;
      const session = await agentSessionApi.createSessionFromDefinition(
        definitionId,
        overrides,
      );
      const existing = Array.isArray(sessions) ? sessions : [];
      sessions = [session, ...existing];
      currentSession = session;
      return session;
    } catch (error) {
      console.error("Failed to create agent session from definition:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 就地把一个已存在会话重指到另一个 AgentDefinition（不新建会话）。
   *
   * 用于「当前会话尚无消息时切换 Agent」：复用现有会话 id，由后端重新快照能力集
   * 与参数、改写 provenance。就地替换列表中对应项并同步当前会话（id 不变、不重排）。
   */
  async reinstantiateFromDefinition(
    sessionId: UUID,
    definitionId: UUID,
    overrides?: InstantiateAgentSessionRequest,
  ): Promise<AgentSession> {
    const updated = await agentSessionApi.reinstantiateSessionFromDefinition(
      sessionId,
      definitionId,
      overrides,
    );
    const index = sessions.findIndex((session) => session.id === sessionId);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === sessionId) {
      currentSession = updated;
    }
    return updated;
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
   * 为会话生成标题（后端一次性 LLM 补全 + 落盘），并把返回的会话回填到列表与
   * 当前会话。失败向上抛，由调用方决定提示与否（自动路径静默，手动路径可提示）。
   */
  async generateTitle(id: UUID): Promise<AgentSession> {
    const updated = await agentSessionApi.generateAgentSessionTitle(id);
    const index = sessions.findIndex((session) => session.id === id);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === id) {
      currentSession = updated;
    }
    return updated;
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
    value: string | number | string[] | McpServerConfig[] | null,
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
   * 即时应用会话名变更（VAL-CARUN-020）：会话名经后端 `SessionInfoChanged{name}`
   * 生命周期信号变更时，由 `agentRunStore` 的会话名变更回调以该会话 id + 新名调用，
   * 就地更新侧栏列表对应项与当前会话的标题，无需重开 / 手动刷新。
   *
   * 纯本地状态更新（不发网络请求，不重排顺序——只换标题）。该会话不在列表内
   * （例如已删除）则为干净 no-op。`name` 为 null（清空标题）时回退为空串，使
   * 侧栏渲染不出现 `undefined`/破裂的占位。
   */
  applySessionName(id: UUID, name: string | null): void {
    const nextName = name ?? "";
    const index = sessions.findIndex((session) => session.id === id);
    if (index === -1) {
      // 不在列表内：干净 no-op（不创建幽灵条目）。
      return;
    }
    sessions[index] = { ...sessions[index], name: nextName };
    if (currentSession?.id === id) {
      currentSession = { ...currentSession, name: nextName };
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
    const prev = sessions.find((session) => session.id === id);
    if (!prev) {
      // 已删除（或从未在列表内）的会话：静默 no-op（GROUP-018 / CROSS-008）。
      // 不发起重拉——对已删 id 的 agent_session_get 必然 NOT_FOUND，
      // 落进下方 catch 会留下无意义的 console.error 噪音。
      return;
    }
    // 在重拉刷新 messageCount 之前捕获「是否首个 run」，供自动起标题判定。
    const wasFirstRun = prev.messageCount === 0;
    const agentDefinitionId = prev.agentDefinitionId;
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
      // 首个 run 后按需自动生成标题（后台、失败静默、绝不覆盖手动标题）。
      void maybeAutoGenerateTitle(id, wasFirstRun, updated.name, agentDefinitionId);
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
