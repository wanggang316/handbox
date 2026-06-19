/**
 * Agent 相关状态管理 - 使用 Svelte 5 runes
 */

import type {
  Agent,
  UUID,
  McpServerConfig,
  AgentReasoningConfig,
} from "../types";
import * as agentApi from "../api/agent";

// 全局状态对象
export const agentState = $state({
  // Agent 列表
  agents: [] as Agent[],

  // 当前选中的 Agent（用于详情页面和编辑）
  currentAgent: null as Agent | null,

  // 正在编辑的 Agent（用于模态框）
  editingAgent: null as Agent | null,

  // 加载状态
  isLoading: false,

  // 错误状态
  error: null as string | null,
});

// Agent 状态管理辅助函数
export const agentStateActions = {
  /**
   * 设置当前 Agent
   */
  setCurrentAgent(agent: Agent | null): void {
    agentState.currentAgent = agent;
  },

  /**
   * 根据 ID 设置当前 Agent
   */
  async setCurrentAgentById(agentId: UUID): Promise<Agent | null> {
    try {
      const agent = await agentActions.getAgent(agentId);
      agentState.currentAgent = agent;
      return agent;
    } catch (error) {
      console.error("Failed to load agent:", error);
      return null;
    }
  },

  /**
   * 开始编辑 Agent（用于模态框）
   */
  startEditAgent(agent: Agent | null): void {
    agentState.editingAgent = agent;
  },

  /**
   * 结束编辑 Agent
   */
  endEditAgent(): void {
    agentState.editingAgent = null;
  },

  /**
   * 清除所有选中状态
   */
  clearSelection(): void {
    agentState.currentAgent = null;
    agentState.editingAgent = null;
  },
};

/**
 * Agent 操作
 */
export const agentActions = {
  /**
   * 加载 Agent 列表
   */
  async loadAgents(): Promise<void> {
    try {
      agentState.isLoading = true;
      agentState.error = null;
      const agentList = await agentApi.getAgents();
      agentState.agents = agentList;
    } catch (error) {
      agentState.error =
        error instanceof Error ? error.message : "加载 Agent 列表失败";
      throw error;
    } finally {
      agentState.isLoading = false;
    }
  },

  /**
   * 获取单个 Agent
   */
  async getAgent(agentId: string): Promise<Agent> {
    const agent = await agentApi.getAgent(agentId);
    return agent;
  },

  /**
   * 创建 Agent
   */
  async createAgent(config: {
    name: string;
    model?: string;
    temperature?: number;
    topP?: number;
    topK?: number;
    reasoning?: AgentReasoningConfig;
    maxTokens?: number;
    systemPrompt?: string;
    mcpServers?: McpServerConfig[];
    skills?: string[];
    generativeUi?: boolean;
  }): Promise<Agent> {
    try {
      agentState.isLoading = true;
      agentState.error = null;

      const agent = await agentApi.createAgent(
        config.name,
        config.model,
        config.temperature,
        config.topP,
        config.topK,
        config.reasoning,
        config.maxTokens,
        config.systemPrompt,
        config.mcpServers,
        config.skills,
        config.generativeUi,
      );

      // 添加到列表
      agentState.agents.unshift(agent);

      return agent;
    } catch (error) {
      agentState.error =
        error instanceof Error ? error.message : "创建 Agent 失败";
      throw error;
    } finally {
      agentState.isLoading = false;
    }
  },

  /**
   * 更新 Agent 名称
   */
  async updateAgentName(agentId: UUID, name: string): Promise<Agent> {
    try {
      agentState.isLoading = true;
      agentState.error = null;

      const updatedAgent = await agentApi.updateAgentName(agentId, name);

      // 更新列表中的 Agent
      const index = agentState.agents.findIndex((a) => a.id === agentId);
      if (index !== -1) {
        agentState.agents[index] = updatedAgent;
      }

      // 如果是当前选中的 Agent，也更新它
      if (agentState.currentAgent?.id === agentId) {
        agentState.currentAgent = updatedAgent;
      }

      return updatedAgent;
    } catch (error) {
      agentState.error =
        error instanceof Error ? error.message : "更新 Agent 名称失败";
      throw error;
    } finally {
      agentState.isLoading = false;
    }
  },

  /**
   * 更新 Agent 字段
   */
  async updateAgentField(
    agentId: UUID,
    fieldName:
      | "model"
      | "temperature"
      | "topP"
      | "topK"
      | "maxTokens"
      | "systemPrompt"
      | "mcpServers"
      | "skills"
      | "reasoning"
      | "generativeUi",
    value:
      | number
      | boolean
      | string
      | McpServerConfig[]
      | string[]
      | AgentReasoningConfig
      | null,
  ): Promise<Agent> {
    try {
      agentState.isLoading = true;
      agentState.error = null;

      const updatedAgent = await agentApi.updateAgentField(
        agentId,
        fieldName,
        value,
      );

      // 更新列表中的 Agent
      const index = agentState.agents.findIndex((a) => a.id === agentId);
      if (index !== -1) {
        agentState.agents[index] = updatedAgent;
      }

      // 如果是当前选中的 Agent，也更新它
      if (agentState.currentAgent?.id === agentId) {
        agentState.currentAgent = updatedAgent;
      }

      return updatedAgent;
    } catch (error) {
      agentState.error =
        error instanceof Error ? error.message : "更新 Agent 失败";
      throw error;
    } finally {
      agentState.isLoading = false;
    }
  },

  /**
   * 删除 Agent
   */
  async deleteAgent(agentId: UUID): Promise<void> {
    try {
      agentState.isLoading = true;
      agentState.error = null;

      await agentApi.deleteAgent(agentId);

      // 从列表中移除
      agentState.agents = agentState.agents.filter((a) => a.id !== agentId);

      // 如果是当前选中的 Agent，清空选择
      if (agentState.currentAgent?.id === agentId) {
        agentStateActions.clearSelection();
      }
    } catch (error) {
      agentState.error =
        error instanceof Error ? error.message : "删除 Agent 失败";
      throw error;
    } finally {
      agentState.isLoading = false;
    }
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    agentState.error = null;
  },

  /**
   * 重置所有状态
   */
  reset(): void {
    agentState.agents = [];
    agentState.currentAgent = null;
    agentState.editingAgent = null;
    agentState.isLoading = false;
    agentState.error = null;
  },
};
