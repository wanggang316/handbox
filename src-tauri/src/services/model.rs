// 模型服务实现

use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{Model, ModelFeature, ModelModality, Provider, Timestamp, UUID};
use crate::storage::{ModelRepository, ProviderRepository};
use handbox_llm::config::LlmConfigProvider;
use handbox_llm::{
    create_llm_client, LlmModelFeature, LlmModelModality, LlmProvider, LlmStandardModel,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// 模型服务
#[derive(Clone)]
pub struct ModelService {
    model_repo: ModelRepository,
    provider_repo: ProviderRepository,
    llm_config: Arc<dyn LlmConfigProvider>,
}

impl ModelService {
    /// 创建新的模型服务实例
    pub fn new(db: Arc<Database>, llm_config: Arc<dyn LlmConfigProvider>) -> Self {
        Self {
            model_repo: ModelRepository::new(Arc::clone(&db)),
            provider_repo: ProviderRepository::new(db),
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
            return Ok(cached_models);
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

    /// 获取当前时间戳
    fn current_timestamp(&self) -> Timestamp {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

/// 将标准模型适配为应用内部的 `Model`
fn adapt_model(standard_model: LlmStandardModel, provider_id: String, now: i64) -> Model {
    use crate::storage::types::ModelParameter;

    let supported_features = standard_model
        .supported_features
        .map(|features| features.into_iter().filter_map(map_llm_feature).collect());

    let input_modalities = standard_model.input_modalities.map(|modalities| {
        modalities
            .into_iter()
            .filter_map(map_llm_modality)
            .collect()
    });

    let output_modalities = standard_model.output_modalities.map(|modalities| {
        modalities
            .into_iter()
            .filter_map(map_llm_modality)
            .collect()
    });

    let parameters = standard_model.parameters.map(|params| {
        params
            .into_iter()
            .map(|p| ModelParameter {
                name: p.name,
                default: p.default,
                min: p.min,
                max: p.max,
            })
            .collect()
    });

    Model {
        id: standard_model.id,
        provider_id,
        name: standard_model.name,
        context_length: standard_model.context_length,
        output_token_limit: standard_model.output_token_limit,
        input_cost: standard_model.input_cost,
        output_cost: standard_model.output_cost,
        supported_features,
        description: standard_model.description,
        input_modalities,
        output_modalities,
        metadata: standard_model.metadata,
        pricing: standard_model.pricing,
        parameters,
        enabled: true,
        favorite: false,
        created_at: now,
        updated_at: now,
    }
}

fn map_llm_feature(feature: LlmModelFeature) -> Option<ModelFeature> {
    match feature {
        LlmModelFeature::Reasoning => Some(ModelFeature::Reasoning),
        LlmModelFeature::Tool => Some(ModelFeature::Tool),
    }
}

fn map_llm_modality(modality: LlmModelModality) -> Option<ModelModality> {
    match modality {
        LlmModelModality::Text => Some(ModelModality::Text),
        LlmModelModality::Image => Some(ModelModality::Image),
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
