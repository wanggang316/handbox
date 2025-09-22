// 供应商服务实现

use crate::llm_client::{adapt_model, create_llm_client};
use crate::models::{
    AddProviderRequest, AppError, Model, Provider, ProviderWithModels, Timestamp, UUID,
};
use crate::services::DatabaseService;
use crate::storage::ProviderRepository;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// 供应商服务
pub struct ProviderService {
    repository: ProviderRepository,
}

impl ProviderService {
    /// 创建新的供应商服务实例
    pub fn new(db: Arc<DatabaseService>) -> Self {
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
    pub async fn create_provider(&self, config: AddProviderRequest) -> Result<Provider, AppError> {
        self.create_provider_with_validation(config, true).await
    }

    /// 创建供应商（可选择是否验证 API Key）
    pub async fn create_provider_with_validation(
        &self,
        config: AddProviderRequest,
        validate_api_key: bool,
    ) -> Result<Provider, AppError> {
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

        // 根据参数决定是否验证 API Key
        if validate_api_key {
            tracing::info!(
                "Validating API key by fetching models for new provider: {}",
                provider.name
            );
            // 对于新供应商，直接获取模型用于验证，先不保存到数据库
            let client = create_llm_client(&provider.provider_type)?;
            match client.list_models(&provider).await {
                Ok(standard_models) => {
                    tracing::info!(
                        "API key validation successful, fetched {} models",
                        standard_models.len()
                    );
                    // API Key 验证成功，创建供应商
                    self.repository.create_provider(&provider).await?;

                    // 然后保存模型（新供应商直接创建，不需要同步状态）
                    if !standard_models.is_empty() {
                        let now = self.current_timestamp();
                        let models: Vec<Model> = standard_models
                            .into_iter()
                            .map(|standard_model| {
                                adapt_model(standard_model, provider.id.clone(), now)
                            })
                            .collect();
                        self.repository.create_models(&models).await?;
                    }

                    tracing::info!(
                        "Successfully created provider and models: {}",
                        provider.name
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to validate API key for provider {}: {}",
                        provider.name,
                        e
                    );
                    // 检查是否是配置相关错误（API Key、端点等）
                    if e.code == "INTERNAL_ERROR"
                        && (e.message.contains("Incorrect API key")
                            || e.message.contains("invalid_api_key")
                            || e.message.contains("API key not valid")
                            || e.message.contains("400 Bad Request")
                            || e.message.contains("401 Unauthorized")
                            || e.message.contains("404 Not Found")
                            || e.message.contains("403 Forbidden"))
                    {
                        // 对于配置错误，直接返回错误，不创建供应商
                        let error = if e.message.contains("404 Not Found") {
                            AppError::provider_api_endpoint_invalid()
                        } else if e.message.contains("403 Forbidden") {
                            AppError::provider_api_permission_denied()
                        } else {
                            AppError::provider_api_key_invalid()
                        };

                        return Err(error);
                    }
                    // 对于其他错误（如网络问题），仍然创建供应商但给出警告
                    self.repository.create_provider(&provider).await?;
                    tracing::warn!("Provider created despite model fetch failure (network or other non-auth error)");
                }
            }
        } else {
            // 不验证 API Key，直接创建供应商
            self.repository.create_provider(&provider).await?;
            tracing::info!("Provider created without API key validation");
        }

        Ok(provider)
    }

    /// 更新供应商
    pub async fn update_provider(
        &self,
        provider_id: &UUID,
        config: AddProviderRequest,
    ) -> Result<Provider, AppError> {
        self.update_provider_with_validation(provider_id, config, true)
            .await
    }

    /// 更新供应商（可选择是否验证 API Key）
    pub async fn update_provider_with_validation(
        &self,
        provider_id: &UUID,
        config: AddProviderRequest,
        validate_api_key: bool,
    ) -> Result<Provider, AppError> {
        let existing_provider = self.get_provider(provider_id).await?;

        // 检查是否有关键配置变更
        let should_refresh_models = config.api_key != existing_provider.api_key
            || config.base_url != existing_provider.base_url
            || config.provider_type != existing_provider.provider_type;

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

        // 如果关键配置有变更且需要验证，先验证 API Key
        if should_refresh_models && validate_api_key {
            tracing::info!(
                "Key configuration changed, validating API key for provider: {}",
                updated_provider.name
            );
            let client = create_llm_client(&updated_provider.provider_type)?;
            match client.list_models(&updated_provider).await {
                Ok(standard_models) => {
                    tracing::info!(
                        "API key validation successful, fetched {} models",
                        standard_models.len()
                    );
                    // API Key 验证成功，更新供应商
                    self.repository.update_provider(&updated_provider).await?;

                    // 同步模型，保留用户状态
                    if !standard_models.is_empty() {
                        let now = self.current_timestamp();
                        let models: Vec<Model> = standard_models
                            .into_iter()
                            .map(|standard_model| {
                                adapt_model(standard_model, updated_provider.id.clone(), now)
                            })
                            .collect();
                        self.repository
                            .sync_provider_models(&updated_provider.id, &models)
                            .await?;
                    }

                    tracing::info!(
                        "Successfully updated provider and synced models: {}",
                        updated_provider.name
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to validate API key for provider {}: {}",
                        updated_provider.name,
                        e
                    );
                    // 检查是否是配置相关错误（API Key、端点等）
                    if e.code == "INTERNAL_ERROR"
                        && (e.message.contains("Incorrect API key")
                            || e.message.contains("invalid_api_key")
                            || e.message.contains("API key not valid")
                            || e.message.contains("400 Bad Request")
                            || e.message.contains("401 Unauthorized")
                            || e.message.contains("404 Not Found")
                            || e.message.contains("403 Forbidden"))
                    {
                        // 对于配置错误，直接返回错误，不更新供应商
                        let error = if e.message.contains("404 Not Found") {
                            AppError::provider_api_endpoint_invalid()
                        } else if e.message.contains("403 Forbidden") {
                            AppError::provider_api_permission_denied()
                        } else {
                            AppError::provider_api_key_invalid()
                        };

                        return Err(error);
                    }
                    // 对于其他错误（如网络问题），仍然更新供应商但给出警告
                    self.repository.update_provider(&updated_provider).await?;
                    tracing::warn!("Provider updated despite model fetch failure (network or other non-auth error)");
                }
            }
        } else {
            // 没有关键配置变更或不需要验证，直接更新供应商
            self.repository.update_provider(&updated_provider).await?;
            if should_refresh_models {
                tracing::info!("Provider updated without API key validation (validation disabled)");
            }
        }

        Ok(updated_provider)
    }

    /// 获取单个供应商
    pub async fn get_provider(&self, provider_id: &UUID) -> Result<Provider, AppError> {
        self.repository
            .get_provider_by_id(provider_id)
            .await?
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

        tracing::info!(
            "Getting provider models for provider: {}, force_refresh: {}",
            provider.name,
            force_refresh
        );
        // 如果不强制刷新，先尝试从数据库获取
        if !force_refresh {
            let cached_models = self.repository.get_models_by_provider(provider_id).await?;
            // if !cached_models.is_empty() {
            return Ok(cached_models);
            // }
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

        // 同步到数据库（保留用户设置的状态）
        self.repository
            .sync_provider_models(&provider.id, &models)
            .await?;

        // 返回数据库中的模型（包含用户设置的状态）
        let synced_models = self.repository.get_models_by_provider(&provider.id).await?;
        Ok(synced_models)
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
        self.repository
            .toggle_model(provider_id, model_id, enabled)
            .await
    }

    /// 切换模型收藏状态
    pub async fn toggle_favorite_model(
        &self,
        provider_id: &UUID,
        model_id: &str,
        favorite: bool,
    ) -> Result<(), AppError> {
        self.repository
            .toggle_favorite_model(provider_id, model_id, favorite)
            .await
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
