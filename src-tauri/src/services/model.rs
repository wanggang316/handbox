// 模型服务实现

use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{Model, ModelModality, Provider, Timestamp, UUID};
use crate::storage::{ChatRepository, ModelRepository, ProviderRepository};
use handbox_llm::config::LlmConfigProvider;
use handbox_llm::types::LlmModelParameter;
use handbox_llm::{create_llm_client, LlmModel, LlmModelModality, LlmProvider};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// 模型服务
#[derive(Clone)]
pub struct ModelService {
    model_repo: ModelRepository,
    provider_repo: ProviderRepository,
    chat_repo: ChatRepository,
    llm_config: Arc<dyn LlmConfigProvider>,
}

impl ModelService {
    /// 创建新的模型服务实例
    pub fn new(db: Arc<Database>, llm_config: Arc<dyn LlmConfigProvider>) -> Self {
        Self {
            model_repo: ModelRepository::new(Arc::clone(&db)),
            provider_repo: ProviderRepository::new(Arc::clone(&db)),
            chat_repo: ChatRepository::new(db),
            llm_config,
        }
    }

    /// 获取所有可用模型（所有启用供应商的启用模型）
    pub async fn get_available_models(&self) -> Result<Vec<Model>, AppError> {
        let providers = self.provider_repo.list_providers().await?;
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
        // 先获取供应商信息
        let provider = self
            .provider_repo
            .get_provider_by_id(provider_id)
            .await?
            .ok_or_else(|| AppError::validation_error("Provider not found"))?;

        tracing::info!(
            "Getting models for provider: {}, force_refresh: {}",
            provider.name,
            force_refresh
        );

        // 如果不强制刷新，先尝试从数据库获取
        if !force_refresh {
            let cached_models = self.model_repo.get_models_by_provider(provider_id).await?;
            // 合并配置文件中的参数信息
            let merged_models = self.merge_model_parameters(cached_models, &provider.provider_type);
            return Ok(merged_models);
        }

        // 强制刷新：从 API 获取最新模型列表
        let client = create_llm_client(&provider.provider_type, Arc::clone(&self.llm_config))
            .map_err(AppError::from)?;
        let context = Self::provider_context(&provider);
        let standard_models = client.list_models(&context).await.map_err(AppError::from)?;

        // 适配为我们的 Model 结构
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
        // 合并配置文件中的参数信息
        let merged_models = self.merge_model_parameters(synced_models, &provider.provider_type);
        Ok(merged_models)
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

    /// 获取所有收藏的模型
    pub async fn get_favorite_models(&self) -> Result<Vec<Model>, AppError> {
        let providers = self.provider_repo.list_providers().await?;
        let mut favorite_models = Vec::new();

        for provider in providers {
            match self.model_repo.get_models_by_provider(&provider.id).await {
                Ok(models) => {
                    favorite_models.extend(models.into_iter().filter(|m| m.favorite));
                }
                Err(_) => continue, // 忽略获取失败的供应商
            }
        }

        Ok(favorite_models)
    }

    // === 私有辅助方法 ===

    /// 构建 LLM Provider 上下文
    fn provider_context(provider: &Provider) -> LlmProvider {
        LlmProvider {
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
        }
    }

    /// 合并模型参数（数据库 + 配置文件）
    fn merge_model_parameters(&self, models: Vec<Model>, provider_type: &str) -> Vec<Model> {
        use crate::config::llm_config::LlmConfig;

        // 尝试将 LlmConfigProvider 转换为 LlmConfig 以访问参数方法
        let llm_config_any = &self.llm_config as &dyn std::any::Any;

        if let Some(config) = llm_config_any.downcast_ref::<LlmConfig>() {
            models
                .into_iter()
                .map(|mut model| {
                    // 从配置获取支持的参数、默认值和最大值
                    let supported_params =
                        config.get_supported_parameters(provider_type, &model.id);
                    let param_defaults = config.get_parameter_defaults(provider_type, &model.id);
                    let param_max = config.get_max_parameters(provider_type, &model.id);

                    // 如果数据库中已有参数配置，则优先使用数据库的
                    // 否则从配置文件构建参数列表
                    if model
                        .support_parameters
                        .as_ref()
                        .map(|params| params.is_empty())
                        .unwrap_or(true)
                    {
                        if !supported_params.is_empty() {
                            let converted_params = supported_params
                                .iter()
                                .map(|param| {
                                    param
                                        .parse::<LlmModelParameter>()
                                        .unwrap_or(LlmModelParameter::Unknown)
                                })
                                .collect::<Vec<_>>();

                            if !converted_params.is_empty() {
                                model.support_parameters = Some(converted_params);
                            }
                        }
                    }

                    if model
                        .default_parameters
                        .as_ref()
                        .map(|params| params.is_empty())
                        .unwrap_or(true)
                    {
                        if !param_defaults.is_empty() {
                            model.default_parameters = Some(param_defaults.clone());
                        }
                    }

                    if model
                        .max_parameters
                        .as_ref()
                        .map(|params| params.is_empty())
                        .unwrap_or(true)
                    {
                        if !param_max.is_empty() {
                            model.max_parameters = Some(param_max.clone());
                        }
                    }

                    model
                })
                .collect()
        } else {
            // 如果无法转换，返回原始模型列表
            models
        }
    }

    /// 获取当前时间戳
    fn current_timestamp(&self) -> Timestamp {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    /// 统计使用指定模型的聊天数量
    pub async fn count_chats_using_model(&self, model_id: &str) -> Result<i32, AppError> {
        self.chat_repo.count_chats_using_model(model_id).await
    }
}

/// 将标准模型适配为应用内部的 `Model`
fn adapt_model(standard_model: LlmModel, provider_id: String, now: i64) -> Model {
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
    } = standard_model;

    let supported_features = supported_features.and_then(|features| {
        let mapped: Vec<String> = features
            .into_iter()
            .filter(|f| !f.trim().is_empty())
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
    use crate::storage::Database;
    use tempfile::tempdir;

    async fn create_service() -> (ModelService, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).await.unwrap();
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider: Arc<dyn LlmConfigProvider> = llm_config.clone();
        (
            ModelService::new(Arc::new(database), llm_config_provider),
            temp_dir,
        )
    }

    #[tokio::test]
    async fn get_available_models_returns_enabled_only() {
        let (service, _dir) = create_service().await;

        // 初始状态下应该没有模型
        let models = service.get_available_models().await.unwrap();
        assert_eq!(models.len(), 0);
    }

    #[tokio::test]
    async fn get_favorite_models_returns_favorites_only() {
        let (service, _dir) = create_service().await;

        // 初始状态下应该没有收藏的模型
        let models = service.get_favorite_models().await.unwrap();
        assert_eq!(models.len(), 0);
    }
}
