// 供应商服务实现

use crate::models::{
    AppError, Model, Provider, ProviderConfig,
    ProviderWithModels, Timestamp, UUID,
};
use crate::services::{DatabaseService, ProviderRepository};
use crate::services::provider_clients::{create_llm_client, adapt_model};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// 供应商服务
pub struct ProviderService {
    repository: ProviderRepository,
}

impl ProviderService {
    /// 创建新的供应商服务实例
    pub fn new(db: DatabaseService) -> Self {
        Self {
            repository: ProviderRepository::new(db),
        }
    }

    /// 初始化模型数据库（在服务启动时调用）
    pub fn initialize_model_database() {
        // 新的动态组装系统不需要预先初始化配置
        tracing::info!("Dynamic LLM client system ready");
    }

    /// 创建供应商
    pub async fn create_provider(&self, config: ProviderConfig) -> Result<Provider, AppError> {
        let provider = Provider {
            id: Uuid::new_v4().to_string(),
            name: config.name,
            provider_type: config.provider_type,
            base_url: config.base_url,
            api_key: config.api_key,
            enabled: config.enabled.unwrap_or(true),
            created_at: self.current_timestamp(),
            updated_at: self.current_timestamp(),
        };

        self.repository.create_provider(&provider).await?;
        
        // 自动拉取并保存模型列表（仅在非测试环境中）
        if !cfg!(test) {
            tracing::info!("Auto-fetching models for new provider: {}", provider.name);
            match self.get_provider_models(&provider.id, true).await {
                Ok(models) => {
                    tracing::info!("Successfully fetched {} models for provider {}", models.len(), provider.name);
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch models for provider {}: {}", provider.name, e);
                    // 不让模型获取失败影响供应商创建
                }
            }
        }

        Ok(provider)
    }

    /// 更新供应商
    pub async fn update_provider(
        &self,
        provider_id: &UUID,
        config: ProviderConfig,
    ) -> Result<Provider, AppError> {
        let existing_provider = self.get_provider(provider_id).await?;
        
        // 检查是否有关键配置变更
        let should_refresh_models = config.api_key != existing_provider.api_key ||
            config.base_url != existing_provider.base_url ||
            config.provider_type != existing_provider.provider_type;

        let updated_provider = Provider {
            id: existing_provider.id,
            name: config.name,
            provider_type: config.provider_type,
            base_url: config.base_url,
            api_key: config.api_key,
            enabled: config.enabled.unwrap_or(existing_provider.enabled),
            created_at: existing_provider.created_at,
            updated_at: self.current_timestamp(),
        };

        self.repository.update_provider(&updated_provider).await?;
        
        // 如果关键配置有变更，自动刷新模型列表（仅在非测试环境中）
        if should_refresh_models && !cfg!(test) {
            tracing::info!("Key configuration changed, auto-refreshing models for provider: {}", updated_provider.name);
            match self.get_provider_models(&updated_provider.id, true).await {
                Ok(models) => {
                    tracing::info!("Successfully refreshed {} models for provider {}", models.len(), updated_provider.name);
                }
                Err(e) => {
                    tracing::warn!("Failed to refresh models for provider {}: {}", updated_provider.name, e);
                    // 不让模型获取失败影响供应商更新
                }
            }
        }

        Ok(updated_provider)
    }

    /// 获取单个供应商
    pub async fn get_provider(&self, provider_id: &UUID) -> Result<Provider, AppError> {
        self.repository.get_provider_by_id(provider_id).await?
            .ok_or_else(|| AppError::validation_error("Provider not found"))
    }

    /// 获取所有供应商
    pub async fn list_providers(&self) -> Result<Vec<Provider>, AppError> {
        self.repository.list_providers().await
    }

    /// 获取供应商及其模型
    pub async fn get_provider_with_models(
        &self,
        provider_id: &UUID,
    ) -> Result<ProviderWithModels, AppError> {
        let provider = self.get_provider(provider_id).await?;
        let models = self.repository.get_models_by_provider(provider_id).await?;
        
        Ok(ProviderWithModels {
            id: provider.id,
            name: provider.name,
            provider_type: provider.provider_type,
            base_url: provider.base_url,
            api_key: provider.api_key,
            enabled: provider.enabled,
            models,
            created_at: provider.created_at,
            updated_at: provider.updated_at,
        })
    }

    /// 删除供应商
    pub async fn delete_provider(&self, provider_id: &UUID) -> Result<(), AppError> {
        self.repository.delete_provider(provider_id).await
    }

    /// 获取所有可用模型
    pub async fn get_available_models(&self) -> Result<Vec<Model>, AppError> {
        // 获取所有启用供应商的模型
        let providers = self.list_providers().await?;
        let mut all_models = Vec::new();
        
        for provider in providers {
            if provider.enabled {
                let models = self.repository.get_models_by_provider(&provider.id).await?;
                all_models.extend(models.into_iter().filter(|m| m.enabled));
            }
        }
        
        Ok(all_models)
    }

    /// 获取供应商的模型列表
    pub async fn get_provider_models(
        &self,
        provider_id: &UUID,
        force_refresh: bool,
    ) -> Result<Vec<Model>, AppError> {
        let provider = self.get_provider(provider_id).await?;
        
        // 如果不强制刷新，先尝试从数据库获取
        if !force_refresh {
            let cached_models = self.repository.get_models_by_provider(provider_id).await?;
            if !cached_models.is_empty() {
                return Ok(cached_models);
            }
        }

        // 使用新的客户端架构获取模型列表
        let client = create_llm_client(&provider.provider_type)?;
        let standard_models = client.list_models(&provider).await?;
        
        // 适配为我们的Model结构
        let now = self.current_timestamp();
        let models: Vec<Model> = standard_models
            .into_iter()
            .map(|standard_model| adapt_model(standard_model, provider.id.clone(), now))
            .collect();

        // 保存到数据库
        if !models.is_empty() {
            self.repository.create_models(&models).await?;
        }

        Ok(models)
    }

    /// 切换供应商启用状态
    pub async fn toggle_provider(
        &self,
        provider_id: &UUID,
        enabled: bool,
    ) -> Result<Provider, AppError> {
        // 先获取供应商
        let mut provider = self.get_provider(provider_id).await?;
        
        // 更新启用状态
        provider.enabled = enabled;
        provider.updated_at = self.current_timestamp();
        
        // 保存到数据库
        self.repository.update_provider(&provider).await?;
        
        Ok(provider)
    }

    /// 切换模型启用状态
    pub async fn toggle_model(
        &self,
        provider_id: &UUID,
        model_id: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        self.repository.toggle_model(provider_id, model_id, enabled).await
    }

    // === 私有辅助方法 ===

    /// 获取当前时间戳
    fn current_timestamp(&self) -> Timestamp {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

}