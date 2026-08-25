/**
 * Params are passed with camelCase keys; Tauri maps them onto the backend's
 * snake_case arguments (see `commands/agent_project.rs`).
 */

import { apiCall } from "./index";
import type { UUID } from "../types";
import type { AgentProject, AgentProjectSettings } from "../types/agentProject";

/** Get-or-create by canonical path. */
export async function createAgentProject(path: string): Promise<AgentProject> {
  return apiCall<AgentProject>("agent_project_create", { path });
}

export async function getAgentProjects(): Promise<AgentProject[]> {
  const list = await apiCall<AgentProject[]>("agent_project_list", {});
  return list || [];
}

/**
 * Writes the settings panel's fields as one group; `null` is a real value
 * (no color / follow the global default editor). The pin is deliberately not
 * part of it — see `setAgentProjectPinned`.
 */
export async function updateAgentProjectSettings(
  projectId: UUID,
  settings: AgentProjectSettings,
): Promise<AgentProject> {
  return apiCall<AgentProject>("agent_project_update_settings", {
    projectId,
    ...settings,
  });
}

export async function setAgentProjectPinned(
  projectId: UUID,
  pinned: boolean,
): Promise<AgentProject> {
  return apiCall<AgentProject>("agent_project_set_pinned", {
    projectId,
    pinned,
  });
}

/** Cascades: deletes the project's sessions and transcripts, aborting active runs. */
export async function deleteAgentProject(projectId: UUID): Promise<void> {
  return apiCall<void>("agent_project_delete", { projectId });
}
