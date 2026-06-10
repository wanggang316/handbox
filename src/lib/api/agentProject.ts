/**
 * Agent Project 相关 API 封装
 *
 * 镜像 `api/agentSession.ts` 的形态：每个函数经 `apiCall(...)` 调用对应的
 * snake_case Tauri 命令，参数以 Tauri 期望的 camelCase key 传入
 * （后端签名见 `commands/agent_project.rs`：snake_case 参数由 Tauri 自动
 * 映射 camelCase，如 `project_id` <- `projectId`）。
 */

import { apiCall } from "./index";
import type { UUID } from "../types";
import type { AgentProject } from "../types/agentProject";

/**
 * 创建 Agent Project（get-or-create by canonical path）
 * 后端签名: agent_project_create(path: String)
 */
export async function createAgentProject(path: string): Promise<AgentProject> {
  return apiCall<AgentProject>("agent_project_create", { path });
}

/**
 * 获取 Agent Project 列表
 */
export async function getAgentProjects(): Promise<AgentProject[]> {
  const list = await apiCall<AgentProject[]>("agent_project_list", {});
  return list || [];
}

/**
 * 重命名 Agent Project
 * 后端签名: agent_project_rename(project_id: UUID, name: String)
 */
export async function renameAgentProject(
  projectId: UUID,
  name: string,
): Promise<AgentProject> {
  return apiCall<AgentProject>("agent_project_rename", { projectId, name });
}

/**
 * 删除 Agent Project（后端级联删除其会话与 transcript，并中止活跃 run）
 */
export async function deleteAgentProject(projectId: UUID): Promise<void> {
  return apiCall<void>("agent_project_delete", { projectId });
}
