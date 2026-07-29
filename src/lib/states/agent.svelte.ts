/**
 * Agent state - Svelte 5 runes.
 */

import type {
  Agent,
  UUID,
  McpServerConfig,
  AgentReasoningConfig,
} from "../types";
import * as agentApi from "../api/agent";

export const agentState = $state({
  agents: [] as Agent[],

  // Selected agent for the detail page.
  currentAgent: null as Agent | null,

  // Agent being edited in the modal.
  editingAgent: null as Agent | null,

  isLoading: false,

  error: null as string | null,
});

export const agentStateActions = {
  setCurrentAgent(agent: Agent | null): void {
    agentState.currentAgent = agent;
  },

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

  startEditAgent(agent: Agent | null): void {
    agentState.editingAgent = agent;
  },

  endEditAgent(): void {
    agentState.editingAgent = null;
  },

  clearSelection(): void {
    agentState.currentAgent = null;
    agentState.editingAgent = null;
  },
};

export const agentActions = {
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

  async getAgent(agentId: string): Promise<Agent> {
    const agent = await agentApi.getAgent(agentId);
    return agent;
  },

  async createAgent(config: {
    name: string;
    temperature?: number;
    topP?: number;
    topK?: number;
    reasoning?: AgentReasoningConfig;
    maxTokens?: number;
    systemPrompt?: string;
    mcpServers?: McpServerConfig[];
    skills?: string[];
    generativeUi?: boolean;
    genuiId?: string;
  }): Promise<Agent> {
    try {
      agentState.isLoading = true;
      agentState.error = null;

      const agent = await agentApi.createAgent(
        config.name,
        config.temperature,
        config.topP,
        config.topK,
        config.reasoning,
        config.maxTokens,
        config.systemPrompt,
        config.mcpServers,
        config.skills,
        config.generativeUi,
        config.genuiId,
      );

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

  async updateAgentName(agentId: UUID, name: string): Promise<Agent> {
    try {
      agentState.isLoading = true;
      agentState.error = null;

      const updatedAgent = await agentApi.updateAgentName(agentId, name);

      const index = agentState.agents.findIndex((a) => a.id === agentId);
      if (index !== -1) {
        agentState.agents[index] = updatedAgent;
      }

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

  async updateAgentField(
    agentId: UUID,
    fieldName:
      | "temperature"
      | "topP"
      | "topK"
      | "maxTokens"
      | "systemPrompt"
      | "mcpServers"
      | "skills"
      | "reasoning"
      | "generativeUi"
      | "genuiId"
      | "providerId"
      | "icon"
      | "description"
      | "builtinTools"
      | "workingDirMode"
      | "toolExecutionMode"
      | "thinkingLevel"
      | "starters",
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

      const index = agentState.agents.findIndex((a) => a.id === agentId);
      if (index !== -1) {
        agentState.agents[index] = updatedAgent;
      }

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

  async deleteAgent(agentId: UUID): Promise<void> {
    try {
      agentState.isLoading = true;
      agentState.error = null;

      await agentApi.deleteAgent(agentId);

      agentState.agents = agentState.agents.filter((a) => a.id !== agentId);

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

  clearError(): void {
    agentState.error = null;
  },

  reset(): void {
    agentState.agents = [];
    agentState.currentAgent = null;
    agentState.editingAgent = null;
    agentState.isLoading = false;
    agentState.error = null;
  },
};
