// 模型服务实现

use crate::models::AppError;
use crate::services::oss_client::OssClient;
use crate::services::Database;
use crate::storage::types::{Model, ModelFeature, ModelModality, Provider, Timestamp, UUID};
use crate::storage::{ModelRepository, ProviderRepository};
use handbox_llm::config::LlmConfigProvider;
use handbox_llm::types::LlmModelParameter;
use handbox_llm::{create_llm_client, LlmModel, LlmModelFeature, LlmModelModality, LlmProvider};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::convert::TryFrom;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const MODEL_SUPPLEMENTS_OBJECT_KEY: &str = "google_models.json";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ModelSupplementPricing {
    prompt: Option<f32>,
    completion: Option<f32>,
    #[serde(rename = "cached_input")]
    cached_input: Option<f32>,
    #[serde(rename = "context_caching")]
    context_caching: Option<f32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ModelSupplementSource {
    #[serde(default)]
    pub scraped_at: Option<String>,
    #[serde(default)]
    pub total_models: Option<u32>,
    #[serde(default)]
    pub detailed_models: Option<u32>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub index_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ModelSupplementMetadata {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub source: Option<ModelSupplementSource>,
    #[serde(default)]
    pub total_models: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ModelSupplementDocument {
    #[serde(default)]
    pub metadata: Option<ModelSupplementMetadata>,
    #[serde(default)]
    pub models: Vec<ModelSupplement>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ModelSupplement {
    pub id: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub output_modalities: Vec<String>,
    #[serde(
        rename = "context_length",
        alias = "content_length",
        alias = "content_lenght"
    )]
    pub context_length: Option<Value>,
    #[serde(default)]
    pub output_max_tokens: Option<Value>,
    #[serde(default)]
    pub pricing: Option<ModelSupplementPricing>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub supported_features: Vec<String>,
    #[serde(default)]
    pub snapshots: Vec<String>,
    #[serde(default)]
    pub endpoints: Vec<String>,
}

pub(crate) async fn fetch_model_supplements_from_oss(
    object_key: &str,
) -> Result<ModelSupplementDocument, AppError> {
    let client = OssClient::from_env()?;
    let content = client.get_object_text(object_key).await?;
    let document: ModelSupplementDocument = serde_json::from_str(&content)
        .map_err(|err| AppError::internal_error(&format!("解析模型补充信息失败: {err}")))?;
    Ok(document)
}

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

    /// 通过 OSS 获取模型补充信息
    pub(crate) async fn fetch_model_supplements(
        &self,
        object_key: &str,
    ) -> Result<ModelSupplementDocument, AppError> {
        fetch_model_supplements_from_oss(object_key).await
    }

    async fn enrich_models_from_oss(
        &self,
        models: &mut [Model],
        object_key: &str,
        provider_type: &str,
    ) {
        match self.fetch_model_supplements(object_key).await {
            Ok(document) => {
                apply_model_supplements(models, &document.models, provider_type);
            }
            Err(err) => tracing::warn!("Failed to load model supplements from OSS: {}", err),
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
        let mut models: Vec<Model> = standard_models
            .into_iter()
            .map(|standard_model| adapt_model(standard_model, provider.id.clone(), now))
            .collect();

        if provider.provider_type.eq_ignore_ascii_case("google") {
            self.enrich_models_from_oss(
                &mut models,
                MODEL_SUPPLEMENTS_OBJECT_KEY,
                &provider.provider_type,
            )
            .await;
        }

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
}

pub(crate) fn apply_model_supplements(
    models: &mut [Model],
    supplements: &[ModelSupplement],
    provider_type: &str,
) {
    if supplements.is_empty() {
        return;
    }

    for model in models.iter_mut() {
        if let Some(supplement) = supplements.iter().find(|item| {
            supplement_matches_provider(item, provider_type)
                && (item.snapshots.iter().any(|snapshot| snapshot == &model.id)
                    || item.id == model.id)
        }) {
            apply_model_supplement_to_model(model, supplement);
        }
    }
}

fn supplement_matches_provider(supplement: &ModelSupplement, provider_type: &str) -> bool {
    match &supplement.provider {
        Some(provider) => provider.eq_ignore_ascii_case(provider_type),
        None => true,
    }
}

fn apply_model_supplement_to_model(model: &mut Model, supplement: &ModelSupplement) {
    if model.context_length.is_none() {
        if let Some(context_length) = supplement
            .context_length
            .as_ref()
            .and_then(|value| value_to_i32(value))
        {
            model.context_length = Some(context_length);
        }
    }

    if model.output_max_tokens.is_none() {
        if let Some(tokens) = supplement
            .output_max_tokens
            .as_ref()
            .and_then(|value| value_to_i32(value))
        {
            model.output_max_tokens = Some(tokens);
        }
    }

    if !supplement.input_modalities.is_empty() {
        if let Some(modalities) = modalities_from_strings(&supplement.input_modalities) {
            model.input_modalities = Some(modalities);
        }
    }

    if !supplement.output_modalities.is_empty() {
        if let Some(modalities) = modalities_from_strings(&supplement.output_modalities) {
            model.output_modalities = Some(modalities);
        }
    }

    if !supplement.supported_features.is_empty() {
        if let Some(features) = features_from_strings(&supplement.supported_features) {
            model.supported_features = Some(features);
        }
    }

    if let Some(pricing) = &supplement.pricing {
        if let Some(pricing_value) = pricing_to_value(pricing) {
            model.pricing = Some(pricing_value);
        }
        if let Some(prompt) = pricing.prompt {
            model.input_cost = Some(prompt);
        }
        if let Some(completion) = pricing.completion {
            model.output_cost = Some(completion);
        }
    }

    if let Some(url) = &supplement.url {
        merge_metadata_value(model, "url", Value::String(url.to_string()));
    }

    if !supplement.endpoints.is_empty() {
        let endpoints_value = Value::Array(
            supplement
                .endpoints
                .iter()
                .map(|item| Value::String(item.clone()))
                .collect(),
        );
        merge_metadata_value(model, "endpoints", endpoints_value);
    }
}

fn modalities_from_strings(values: &[String]) -> Option<Vec<ModelModality>> {
    let mut result = Vec::new();
    for value in values {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => {
                if !result.contains(&ModelModality::Text) {
                    result.push(ModelModality::Text);
                }
            }
            "image" => {
                if !result.contains(&ModelModality::Image) {
                    result.push(ModelModality::Image);
                }
            }
            "file" => {
                if !result.contains(&ModelModality::File) {
                    result.push(ModelModality::File);
                }
            }
            "audio" => {
                if !result.contains(&ModelModality::Audio) {
                    result.push(ModelModality::Audio);
                }
            }
            "video" => {
                if !result.contains(&ModelModality::Video) {
                    result.push(ModelModality::Video);
                }
            }
            _ => {}
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn features_from_strings(values: &[String]) -> Option<Vec<ModelFeature>> {
    let mut result = Vec::new();
    for value in values {
        match value.trim().to_ascii_lowercase().as_str() {
            "reasoning" => {
                if !result.contains(&ModelFeature::Reasoning) {
                    result.push(ModelFeature::Reasoning);
                }
            }
            "tool" | "tools" | "function-calling" | "function_calling" | "function_call" => {
                if !result.contains(&ModelFeature::Tool) {
                    result.push(ModelFeature::Tool);
                }
            }
            _ => {}
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn value_to_i32(value: &Value) -> Option<i32> {
    match value {
        Value::Number(number) => number.as_i64().and_then(|raw| i32::try_from(raw).ok()),
        Value::String(text) => text.trim().parse::<i32>().ok(),
        _ => None,
    }
}

fn pricing_to_value(pricing: &ModelSupplementPricing) -> Option<Value> {
    let mut map = Map::new();

    if let Some(prompt) = pricing.prompt {
        map.insert("prompt".to_string(), Value::from(prompt as f64));
    }
    if let Some(completion) = pricing.completion {
        map.insert("completion".to_string(), Value::from(completion as f64));
    }
    if let Some(cached_input) = pricing.cached_input {
        map.insert("cached_input".to_string(), Value::from(cached_input as f64));
    }
    if let Some(context_caching) = pricing.context_caching {
        map.insert(
            "context_caching".to_string(),
            Value::from(context_caching as f64),
        );
    }

    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
}

fn merge_metadata_value(model: &mut Model, key: &str, value: Value) {
    if key.is_empty() {
        return;
    }

    let mut metadata_map = match model.metadata.take() {
        Some(Value::Object(map)) => map,
        Some(other) => {
            let mut map = Map::new();
            map.insert("raw".to_string(), other);
            map
        }
        None => Map::new(),
    };

    metadata_map.insert(key.to_string(), value);
    model.metadata = Some(Value::Object(metadata_map));
}

/// 将标准模型适配为应用内部的 `Model`
fn adapt_model(standard_model: LlmModel, provider_id: String, now: i64) -> Model {
    let LlmModel {
        id,
        name,
        context_length,
        output_max_tokens,
        input_cost,
        output_cost,
        supported_features,
        description,
        input_modalities,
        output_modalities,
        metadata,
        pricing,
        support_parameters,
        default_parameters,
        max_parameters,
    } = standard_model;

    let supported_features = supported_features
        .map(|features| features.into_iter().filter_map(map_llm_feature).collect());

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
        input_cost,
        output_cost,
        supported_features,
        description,
        input_modalities,
        output_modalities,
        metadata,
        pricing,
        support_parameters,
        default_parameters,
        max_parameters,
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
