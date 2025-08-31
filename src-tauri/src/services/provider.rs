// 供应商服务实现

use crate::models::{
    AppError, Model, ModelFeature, Provider, ProviderConfig, ProviderType,
    ProviderWithModels, Timestamp, UUID,
};
use crate::services::{DatabaseService, ProviderRepository};
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
        let now = self.current_timestamp();
        // TODO: 实际从OpenAI API获取模型列表
        Ok(vec![
            Model {
                id: "gpt-4".to_string(),
                provider_id: provider.id.clone(),
                name: "GPT-4".to_string(),
                context_length: Some(8192),
                input_cost: Some(0.03),
                output_cost: Some(0.06),
                supported_features: vec![
                    ModelFeature::Text,
                    ModelFeature::Vision,
                    ModelFeature::FunctionCalling,
                ],
                enabled: false,
                created_at: now,
                updated_at: now,
            },
            Model {
                id: "gpt-3.5-turbo".to_string(),
                provider_id: provider.id.clone(),
                name: "GPT-3.5 Turbo".to_string(),
                context_length: Some(4096),
                input_cost: Some(0.0015),
                output_cost: Some(0.002),
                supported_features: vec![
                    ModelFeature::Text,
                    ModelFeature::FunctionCalling,
                ],
                enabled: false,
                created_at: now,
                updated_at: now,
            },
        ])
    }

    async fn get_anthropic_models(&self, provider: &Provider) -> Result<Vec<Model>, AppError> {
        let now = self.current_timestamp();
        // TODO: 实际从Anthropic API获取模型列表
        Ok(vec![Model {
            id: "claude-3-opus".to_string(),
            provider_id: provider.id.clone(),
            name: "Claude 3 Opus".to_string(),
            context_length: Some(200000),
            input_cost: Some(15.0),
            output_cost: Some(75.0),
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
            ],
            enabled: false,
            created_at: now,
            updated_at: now,
        }])
    }

    async fn get_google_models(&self, provider: &Provider) -> Result<Vec<Model>, AppError> {
        let now = self.current_timestamp();
        // TODO: 实际从Google API获取模型列表
        Ok(vec![Model {
            id: "gemini-pro".to_string(),
            provider_id: provider.id.clone(),
            name: "Gemini Pro".to_string(),
            context_length: Some(32768),
            input_cost: None,
            output_cost: None,
            supported_features: vec![
                ModelFeature::Text,
                ModelFeature::Vision,
                ModelFeature::FunctionCalling,
            ],
            enabled: false,
            created_at: now,
            updated_at: now,
        }])
    }

    async fn get_deepseek_models(&self, _provider: &Provider) -> Result<Vec<Model>, AppError> {
        // TODO: 实际从DeepSeek API获取模型列表
        Ok(vec![])
    }

    async fn get_openrouter_models(&self, _provider: &Provider) -> Result<Vec<Model>, AppError> {
        // TODO: 实际从OpenRouter API获取模型列表
        Ok(vec![])
    }
}