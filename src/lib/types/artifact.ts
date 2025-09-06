/**
 * Artifact 相关类型定义
 */

import type { BaseEntity, UUID } from './index';

// Artifact 配置
export interface ArtifactConfig {
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  parameters?: {
    temperature?: number;
    topP?: number;
    maxTokens?: number;
    contextLength?: number;
    stream?: boolean;
  };
  mcpServers?: string[];
}

// Artifact 实体
export interface Artifact extends BaseEntity {
  name: string;
  description?: string;
  config: ArtifactConfig;
  lastUsedAt?: number;
  useCount: number;
  tags?: string[];
}

// Artifact 创建请求
export interface CreateArtifactRequest {
  name: string;
  description?: string;
  config: ArtifactConfig;
  tags?: string[];
}

// Artifact 更新请求
export interface UpdateArtifactRequest {
  id: UUID;
  name?: string;
  description?: string;
  config?: Partial<ArtifactConfig>;
  tags?: string[];
}

// Artifact 使用请求
export interface UseArtifactRequest {
  artifactId: UUID;
  chatName?: string;
}

// Artifact 列表过滤
export interface ArtifactFilter {
  search?: string;
  tags?: string[];
  sortBy?: 'name' | 'createdAt' | 'lastUsedAt' | 'useCount';
  sortOrder?: 'asc' | 'desc';
  limit?: number;
  offset?: number;
}

// Artifact 统计
export interface ArtifactStats {
  total: number;
  recentlyUsed: number;
  mostUsed: Artifact[];
  popularTags: Array<{ tag: string; count: number }>;
}