// 供应商服务实现

use crate::models::{
    AppError, Model, ModelFeature, Provider, ProviderConfig, ProviderType,
    ProviderWithModels, Timestamp, UUID,
};
use crate::services::{DatabaseService, ProviderRepository};
use crate::services::model_data::{
    get_openai_model_info, get_deepseek_model_info, get_anthropic_model_info,
    parse_openrouter_price, parse_openrouter_features
};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// 供应商服务
pub struct ProviderService {
    repository: ProviderRepository,
}

impl ProviderService {
    pub fn new(db: DatabaseService) -> Self {
        Self { repository: ProviderRepository::new(db) }
    }

    /// 获取所有供应商列表
    pub async fn list_providers(&self) -> Result<Vec<Provider>, AppError> {
        self.repository.list_providers().await
    }

    /// 根据ID获取供应商
    pub async fn get_provider(&self, provider_id: &UUID) -> Result<Provider, AppError> {
        match self.repository.get_provider_by_id(provider_id).await? {
            Some(provider) => Ok(provider),
            None => Err(AppError::not_found("Provider not found")),
        }
    }

    /// 获取带模型的供应商
    pub async fn get_provider_with_models(&self, provider_id: &UUID) -> Result<ProviderWithModels, AppError> {
        match self.repository.get_provider_with_models(provider_id).await? {
            Some(provider) => Ok(provider),
            None => Err(AppError::not_found("Provider not found")),
        }
    }

    /// 创建新供应商
    pub async fn create_provider(&self, config: ProviderConfig) -> Result<Provider, AppError> {
        let now = self.current_timestamp();
        let provider_id = Uuid::new_v4().to_string();

        let provider = Provider {
            id: provider_id.clone(),
            name: config
                .name
                .unwrap_or_else(|| self.get_default_name(&config.provider_type)),
            provider_type: config.provider_type,
            base_url: config.base_url,
            api_key: config.api_key.clone(),
            enabled: config.enabled.unwrap_or(false),
            created_at: now,
            updated_at: now,
        };

        // 创建数据库记录
        self.repository.create_provider(&provider).await?;

        // 自动拉取并保存模型列表
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

        Ok(provider)
    }



    /// 更新供应商配置
    pub async fn update_provider(
        &self,
        provider_id: &UUID,
        config: ProviderConfig,
    ) -> Result<Provider, AppError> {
        let mut provider = self.get_provider(provider_id).await?;
        let now = self.current_timestamp();

        // 检查关键配置是否发生变化（API key, base_url, provider_type）
        let need_refresh_models = provider.api_key != config.api_key 
            || provider.base_url != config.base_url 
            || provider.provider_type != config.provider_type;

        // 更新配置
        if let Some(name) = config.name {
            provider.name = name;
        }
        provider.provider_type = config.provider_type;
        provider.base_url = config.base_url;
        provider.api_key = config.api_key;
        if let Some(enabled) = config.enabled {
            provider.enabled = enabled;
        }
        provider.updated_at = now;

        tracing::info!("Provider {} - Updating provider with data: {:?}", provider_id, provider);
        // 更新数据库记录
        self.repository.update_provider(&provider).await?;

        // 如果关键配置发生变化，自动重新拉取模型列表
        if need_refresh_models {
            tracing::info!("Key configuration changed for provider {}, refreshing models", provider.name);
            match self.get_provider_models(&provider.id, true).await {
                Ok(models) => {
                    tracing::info!("Successfully refreshed {} models for provider {}", models.len(), provider.name);
                }
                Err(e) => {
                    tracing::warn!("Failed to refresh models for provider {}: {}", provider.name, e);
                    // 不让模型获取失败影响供应商更新
                }
            }
        }

        Ok(provider)
    }

    /// 删除供应商
    pub async fn delete_provider(&self, provider_id: &UUID) -> Result<(), AppError> {
        // 删除数据库记录
        self.repository.delete_provider(provider_id).await?;
        Ok(())
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

        // 从API获取最新模型列表
        let models = match provider.provider_type {
            ProviderType::OpenAI | ProviderType::CustomOpenAI => {
                self.get_openai_models(&provider).await?
            }
            ProviderType::Anthropic | ProviderType::CustomAnthropic => {
                self.get_anthropic_models(&provider).await?
            }
            ProviderType::Google => self.get_google_models(&provider).await?,
            ProviderType::DeepSeek => self.get_deepseek_models(&provider).await?,
            ProviderType::OpenRouter => self.get_openrouter_models(&provider).await?,
        };

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
        let mut provider = self.get_provider(provider_id).await?;
        provider.enabled = enabled;
        provider.updated_at = self.current_timestamp();

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

    // 私有辅助方法

    fn current_timestamp(&self) -> Timestamp {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    fn get_default_name(&self, provider_type: &ProviderType) -> String {
        match provider_type {
            ProviderType::OpenAI => "OpenAI".to_string(),
            ProviderType::Anthropic => "Anthropic".to_string(),
            ProviderType::Google => "Google AI".to_string(),
            ProviderType::DeepSeek => "DeepSeek".to_string(),
            ProviderType::OpenRouter => "OpenRouter".to_string(),
            ProviderType::CustomOpenAI => "Custom OpenAI".to_string(),
            ProviderType::CustomAnthropic => "Custom Anthropic".to_string(),
        }
    }

    async fn get_openai_models(&self, provider: &Provider) -> Result<Vec<Model>, AppError> {
        use crate::models::provider::OpenAIModelsResponse;
        
        let client = reqwest::Client::new();
        let url = format!("{}/models", provider.base_url);
        
        tracing::info!("Fetching OpenAI models from: {}", url);
        
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to fetch OpenAI models: {}", e)))?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "OpenAI API returned error {}: {}",
                status, error_text
            )));
        }
        
        let models_response: OpenAIModelsResponse = response
            .json()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to parse OpenAI response: {}", e)))?;
            
        let now = self.current_timestamp();
        let mut result_models = Vec::new();
        
        tracing::info!("Received {} models from OpenAI", models_response.data.len());
        
        for api_model in models_response.data {
            if let Some(model_info) = get_openai_model_info(&api_model.id) {
                let model_name = model_info.name.clone();
                result_models.push(Model {
                    id: api_model.id.clone(),
                    provider_id: provider.id.clone(),
                    name: model_info.name,
                    context_length: model_info.context_length,
                    input_cost: model_info.input_cost,
                    output_cost: model_info.output_cost,
                    supported_features: model_info.supported_features,
                    enabled: false,
                    created_at: now,
                    updated_at: now,
                });
                tracing::debug!("Added model: {} ({})", model_name, api_model.id);
            } else {
                tracing::debug!("Unknown OpenAI model: {}, skipping", api_model.id);
            }
        }
        
        tracing::info!("Successfully processed {} OpenAI models", result_models.len());
        Ok(result_models)
    }

    async fn get_anthropic_models(&self, provider: &Provider) -> Result<Vec<Model>, AppError> {
        // Anthropic 没有公开的模型列表API，使用本地数据库
        let now = self.current_timestamp();
        let known_models = [
            "claude-3-opus-20240229",
            "claude-3-sonnet-20240229", 
            "claude-3-haiku-20240307",
            "claude-3-5-sonnet-20241022",
            "claude-3-5-haiku-20241022",
        ];
        
        let mut result_models = Vec::new();
        
        tracing::info!("Creating Anthropic models from known list");
        
        for model_id in known_models {
            if let Some(model_info) = get_anthropic_model_info(model_id) {
                let model_name = model_info.name.clone();
                result_models.push(Model {
                    id: model_id.to_string(),
                    provider_id: provider.id.clone(),
                    name: model_info.name,
                    context_length: model_info.context_length,
                    input_cost: model_info.input_cost,
                    output_cost: model_info.output_cost,
                    supported_features: model_info.supported_features,
                    enabled: false,
                    created_at: now,
                    updated_at: now,
                });
                tracing::debug!("Added Anthropic model: {} ({})", model_name, model_id);
            }
        }
        
        tracing::info!("Successfully processed {} Anthropic models", result_models.len());
        Ok(result_models)
    }

    async fn get_google_models(&self, provider: &Provider) -> Result<Vec<Model>, AppError> {
        use crate::models::provider::GoogleModelsResponse;
        
        let client = reqwest::Client::new();
        let url = format!("{}/models", provider.base_url);
        
        tracing::info!("Fetching Google models from: {}", url);
        
        let response = client
            .get(&url)
            .query(&[("key", &provider.api_key)])
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to fetch Google models: {}", e)))?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "Google API returned error {}: {}",
                status, error_text
            )));
        }
        
        let models_response: GoogleModelsResponse = response
            .json()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to parse Google response: {}", e)))?;
            
        let now = self.current_timestamp();
        let mut result_models = Vec::new();
        
        tracing::info!("Received {} models from Google", models_response.models.len());
        
        for api_model in models_response.models {
            // 过滤掉不支持 generateContent 的模型
            let supports_generate = api_model.supported_generation_methods
                .as_ref()
                .map(|methods| methods.contains(&"generateContent".to_string()))
                .unwrap_or(false);
                
            if !supports_generate {
                continue;
            }
            
            // 从name字段提取模型ID（去掉 "models/" 前缀）
            let model_id = if api_model.name.starts_with("models/") {
                api_model.name.strip_prefix("models/").unwrap_or(&api_model.name).to_string()
            } else {
                api_model.name.clone()
            };
            
            // 解析模型特性
            let mut features = vec![ModelFeature::Text, ModelFeature::Streaming];
            
            // 判断是否支持函数调用和视觉
            let model_name_lower = api_model.display_name.to_lowercase();
            if !model_name_lower.contains("embedding") && !model_name_lower.contains("text") {
                features.push(ModelFeature::FunctionCalling);
            }
            
            // 大多数Gemini模型支持视觉
            if model_name_lower.contains("pro") || model_name_lower.contains("flash") {
                features.push(ModelFeature::Vision);
            }
            
            // 检查是否支持推理（thinking模式）
            if api_model.thinking.unwrap_or(false) {
                features.push(ModelFeature::Reasoning);
            }
            
            let display_name = api_model.display_name.clone();
            let model_id_clone = model_id.clone();
            result_models.push(Model {
                id: model_id,
                provider_id: provider.id.clone(),
                name: api_model.display_name,
                context_length: api_model.input_token_limit,
                input_cost: None, // Google 的定价结构比较复杂，这里先留空
                output_cost: None,
                supported_features: features,
                enabled: false,
                created_at: now,
                updated_at: now,
            });
            
            tracing::debug!("Added Google model: {} ({})", display_name, model_id_clone);
        }
        
        tracing::info!("Successfully processed {} Google models", result_models.len());
        Ok(result_models)
    }

    async fn get_deepseek_models(&self, provider: &Provider) -> Result<Vec<Model>, AppError> {
        use crate::models::provider::DeepSeekModelsResponse;
        
        let client = reqwest::Client::new();
        let url = format!("{}/models", provider.base_url);
        
        tracing::info!("Fetching DeepSeek models from: {}", url);
        
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to fetch DeepSeek models: {}", e)))?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "DeepSeek API returned error {}: {}",
                status, error_text
            )));
        }
        
        let models_response: DeepSeekModelsResponse = response
            .json()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to parse DeepSeek response: {}", e)))?;
            
        let now = self.current_timestamp();
        let mut result_models = Vec::new();
        
        tracing::info!("Received {} models from DeepSeek", models_response.data.len());
        
        for api_model in models_response.data {
            if let Some(model_info) = get_deepseek_model_info(&api_model.id) {
                let model_name = model_info.name.clone();
                result_models.push(Model {
                    id: api_model.id.clone(),
                    provider_id: provider.id.clone(),
                    name: model_info.name,
                    context_length: model_info.context_length,
                    input_cost: model_info.input_cost,
                    output_cost: model_info.output_cost,
                    supported_features: model_info.supported_features,
                    enabled: false,
                    created_at: now,
                    updated_at: now,
                });
                tracing::debug!("Added model: {} ({})", model_name, api_model.id);
            } else {
                tracing::debug!("Unknown DeepSeek model: {}, skipping", api_model.id);
            }
        }
        
        tracing::info!("Successfully processed {} DeepSeek models", result_models.len());
        Ok(result_models)
    }

    async fn get_openrouter_models(&self, provider: &Provider) -> Result<Vec<Model>, AppError> {
        use crate::models::provider::OpenRouterModelsResponse;
        
        let client = reqwest::Client::new();
        let url = format!("{}/models", provider.base_url);
        
        tracing::info!("Fetching OpenRouter models from: {}", url);
        
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to fetch OpenRouter models: {}", e)))?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "OpenRouter API returned error {}: {}",
                status, error_text
            )));
        }
        
        let models_response: OpenRouterModelsResponse = response
            .json()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to parse OpenRouter response: {}", e)))?;
            
        let now = self.current_timestamp();
        let mut result_models = Vec::new();
        
        tracing::info!("Received {} models from OpenRouter", models_response.data.len());
        
        for api_model in models_response.data {
            // 解析定价信息
            let input_cost = api_model.pricing
                .as_ref()
                .and_then(|p| p.prompt.as_ref())
                .and_then(|price| parse_openrouter_price(price));
                
            let output_cost = api_model.pricing
                .as_ref()
                .and_then(|p| p.completion.as_ref())
                .and_then(|price| parse_openrouter_price(price));
            
            // 解析模型特性
            let modality = api_model.architecture
                .as_ref()
                .and_then(|arch| arch.modality.as_ref())
                .map(|s| s.as_str());
                
            let features = parse_openrouter_features(modality, &api_model.name);
            
            result_models.push(Model {
                id: api_model.id.clone(),
                provider_id: provider.id.clone(),
                name: api_model.name.clone(),
                context_length: api_model.context_length,
                input_cost,
                output_cost,
                supported_features: features,
                enabled: false,
                created_at: now,
                updated_at: now,
            });
            
            tracing::debug!("Added OpenRouter model: {} ({})", api_model.name, api_model.id);
        }
        
        tracing::info!("Successfully processed {} OpenRouter models", result_models.len());
        Ok(result_models)
    }
}