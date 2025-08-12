// 供应商服务实现

use crate::services::StorageService;
use std::sync::Arc;

/// 供应商服务
pub struct ProviderService {
    storage: Arc<StorageService>,
}

impl ProviderService {
    pub fn new(storage: Arc<StorageService>) -> Self {
        Self { storage }
    }

    // TODO: 实现供应商管理功能
}
