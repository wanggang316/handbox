/**
 * Agent Project 状态管理 - Svelte 5 runes
 *
 * 镜像 `states/agentSession.svelte.ts` 的约定：模块级 `$state` 变量 +
 * getter/setter 暴露的状态对象 + 一个动作对象。列表本身不维护展示顺序 ——
 * 分组与排序由 `utils/agentGrouping.ts` 的纯函数 selectors 负责。
 */

import type { UUID } from "../types";
import type { AgentProject } from "../types/agentProject";
import * as agentProjectApi from "../api/agentProject";
import { agentSessionState } from "./agentSession.svelte";

// ============================================
// Agent Project 状态 - 使用 Svelte 5 runes
// ============================================
let projects = $state<AgentProject[]>([]);
let isLoading = $state(false);

export const agentProjectState = {
  get projects() {
    return projects;
  },
  set projects(value) {
    projects = value;
  },

  get isLoading() {
    return isLoading;
  },
  set isLoading(value) {
    isLoading = value;
  },
};

export const agentProjectActions = {
  /**
   * 加载 Agent Project 列表（整体替换，展示顺序由 selectors 决定）。
   */
  async loadProjects(): Promise<void> {
    try {
      isLoading = true;
      projects = await agentProjectApi.getAgentProjects();
    } catch (error) {
      console.error("Failed to load agent projects:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 创建 Agent Project（后端为 get-or-create by canonical path）。
   *
   * 去重按 id 判断：若返回的项目已在列表内（同 path 命中既有项目），
   * 原位替换而不重复插入；否则插入列表顶部。
   */
  async createProject(path: string): Promise<AgentProject> {
    try {
      isLoading = true;
      const project = await agentProjectApi.createAgentProject(path);
      const index = projects.findIndex((item) => item.id === project.id);
      if (index !== -1) {
        projects[index] = project;
      } else {
        projects = [project, ...projects];
      }
      return project;
    } catch (error) {
      console.error("Failed to create agent project:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 重命名 Agent Project（原位替换列表项）。
   */
  async renameProject(id: UUID, name: string): Promise<void> {
    const updated = await agentProjectApi.renameAgentProject(id, name);
    const index = projects.findIndex((project) => project.id === id);
    if (index !== -1) {
      projects[index] = updated;
    }
  },

  /**
   * 删除 Agent Project：从列表移除，并联动 agentSession store 移除该
   * 项目下的全部会话（镜像后端级联删除）；若当前会话属于该项目则清空。
   */
  async deleteProject(id: UUID): Promise<void> {
    try {
      isLoading = true;
      await agentProjectApi.deleteAgentProject(id);
      projects = projects.filter((project) => project.id !== id);
      agentSessionState.sessions = agentSessionState.sessions.filter(
        (session) => session.projectId !== id,
      );
      if (agentSessionState.currentSession?.projectId === id) {
        agentSessionState.currentSession = null;
      }
    } catch (error) {
      console.error("Failed to delete agent project:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },
};
