// Anthropic 模型适配器实现

use super::model_client::{ModelFetcher, ModelSupplementer};
use crate::error::LlmClientError;
use crate::types::{merge_pricing, LlmModel, LlmProvider, ModelSupplement};
use async_trait::async_trait;
use serde_json::Value;

/// Anthropic 模型数据获取器（没有公开 API）
pub struct AnthropicFetcher;

#[async_trait]
impl ModelFetcher for AnthropicFetcher {
    /// Anthropic 没有公开的模型列表 API，返回空 Vec
    async fn fetch_base_models(
        &self,
        _provider: &LlmProvider,
    ) -> Result<Vec<LlmModel>, LlmClientError> {
        Ok(Vec::new())
    }
}

/// Anthropic 模型补充器
pub struct AnthropicSupplementer;

impl ModelSupplementer for AnthropicSupplementer {
    /// Anthropic 合并策略：完全从 supplement 构建模型
    /// 因为 Anthropic 没有公开 API，所有数据都来自 supplement
    /// 一个 supplement 可能展开为多个模型（如果有 snapshots）
    fn merge_supplement(&self, _model: LlmModel, supplement: &ModelSupplement) -> Vec<LlmModel> {
        let model_code = supplement.model_code.clone();

        // Get snapshot IDs - if no snapshots, use model_code as the single ID
        let snapshot_ids: Vec<String> = if supplement.snapshots.is_empty() {
            vec![model_code.clone()]
        } else {
            supplement.snapshots.clone()
        };

        let mut models = Vec::new();

        for snapshot_id in snapshot_ids.iter() {
            // Build metadata
            let mut metadata_map = supplement
                .metadata
                .as_ref()
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();

            // Add model_code to metadata
            metadata_map.insert("model_code".to_string(), Value::String(model_code.clone()));

            // Add resolved snapshot if we have multiple snapshots
            if !supplement.snapshots.is_empty() {
                metadata_map.insert(
                    "snapshots".to_string(),
                    Value::Array(
                        supplement
                            .snapshots
                            .iter()
                            .cloned()
                            .map(Value::String)
                            .collect(),
                    ),
                );
                metadata_map.insert(
                    "resolved_snapshot".to_string(),
                    Value::String(snapshot_id.clone()),
                );
            }

            // Add endpoints to metadata if present
            if !supplement.endpoints.is_empty() {
                metadata_map.insert(
                    "endpoints".to_string(),
                    Value::Array(
                        supplement
                            .endpoints
                            .iter()
                            .cloned()
                            .map(Value::String)
                            .collect(),
                    ),
                );
            }

            // Add URL to metadata if present
            if let Some(ref url_value) = supplement.url {
                metadata_map.insert("url".to_string(), Value::String(url_value.clone()));
            }

            // Add currency to metadata if present
            if let Some(ref currency_value) = supplement.currency {
                metadata_map.insert(
                    "currency".to_string(),
                    Value::String(currency_value.clone()),
                );
            }

            let mut model = LlmModel {
                id: snapshot_id.clone(),
                name: supplement
                    .name
                    .clone()
                    .unwrap_or_else(|| snapshot_id.clone()),
                context_length: supplement.context_length,
                output_max_tokens: supplement.output_max_tokens,
                supported_features: supplement.supported_features.clone(),
                description: supplement.description.clone(),
                input_modalities: supplement.input_modalities.clone(),
                output_modalities: supplement.output_modalities.clone(),
                metadata: if metadata_map.is_empty() {
                    None
                } else {
                    Some(Value::Object(metadata_map))
                },
                pricing: None,
                url: supplement.url.clone(),
                support_parameters: supplement.support_parameters.clone(),
                default_parameters: supplement.default_parameters.clone(),
                max_parameters: supplement.max_parameters.clone(),
                supported_methods: supplement.supported_methods.clone(),
            };

            // Merge pricing
            let currency = supplement.currency.as_deref().or(Some("USD"));
            merge_pricing(
                &mut model.pricing,
                supplement.input_cost,
                supplement.output_cost,
                currency,
            );

            models.push(model);
        }

        models
    }
}
