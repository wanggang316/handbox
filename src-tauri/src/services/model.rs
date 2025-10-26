// 模型服务实现

use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{Model, ModelModality, Provider, Timestamp, UUID};
use crate::storage::{ChatRepository, ModelRepository, ProviderRepository};
use handbox_llm::config::LlmConfigProvider;
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

    /// 从远程 API 获取模型并保存到数据库
    ///
    /// # 参数
    /// - `provider`: 供应商信息
    /// - `sync`: true = 同步模型（保留用户状态），false = 创建新模型
    pub(crate) async fn fetch_and_sync_models(
        &self,
        provider: &Provider,
        sync: bool,
    ) -> Result<(), AppError> {
        tracing::info!("Fetching models from API for provider: {}", provider.name);

        let client = create_llm_client(&provider.provider_type, Arc::clone(&self.llm_config))
            .map_err(AppError::from)?;
        let context = Self::provider_context(provider);

        // 获取模型列表，使用友好的错误转换
        let llm_models = client
            .list_models(&context)
            .await
            .map_err(AppError::from_llm_fetch_error)?;

        tracing::info!(
            "Successfully fetched {} models for provider: {}",
            llm_models.len(),
            provider.name
        );

        // 适配为我们的 Model 结构
        let now = self.current_timestamp();
        let models: Vec<Model> = llm_models
            .into_iter()
            .map(|llm_model| adapt_model(llm_model, provider.id.clone(), now))
            .collect();

        // 保存或同步模型
        if !models.is_empty() {
            if sync {
                // 同步模型，保留用户状态
                self.model_repo
                    .sync_provider_models(&provider.id, &models)
                    .await?;
                tracing::info!(
                    "Successfully synced {} models for provider: {}",
                    models.len(),
                    provider.name
                );
            } else {
                // 创建新模型
                self.model_repo.create_models(&models).await?;
                tracing::info!(
                    "Successfully created {} models for provider: {}",
                    models.len(),
                    provider.name
                );
            }
        }

        Ok(())
    }

    /// 获取供应商的模型列表
    pub async fn get_provider_models(
        &self,
        provider_id: &UUID,
        refresh_from_remote: bool,
    ) -> Result<Vec<Model>, AppError> {
        // 先获取供应商信息
        let provider = self
            .provider_repo
            .get_provider_by_id(provider_id)
            .await?
            .ok_or_else(|| AppError::validation_error("Provider not found"))?;

        tracing::info!(
            "Getting models for provider: {}, refresh_from_remote: {}",
            provider.name,
            refresh_from_remote
        );

        // 如果不拉取远程，先尝试从数据库获取
        if !refresh_from_remote {
            let cached_models = self.model_repo.get_models_by_provider(provider_id).await?;
            return Ok(cached_models);
        }

        // 远程刷新：从 API 获取最新模型列表并同步
        self.fetch_and_sync_models(&provider, true).await?;

        // 返回数据库中的模型（包含用户设置的状态）
        let synced_models = self.model_repo.get_models_by_provider(&provider.id).await?;
        Ok(synced_models)
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

    /// 构建 LLM Provider 上下文
    fn provider_context(provider: &Provider) -> LlmProvider {
        LlmProvider {
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
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
pub(crate) fn adapt_model(llm_model: LlmModel, provider_id: String, now: i64) -> Model {
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
        supported_methods,
    } = llm_model;

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

    let supported_methods = supported_methods.and_then(|methods| {
        if methods.is_empty() {
            None
        } else {
            Some(methods)
        }
    });

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
        supported_methods,
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
