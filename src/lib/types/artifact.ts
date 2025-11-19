/**
 * Artifact 相关类型定义
 */

import type { BaseEntity, UUID, ModelParameters } from "./index";

// Artifact 类型
export type ArtifactType = "shell" | "python" | "web";

// 执行配置
export interface ExecutionConfig {
  args?: string[];
  env?: Record<string, string>;
  permissions?: string[];
  timeout?: number; // milliseconds
}

// Artifact 实体
export interface Artifact extends BaseEntity {
  name: string;
  description?: string;
  type: ArtifactType;

  // Code/Resource paths
  entryFile: string;
  sourcePath?: string;

  // Optional AI model configuration
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  modelParameters?: ModelParameters;
  tools?: string[]; // MCP server names or tool identifiers

  // Execution configuration
  executionConfig: ExecutionConfig;

  // Installation & lifecycle
  isBuiltin: boolean;
  isInstalled: boolean;
  installedVersion?: string;
  installedAt?: number;
  lastRunAt?: number;
  runCount: number;

  // Metadata
  tags: string[];
  icon?: string;
  author?: string;
}

// 创建 Artifact 请求
export interface CreateArtifactRequest {
  name: string;
  description?: string;
  type: ArtifactType;
  entryFile: string;
  sourcePath?: string;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  modelParameters?: ModelParameters;
  tools?: string[];
  executionConfig?: ExecutionConfig;
  tags?: string[];
  icon?: string;
}

// 更新 Artifact 请求
export interface UpdateArtifactRequest {
  id: UUID;
  name?: string;
  description?: string;
  entryFile?: string;
  sourcePath?: string;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  modelParameters?: ModelParameters;
  tools?: string[];
  executionConfig?: ExecutionConfig;
  tags?: string[];
  icon?: string;
}

// 安装 Artifact 请求
export interface InstallArtifactRequest {
  artifactId: UUID;
  modelId?: string;
  providerId?: string;
}

// 执行 Artifact 请求
export interface ExecuteArtifactRequest {
  artifactId: UUID;
  args?: string[];
  env?: Record<string, string>;
}

// 执行结果
export interface ExecutionResult {
  success: boolean;
  stdout?: string;
  stderr?: string;
  exitCode?: number;
  duration: number;
  error?: string;
}

// Artifact 列表过滤
export interface ArtifactFilter {
  search?: string;
  artifactType?: ArtifactType;
  isBuiltin?: boolean;
  isInstalled?: boolean;
  tags?: string[];
  sortBy?: string;
  sortOrder?: string;
  limit?: number;
  offset?: number;
}
