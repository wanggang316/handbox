// 设置服务实现

use crate::services::StorageService;
use std::sync::Arc;

/// 设置服务
pub struct SettingsService {
    storage: Arc<StorageService>,
}

impl SettingsService {
    pub fn new(storage: Arc<StorageService>) -> Self {
        Self { storage }
    }

    // TODO: 实现设置管理功能
}
