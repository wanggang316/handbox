/**
 * Agent project state - Svelte 5 runes.
 *
 * Mirrors the conventions of `states/agentSession.svelte.ts`: module-level
 * `$state` variables + a getter/setter state object + one actions object. The
 * list itself keeps no display order — grouping and sorting are pure-function
 * selectors in `utils/agentGrouping.ts`.
 */

import type { UUID } from "../types";
import type { AgentProject, AgentProjectSettings } from "../types/agentProject";
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

/** Swap a project row for the backend's copy, keyed by id (no-op if it is gone). */
function replaceProject(project: AgentProject): void {
  const index = projects.findIndex((item) => item.id === project.id);
  if (index !== -1) projects[index] = project;
}

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

  /**
   * Save the settings panel's fields. Not optimistic: the backend validates
   * the name and the color, and a sidebar row that already showed a rejected
   * value would have to be walked back.
   */
  async updateProjectSettings(
    id: UUID,
    settings: AgentProjectSettings,
  ): Promise<AgentProject> {
    const updated = await agentProjectApi.updateAgentProjectSettings(
      id,
      settings,
    );
    replaceProject(updated);
    return updated;
  },

  /**
   * Pin / unpin a project. Optimistic so the row reorders under the click; a
   * failure rolls the flag back and rethrows for the caller's error bar.
   */
  async setPinned(id: UUID, pinned: boolean): Promise<void> {
    const index = projects.findIndex((project) => project.id === id);
    if (index === -1) return;
    const previous = projects[index].pinned;
    projects[index] = { ...projects[index], pinned };
    try {
      replaceProject(await agentProjectApi.setAgentProjectPinned(id, pinned));
    } catch (error) {
      const current = projects.findIndex((project) => project.id === id);
      if (current !== -1) {
        projects[current] = { ...projects[current], pinned: previous };
      }
      console.error("Failed to pin agent project:", error);
      throw error;
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
