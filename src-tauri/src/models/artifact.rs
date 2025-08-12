// Artifact 相关数据模型

use super::{ChatConfig, Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// Artifact 实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: UUID,
    pub name: String,
    pub description: Option<String>,
    pub config: ChatConfig,
    pub last_used_at: Option<Timestamp>,
    pub use_count: i32,
    pub tags: Option<Vec<String>>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 创建 Artifact 请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateArtifactRequest {
    pub name: String,
    pub description: Option<String>,
    pub config: ChatConfig,
    pub tags: Option<Vec<String>>,
}

/// 更新 Artifact 请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateArtifactRequest {
    pub id: UUID,
    pub name: Option<String>,
    pub description: Option<String>,
    pub config: Option<ChatConfig>,
    pub tags: Option<Vec<String>>,
}

/// 使用 Artifact 请求
#[derive(Debug, Clone, Deserialize)]
pub struct UseArtifactRequest {
    pub artifact_id: UUID,
    pub session_name: Option<String>,
}

/// Artifact 过滤器
#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactFilter {
    pub search: Option<String>,
    pub tags: Option<Vec<String>>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Artifact 统计
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactStats {
    pub total: i32,
    pub recently_used: i32,
    pub most_used: Vec<Artifact>,
    pub popular_tags: Vec<TagStats>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagStats {
    pub tag: String,
    pub count: i32,
}
