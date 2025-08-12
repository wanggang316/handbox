// Artifact 服务实现

use crate::services::StorageService;
use std::sync::Arc;

/// Artifact 服务
pub struct ArtifactService {
    storage: Arc<StorageService>,
}

impl ArtifactService {
    pub fn new(storage: Arc<StorageService>) -> Self {
        Self { storage }
    }

    // TODO: 实现 Artifact 管理功能
}
