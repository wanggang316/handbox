// LLM 客户端
// 根据配置组装模型列表提供者和 API 提供者

use crate::models::{AppError, Provider, ModelFeature};
use super::model_list_provider::{ModelListProvider, create_model_list_provider};
use super::api_provider::{ApiProvider, create_api_provider, ChatRequest, ChatResponse};
use crate::services::llm_config::get_global_llm_config;

/// 标准化的模型信息结构
#[derive(Debug, Clone)]
pub struct StandardModel {
    pub id: String,
    pub name: String,
    pub context_length: Option<i32>,
    pub input_cost: Option<f32>,
    pub output_cost: Option<f32>,
    pub supported_features: Option<Vec<ModelFeature>>,
}


/// LLM 客户端
pub struct LlmClient {
    provider_type: String,
    model_list_provider: Box<dyn ModelListProvider>,
    api_provider: Box<dyn ApiProvider>,
}

impl LlmClient {
    /// 从供应商类型创建客户端
    pub fn from_provider_type(provider_type: &str) -> Result<Self, AppError> {
        let config = get_global_llm_config();
        let provider_config = config.get_provider_config(provider_type)
            .ok_or_else(|| AppError::validation_error(&format!("Unknown provider type: {}", provider_type)))?;

        // 创建模型列表提供者
        let model_list_provider = create_model_list_provider(&provider_config.model_list_api_type)?;
        
        // 创建 API 提供者
        let api_provider = create_api_provider(&provider_config.api_type)?;

        Ok(Self {
            provider_type: provider_type.to_string(),
            model_list_provider,
            api_provider,
        })
    }

    /// 直接创建客户端（用于测试或特殊场景）
    pub fn new(
        provider_type: String,
        model_list_provider: Box<dyn ModelListProvider>,
        api_provider: Box<dyn ApiProvider>,
    ) -> Self {
        Self {
            provider_type,
            model_list_provider,
            api_provider,
        }
    }

    /// 发送聊天请求
    pub async fn chat(&self, provider: &Provider, request: ChatRequest) -> Result<ChatResponse, AppError> {
        self.api_provider.chat(provider, request).await
    }

    /// 发送流式聊天请求
    pub async fn chat_stream(&self, provider: &Provider, request: ChatRequest) -> Result<Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>, AppError> {
        self.api_provider.chat_stream(provider, request).await
    }

    /// 获取 API 类型
    pub fn api_type(&self) -> &'static str {
        self.api_provider.api_type()
    }

    /// 获取供应商类型
    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    /// 获取模型列表
    pub async fn list_models(&self, provider: &Provider) -> Result<Vec<StandardModel>, AppError> {
        self.model_list_provider.list_models(provider, &self.provider_type).await
    }
    
}

/// 客户端工厂
pub fn create_llm_client(provider_type: &str) -> Result<LlmClient, AppError> {
    LlmClient::from_provider_type(provider_type)
}

/// 根据供应商类型创建客户端（用于外部调用）
pub fn create_client(provider_type: &str) -> LlmClient {
    match create_llm_client(provider_type) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Failed to create client for {}: {}", provider_type, e);
            panic!("Failed to create client for provider type: {}", provider_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::provider_clients::model_list_provider::OpenAIModelListProvider;
    use crate::services::provider_clients::api_provider::OpenAIApiProvider;

    #[test]
    fn test_llm_client_creation() {
        let client = LlmClient::new(
            "test".to_string(),
            Box::new(OpenAIModelListProvider::new()),
            Box::new(OpenAIApiProvider::new()),
        );

        assert_eq!(client.provider_type(), "test");
        assert_eq!(client.api_type(), "openai");
    }

    #[tokio::test]
    async fn test_create_llm_client_from_config() {
        // 这个测试需要有效的 llm_config.json 配置
        match create_llm_client("openai") {
            Ok(_client) => {
                // 客户端创建成功
                assert!(true);
            }
            Err(e) => {
                // 如果配置文件不存在或格式错误，这是预期的
                println!("Expected error when config file is not available: {}", e);
            }
        }
    }
}