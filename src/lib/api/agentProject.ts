/**
 * Params are passed with camelCase keys; Tauri maps them onto the backend's
 * snake_case arguments (see `commands/agent_project.rs`).
 */

import { apiCall } from "./index";
import type { UUID } from "../types";
import type { AgentProject } from "../types/agentProject";

/** Get-or-create by canonical path. */
export async function createAgentProject(path: string): Promise<AgentProject> {
  return apiCall<AgentProject>("agent_project_create", { path });
}

export async function getAgentProjects(): Promise<AgentProject[]> {
  const list = await apiCall<AgentProject[]>("agent_project_list", {});
  return list || [];
}

export async function renameAgentProject(
  projectId: UUID,
  name: string,
): Promise<AgentProject> {
  return apiCall<AgentProject>("agent_project_rename", { projectId, name });
}

/** Cascades: deletes the project's sessions and transcripts, aborting active runs. */
export async function deleteAgentProject(projectId: UUID): Promise<void> {
  return apiCall<void>("agent_project_delete", { projectId });
}
