// 搜索服务实现

use crate::services::StorageService;
use std::sync::Arc;

/// 搜索服务
pub struct SearchService {
    storage: Arc<StorageService>,
}

impl SearchService {
    pub fn new(storage: Arc<StorageService>) -> Self {
        Self { storage }
    }

    // TODO: 实现全文搜索功能
}
