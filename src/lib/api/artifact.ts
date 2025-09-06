/**
 * Artifact 相关 API 封装
 */

import { apiCall } from './index';
import type { 
  Artifact, 
  CreateArtifactRequest,
  UpdateArtifactRequest,
  UseArtifactRequest,
  ArtifactFilter,
  ArtifactStats,
  ChatSession,
  UUID 
} from '../types';

/**
 * 获取 Artifact 列表
 */
export async function getArtifacts(filter?: ArtifactFilter): Promise<Artifact[]> {
  return apiCall<Artifact[]>('artifact_list', filter);
}

/**
 * 获取 Artifact 详情
 */
export async function getArtifact(artifactId: UUID): Promise<Artifact> {
  return apiCall<Artifact>('artifact_get', { artifactId });
}

/**
 * 创建 Artifact
 */
export async function createArtifact(request: CreateArtifactRequest): Promise<Artifact> {
  return apiCall<Artifact>('artifact_create', request);
}

/**
 * 更新 Artifact
 */
export async function updateArtifact(request: UpdateArtifactRequest): Promise<Artifact> {
  return apiCall<Artifact>('artifact_update', request);
}

/**
 * 删除 Artifact
 */
export async function deleteArtifact(artifactId: UUID): Promise<void> {
  return apiCall<void>('artifact_delete', { artifactId });
}

/**
 * 使用 Artifact 创建新会话
 */
export async function useArtifact(request: UseArtifactRequest): Promise<ChatSession> {
  return apiCall<ChatSession>('artifact_use', request);
}

/**
 * 复制 Artifact
 */
export async function duplicateArtifact(
  artifactId: UUID,
  name?: string
): Promise<Artifact> {
  return apiCall<Artifact>('artifact_duplicate', { artifactId, name });
}

/**
 * 获取 Artifact 统计信息
 */
export async function getArtifactStats(): Promise<ArtifactStats> {
  return apiCall<ArtifactStats>('artifact_stats');
}

/**
 * 从会话保存为 Artifact
 */
export async function saveChatAsArtifact(
  chatId: UUID,
  name: string,
  description?: string,
  tags?: string[]
): Promise<Artifact> {
  return apiCall<Artifact>('artifact_save_from_chat', {
    chatId,
    name,
    description,
    tags
  });
}

/**
 * 搜索 Artifact
 */
export async function searchArtifacts(
  query: string,
  limit?: number
): Promise<Artifact[]> {
  return apiCall<Artifact[]>('artifact_search', { query, limit });
}