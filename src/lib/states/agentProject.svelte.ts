/**
 * Agent project state - Svelte 5 runes.
 *
 * Mirrors the conventions of `states/agentSession.svelte.ts`: module-level
 * `$state` variables + a getter/setter state object + one actions object. The
 * list itself keeps no display order — grouping and sorting are pure-function
 * selectors in `utils/agentGrouping.ts`.
 */

import type { UUID } from "../types";
import type { AgentProject } from "../types/agentProject";
import * as agentProjectApi from "../api/agentProject";
import { agentSessionState } from "./agentSession.svelte";

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
  /** Load the project list (wholesale replace; display order is up to selectors). */
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
   * Create an agent project (backend is get-or-create by canonical path).
   *
   * Dedupe by id: if the returned project is already in the list (same path
   * hit an existing project), replace it in place instead of inserting a
   * duplicate; otherwise insert at the top.
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

  async renameProject(id: UUID, name: string): Promise<void> {
    const updated = await agentProjectApi.renameAgentProject(id, name);
    const index = projects.findIndex((project) => project.id === id);
    if (index !== -1) {
      projects[index] = updated;
    }
  },

  /**
   * Delete an agent project: remove it from the list and also remove all of
   * its sessions from the agentSession store (mirrors the backend cascade
   * delete); clear the current session if it belongs to the project.
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
