/**
 * Agent Project 类型定义 - 镜像后端 Rust 形状
 *
 * 对齐 `storage/types/agent_project.rs`（`#[serde(rename_all = "camelCase")]`）：
 * id / path / name 为 TEXT，created_at / updated_at 为毫秒时间戳 i64。
 * `path` 是 canonical 化后的工作目录（canonicalize 在后端 service 层完成）。
 */

import type { UUID, Timestamp } from "./index";

/** Agent Project 实体 - 按工作目录分组 Agent 模式会话。 */
export interface AgentProject {
  id: UUID;
  path: string;
  name: string;
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

/**
 * 创建 Agent Project 请求（get-or-create 语义：同 canonical path 返回已有项目）。
 *
 * 后端命令 `agent_project_create` 直接接收 `path` 参数；`name` 由后端从
 * path 推导，前端无需提供。
 */
export interface CreateAgentProjectRequest {
  path: string;
}
