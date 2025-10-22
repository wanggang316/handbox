// 供应商服务实现

use crate::models::{AddProviderRequest, AppError};
use crate::services::Database;
use crate::storage::types::{Model, ModelModality, Provider, ProviderWithModels, Timestamp, UUID};
use crate::storage::{ChatRepository, ModelRepository, ProviderRepository};
use handbox_llm::config::LlmConfigProvider;
use handbox_llm::{create_llm_client, LlmModel, LlmModelModality, LlmProvider};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// 供应商服务
#[derive(Clone)]
pub struct ProviderService {
    provider_repo: ProviderRepository,
    model_repo: ModelRepository,
    chat_repo: ChatRepository,
    llm_config: Arc<dyn LlmConfigProvider>,
}

impl ProviderService {
    /// 创建新的供应商服务实例
    pub fn new(db: Arc<Database>, llm_config: Arc<dyn LlmConfigProvider>) -> Self {
        Self {
            provider_repo: ProviderRepository::new(Arc::clone(&db)),
            model_repo: ModelRepository::new(Arc::clone(&db)),
            chat_repo: ChatRepository::new(db),
            llm_config,
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

    fn provider_context(provider: &Provider) -> LlmProvider {
        LlmProvider {
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
        }
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
            let client = create_llm_client(&provider.provider_type, Arc::clone(&self.llm_config))
                .map_err(AppError::from)?;
            let context = ProviderService::provider_context(&provider);
            match client.list_models(&context).await {
                Ok(standard_models) => {
                    tracing::info!(
                        "API key validation successful, fetched {} models",
                        standard_models.len()
                    );
                    // API Key 验证成功，创建供应商
                    self.provider_repo.create_provider(&provider).await?;

                    // 然后保存模型（新供应商直接创建，不需要同步状态）
                    if !standard_models.is_empty() {
                        let now = self.current_timestamp();
                        let models: Vec<Model> = standard_models
                            .into_iter()
                            .map(|standard_model| {
                                adapt_model(standard_model, provider.id.clone(), now)
                            })
                            .collect();
                        self.model_repo.create_models(&models).await?;
                    }

                    tracing::info!(
                        "Successfully created provider and models: {}",
                        provider.name
                    );
                }
                Err(err) => {
                    let e: AppError = err.into();
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
                    self.provider_repo.create_provider(&provider).await?;
                    tracing::warn!("Provider created despite model fetch failure (network or other non-auth error)");
                }
            }
        } else {
            // 不验证 API Key，直接创建供应商
            self.provider_repo.create_provider(&provider).await?;
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
            let client = create_llm_client(
                &updated_provider.provider_type,
                Arc::clone(&self.llm_config),
            )
            .map_err(AppError::from)?;
            let context = ProviderService::provider_context(&updated_provider);
            match client.list_models(&context).await {
                Ok(standard_models) => {
                    tracing::info!(
                        "API key validation successful, fetched {} models",
                        standard_models.len()
                    );
                    // API Key 验证成功，更新供应商
                    self.provider_repo
                        .update_provider(&updated_provider)
                        .await?;

                    // 同步模型，保留用户状态
                    if !standard_models.is_empty() {
                        let now = self.current_timestamp();
                        let models: Vec<Model> = standard_models
                            .into_iter()
                            .map(|standard_model| {
                                adapt_model(standard_model, updated_provider.id.clone(), now)
                            })
                            .collect();
                        self.model_repo
                            .sync_provider_models(&updated_provider.id, &models)
                            .await?;
                    }

                    tracing::info!(
                        "Successfully updated provider and synced models: {}",
                        updated_provider.name
                    );
                }
                Err(err) => {
                    let e: AppError = err.into();
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
                    self.provider_repo
                        .update_provider(&updated_provider)
                        .await?;
                    tracing::warn!("Provider updated despite model fetch failure (network or other non-auth error)");
                }
            }
        } else {
            // 没有关键配置变更或不需要验证，直接更新供应商
            self.provider_repo
                .update_provider(&updated_provider)
                .await?;
            if should_refresh_models {
                tracing::info!("Provider updated without API key validation (validation disabled)");
            }
        }

        Ok(updated_provider)
    }

    /// 获取单个供应商
    pub async fn get_provider(&self, provider_id: &UUID) -> Result<Provider, AppError> {
        self.provider_repo
            .get_provider_by_id(provider_id)
            .await?
            .ok_or_else(|| AppError::validation_error("Provider not found"))
    }

    /// 获取所有供应商
    pub async fn list_providers(&self) -> Result<Vec<Provider>, AppError> {
        self.provider_repo.list_providers().await
    }

    /// 获取供应商及其模型
    pub async fn get_provider_with_models(
        &self,
        provider_id: &UUID,
    ) -> Result<ProviderWithModels, AppError> {
        let provider = self.get_provider(provider_id).await?;
        let models = self.model_repo.get_models_by_provider(provider_id).await?;

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
        self.provider_repo.delete_provider(provider_id).await
    }

    /// 获取所有可用模型
    pub async fn get_available_models(&self) -> Result<Vec<Model>, AppError> {
        // 获取所有启用供应商的模型
        let providers = self.list_providers().await?;
        let mut all_models = Vec::new();

        for provider in providers {
            if provider.enabled {
                let models = self.model_repo.get_models_by_provider(&provider.id).await?;
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
            let cached_models = self.model_repo.get_models_by_provider(provider_id).await?;
            // if !cached_models.is_empty() {
            return Ok(cached_models);
            // }
        }

        // 使用新的客户端架构获取模型列表
        let client = create_llm_client(&provider.provider_type, Arc::clone(&self.llm_config))
            .map_err(AppError::from)?;
        let context = ProviderService::provider_context(&provider);
        let standard_models = client.list_models(&context).await.map_err(AppError::from)?;

        // 适配为我们的Model结构
        let now = self.current_timestamp();
        let models: Vec<Model> = standard_models
            .into_iter()
            .map(|standard_model| adapt_model(standard_model, provider.id.clone(), now))
            .collect();

        // 同步到数据库（保留用户设置的状态）
        self.model_repo
            .sync_provider_models(&provider.id, &models)
            .await?;

        // 返回数据库中的模型（包含用户设置的状态）
        let synced_models = self.model_repo.get_models_by_provider(&provider.id).await?;
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
        self.provider_repo.update_provider(&provider).await?;

        Ok(provider)
    }

    /// 切换模型启用状态
    pub async fn toggle_model(
        &self,
        provider_id: &UUID,
        model_id: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        self.model_repo
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
        self.model_repo
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

    /// 统计使用指定供应商的聊天数量
    pub async fn count_chats_using_provider(&self, provider_id: &str) -> Result<i32, AppError> {
        self.chat_repo.count_chats_using_provider(provider_id).await
    }
}

/// 将标准模型适配为应用内部的 `Model`
fn adapt_model(llm_model: LlmModel, provider_id: String, now: i64) -> Model {
    let LlmModel {
        id,
        name,
        context_length,
        output_max_tokens,
        supported_features,
        description,
        input_modalities,
        output_modalities,
        metadata,
        pricing,
        url,
        support_parameters,
        default_parameters,
        max_parameters,
    } = llm_model;

    let supported_features = supported_features.and_then(|features| {
        let mapped: Vec<String> = features
            .into_iter()
            .filter(|feature| !feature.trim().is_empty())
            .collect();
        if mapped.is_empty() {
            None
        } else {
            Some(mapped)
        }
    });

    let input_modalities = input_modalities.map(|modalities| {
        modalities
            .into_iter()
            .filter_map(map_llm_modality)
            .collect()
    });

    let output_modalities = output_modalities.map(|modalities| {
        modalities
            .into_iter()
            .filter_map(map_llm_modality)
            .collect()
    });

    let support_parameters = if support_parameters.is_empty() {
        None
    } else {
        Some(support_parameters)
    };

    Model {
        id,
        provider_id,
        name,
        context_length,
        output_max_tokens,
        supported_features,
        description,
        input_modalities,
        output_modalities,
        metadata,
        pricing,
        url,
        support_parameters,
        default_parameters,
        max_parameters,
        enabled: true,
        favorite: false,
        created_at: now,
        updated_at: now,
    }
}

fn map_llm_modality(modality: LlmModelModality) -> Option<ModelModality> {
    match modality {
        LlmModelModality::Text => Some(ModelModality::Text),
        LlmModelModality::Image => Some(ModelModality::Image),
        LlmModelModality::Pdf => Some(ModelModality::Pdf),
        LlmModelModality::File => Some(ModelModality::File),
        LlmModelModality::Audio => Some(ModelModality::Audio),
        LlmModelModality::Video => Some(ModelModality::Video),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::llm_config::LlmConfig;
    use crate::models::AddProviderRequest;
    use crate::storage::Database;
    use tempfile::tempdir;

    async fn create_service() -> (ProviderService, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).await.unwrap();
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider: Arc<dyn LlmConfigProvider> = llm_config.clone();
        (
            ProviderService::new(Arc::new(database), llm_config_provider),
            temp_dir,
        )
    }

    #[tokio::test]
    async fn create_provider_stores_record() {
        let (service, _dir) = create_service().await;

        let config = AddProviderRequest {
            name: "Test Provider".to_string(),
            provider_type: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "test-key".to_string(),
            enabled: Some(false),
        };

        let created = service.create_provider(config).await.unwrap();
        let fetched = service.get_provider(&created.id).await.unwrap();

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Test Provider");
        assert_eq!(fetched.provider_type, "anthropic");
    }

    #[tokio::test]
    async fn list_providers_returns_all() {
        let (service, _dir) = create_service().await;

        let configs = vec![
            AddProviderRequest {
                name: "OpenAI Provider".to_string(),
                provider_type: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "key1".to_string(),
                enabled: Some(true),
            },
            AddProviderRequest {
                name: "Anthropic Provider".to_string(),
                provider_type: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                api_key: "key2".to_string(),
                enabled: Some(false),
            },
        ];

        for cfg in configs {
            service.create_provider(cfg).await.unwrap();
        }

        let providers = service.list_providers().await.unwrap();
        assert_eq!(providers.len(), 2);
    }

    #[tokio::test]
    async fn update_provider_changes_fields() {
        let (service, _dir) = create_service().await;

        let provider = service
            .create_provider(AddProviderRequest {
                name: "Original".to_string(),
                provider_type: "google".to_string(),
                base_url: "https://api.google.com".to_string(),
                api_key: "original".to_string(),
                enabled: Some(false),
            })
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

        let updated = service
            .update_provider(
                &provider.id,
                AddProviderRequest {
                    name: "Updated".to_string(),
                    provider_type: "google".to_string(),
                    base_url: "https://updated.google.com".to_string(),
                    api_key: "updated".to_string(),
                    enabled: Some(true),
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.base_url, "https://updated.google.com");
        assert!(updated.enabled);
        assert!(updated.updated_at > provider.updated_at);
    }

    #[tokio::test]
    async fn delete_provider_removes_record() {
        let (service, _dir) = create_service().await;

        let provider = service
            .create_provider(AddProviderRequest {
                name: "To Delete".to_string(),
                provider_type: "deepseek".to_string(),
                base_url: "https://api.deepseek.com".to_string(),
                api_key: "delete".to_string(),
                enabled: Some(true),
            })
            .await
            .unwrap();

        service.delete_provider(&provider.id).await.unwrap();
        assert!(service.get_provider(&provider.id).await.is_err());
    }

    #[tokio::test]
    async fn toggle_provider_updates_flag() {
        let (service, _dir) = create_service().await;

        let provider = service
            .create_provider(AddProviderRequest {
                name: "Toggle".to_string(),
                provider_type: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                api_key: "toggle".to_string(),
                enabled: Some(false),
            })
            .await
            .unwrap();

        assert!(!provider.enabled);
        assert!(
            service
                .toggle_provider(&provider.id, true)
                .await
                .unwrap()
                .enabled
        );
        assert!(
            !service
                .toggle_provider(&provider.id, false)
                .await
                .unwrap()
                .enabled
        );
    }
}
