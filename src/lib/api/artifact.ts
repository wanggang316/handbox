/**
 * Artifact 相关 API 封装
 */

import { apiCall } from "./index";
import type {
  Artifact,
  CreateArtifactRequest,
  UpdateArtifactRequest,
  InstallArtifactRequest,
  ExecuteArtifactRequest,
  ExecutionResult,
  ArtifactFilter,
  UUID,
} from "../types";

/**
 * 获取 Artifact 列表
 */
export async function getArtifacts(
  filter?: ArtifactFilter,
): Promise<Artifact[]> {
  return apiCall<Artifact[]>("artifact_list", filter);
}

/**
 * 获取 Artifact 详情
 */
export async function getArtifact(artifactId: UUID): Promise<Artifact> {
  return apiCall<Artifact>("artifact_get", { artifactId });
}

/**
 * 创建 Artifact
 */
export async function createArtifact(
  request: CreateArtifactRequest,
): Promise<Artifact> {
  return apiCall<Artifact>("artifact_create", request);
}

/**
 * 更新 Artifact
 */
export async function updateArtifact(
  request: UpdateArtifactRequest,
): Promise<Artifact> {
  return apiCall<Artifact>("artifact_update", request);
}

/**
 * 删除 Artifact
 */
export async function deleteArtifact(artifactId: UUID): Promise<void> {
  return apiCall<void>("artifact_delete", { artifactId });
}

/**
 * 安装 Artifact
 */
export async function installArtifact(
  request: InstallArtifactRequest,
): Promise<Artifact> {
  return apiCall<Artifact>("artifact_install", request);
}

/**
 * 执行 Artifact
 */
export async function executeArtifact(
  request: ExecuteArtifactRequest,
): Promise<ExecutionResult> {
  return apiCall<ExecutionResult>("artifact_execute", request);
}

/**
 * 初始化内置 Artifacts
 */
export async function initBuiltinArtifacts(): Promise<Artifact[]> {
  return apiCall<Artifact[]>("artifact_init_builtin");
}
