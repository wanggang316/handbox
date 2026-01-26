// 模型服务实现

use crate::models::model::ModelResponse;
use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{Model, ModelModality, Provider, Timestamp, UUID};
use crate::storage::{ChatRepository, ModelRepository, ProviderRepository};
use handbox_llm::config::LlmConfigProvider;
use handbox_llm::{create_llm_client, LlmModel, LlmModelModality, LlmProvider};
use std::collections::HashMap;
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

    /// 获取供应商的模型列表（转换为 ModelResponse 并过滤掉无效模型）
    pub async fn get_provider_models(
        &self,
        provider_id: &UUID,
        refresh_from_remote: bool,
    ) -> Result<Vec<ModelResponse>, AppError> {
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

        // 获取原始模型列表
        let models = if !refresh_from_remote {
            // 从数据库获取缓存
            self.model_repo.get_models_by_provider(provider_id).await?
        } else {
            // 远程刷新：从 API 获取最新模型列表并同步
            self.fetch_and_sync_models(&provider, true).await?;
            // 返回数据库中的模型（包含用户设置的状态）
            self.model_repo.get_models_by_provider(&provider.id).await?
        };

        // 转换为 ModelResponse 并过滤掉 chat_method 为空的模型
        // 传递 provider_type 以支持供应商级别的参数覆盖
        let provider_type = provider.provider_type.clone();
        Ok(models
            .into_iter()
            .map(|model| ModelResponse::from_model_with_provider(model, Some(&provider_type)))
            .filter(|model| model.chat_method.is_some())
            .collect())
    }

    /// 获取单个模型
    pub async fn get_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<Model>, AppError> {
        self.model_repo.get_model(provider_id, model_id).await
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

    /// 批量获取多个供应商的模型列表
    pub async fn get_providers_models_batch(
        &self,
        provider_ids: &[UUID],
        refresh_from_remote: bool,
    ) -> Result<HashMap<UUID, Vec<ModelResponse>>, AppError> {
        if provider_ids.is_empty() {
            return Ok(HashMap::new());
        }

        if !refresh_from_remote {
            // 从数据库批量获取所有模型
            let all_models = self
                .model_repo
                .get_models_by_providers(provider_ids)
                .await?;

            // 按 provider_id 分组
            let mut result: HashMap<UUID, Vec<ModelResponse>> = HashMap::new();
            for provider_id in provider_ids {
                let provider = self
                    .provider_repo
                    .get_provider_by_id(provider_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::validation_error(&format!("Provider {} not found", provider_id))
                    })?;

                let provider_models: Vec<ModelResponse> = all_models
                    .iter()
                    .filter(|m| &m.provider_id == provider_id)
                    .map(|m| {
                        ModelResponse::from_model_with_provider(
                            m.clone(),
                            Some(&provider.provider_type),
                        )
                    })
                    .filter(|m| m.chat_method.is_some())
                    .collect();

                result.insert(provider_id.clone(), provider_models);
            }

            Ok(result)
        } else {
            // 远程刷新：并行获取每个供应商的模型
            use futures::future::join_all;

            let fetch_futures: Vec<_> = provider_ids
                .iter()
                .map(|provider_id| {
                    let provider_id = provider_id.clone();
                    async move {
                        let provider_result =
                            self.provider_repo.get_provider_by_id(&provider_id).await;
                        match provider_result {
                            Ok(Some(provider)) => {
                                match self.fetch_and_sync_models(&provider, true).await {
                                    Ok(()) => {
                                        let models = self
                                            .model_repo
                                            .get_models_by_provider(&provider_id)
                                            .await
                                            .ok();
                                        let provider_type = provider.provider_type.clone();
                                        (provider_id.clone(), models, Some(provider_type))
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to fetch models for {}: {}",
                                            provider_id,
                                            e
                                        );
                                        (provider_id.clone(), None, None)
                                    }
                                }
                            }
                            Ok(None) => (provider_id.clone(), None, None),
                            Err(e) => {
                                tracing::error!("Failed to get provider {}: {}", provider_id, e);
                                (provider_id.clone(), None, None)
                            }
                        }
                    }
                })
                .collect();

            let results = join_all(fetch_futures).await;

            let mut result: HashMap<UUID, Vec<ModelResponse>> = HashMap::new();
            for (provider_id, models_opt, provider_type_opt) in results {
                if let (Some(models), Some(provider_type)) = (models_opt, provider_type_opt) {
                    let model_responses: Vec<ModelResponse> = models
                        .into_iter()
                        .map(|m| ModelResponse::from_model_with_provider(m, Some(&provider_type)))
                        .filter(|m| m.chat_method.is_some())
                        .collect();
                    result.insert(provider_id, model_responses);
                }
            }

            Ok(result)
        }
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
        supported_parameters,
        default_parameters,
        max_parameters,
        supported_methods,
        created_at,
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
        supported_parameters,
        default_parameters,
        max_parameters,
        supported_methods,
        model_created_at: created_at,
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
    use crate::storage::types::{Model, UUID};

    /// 创建一个测试用的 Model，包含 chat_methods
    fn create_test_model_with_chat_methods(id: &str, provider_id: &str) -> Model {
        Model {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            name: format!("Test Model {id}"),
            context_length: Some(4096),
            output_max_tokens: Some(2048),
            supported_features: None,
            description: Some("A test model".to_string()),
            input_modalities: None,
            output_modalities: None,
            metadata: None,
            pricing: None,
            url: None,
            supported_parameters: None,
            default_parameters: None,
            max_parameters: None,
            supported_methods: Some(vec!["completions".to_string()]),
            model_created_at: None,
            enabled: true,
            favorite: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// 创建一个测试用的 Model，不包含 chat_methods
    fn create_test_model_without_chat_methods(id: &str, provider_id: &str) -> Model {
        Model {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            name: format!("Test Model {id}"),
            context_length: Some(4096),
            output_max_tokens: Some(2048),
            supported_features: None,
            description: Some("A test model without chat methods".to_string()),
            input_modalities: None,
            output_modalities: None,
            metadata: None,
            pricing: None,
            url: None,
            supported_parameters: None,
            default_parameters: None,
            max_parameters: None,
            supported_methods: None, // 没有 supported_methods
            model_created_at: None,
            enabled: true,
            favorite: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn test_model_response_conversion_filters_empty_chat_methods() {
        // 测试场景：验证 ModelResponse::from_model 转换和过滤逻辑
        // 1. 有 supported_methods 的模型应该有 chat_methods
        // 2. 没有 supported_methods 但有全局配置支持的模型可能也有 chat_methods
        // 3. 完全没有任何支持的模型应该被过滤掉

        let models = vec![
            create_test_model_with_chat_methods("model1", "provider1"),
            create_test_model_without_chat_methods("model2", "provider1"),
            create_test_model_with_chat_methods("model3", "provider1"),
        ];

        // 转换为 ModelResponse 并过滤掉 chat_method 为 None 的模型
        // （这是 get_provider_models 方法中使用的逻辑）
        let filtered_responses: Vec<ModelResponse> = models
            .into_iter()
            .map(ModelResponse::from_model)
            .filter(|model| model.chat_method.is_some())
            .collect();

        // 验证所有返回的模型都有 chat_method
        for response in &filtered_responses {
            assert!(
                response.chat_method.is_some(),
                "All returned models should have chat_method"
            );
            assert!(
                response.supported_chat_methods.is_some(),
                "All returned models should have supported_chat_methods"
            );
        }

        // 验证过滤逻辑正常工作（至少返回了一些模型）
        assert!(
            !filtered_responses.is_empty(),
            "Should have at least some models with chat_method"
        );
    }

    #[test]
    fn test_model_response_conversion_empty_input() {
        let models: Vec<Model> = vec![];

        // 转换和过滤空列表
        let responses: Vec<ModelResponse> = models
            .into_iter()
            .map(ModelResponse::from_model)
            .filter(|model| model.chat_method.is_some())
            .collect();

        assert_eq!(responses.len(), 0);
    }
}
