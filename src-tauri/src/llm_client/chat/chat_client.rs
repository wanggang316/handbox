use async_trait::async_trait;
use futures::Stream;

use crate::llm_client::types::{ChatApiType, ChatRequest, ChatResponse};
use crate::models::{AppError, Provider};

use super::{
    anthropic::AnthropicChatClient, google::GoogleChatClient,
    openai_completions::OpenAICompletionsChatClient, openai_responses::OpenAIResponsesChatClient,
};

#[async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError>;

    async fn chat_stream(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<Box<dyn Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>, AppError>;

    fn api_type(&self) -> &'static str;
}

pub fn create_chat_client(api_type: ChatApiType) -> Result<Box<dyn ChatClient>, AppError> {
    Ok(match api_type {
        ChatApiType::OpenAICompletions => Box::new(OpenAICompletionsChatClient::new()) as Box<_>,
        ChatApiType::OpenAIResponses => Box::new(OpenAIResponsesChatClient::new()) as Box<_>,
        ChatApiType::Google => Box::new(GoogleChatClient::new()) as Box<_>,
        ChatApiType::Anthropic => Box::new(AnthropicChatClient::new()) as Box<_>,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_client::types::ChatApiType;

    #[test]
    fn test_create_openai_completions_client() {
        let client = create_chat_client(ChatApiType::OpenAICompletions).unwrap();
        assert_eq!(client.api_type(), "openai-completions");
    }

    #[test]
    fn test_create_openai_responses_client() {
        let client = create_chat_client(ChatApiType::OpenAIResponses).unwrap();
        assert_eq!(client.api_type(), "openai-responses");
    }

    #[test]
    fn test_create_openai_legacy_client() {
        let client = create_chat_client(ChatApiType::OpenAICompletions).unwrap();
        assert_eq!(client.api_type(), "openai-completions");
    }

    #[test]
    fn test_create_google_client() {
        let client = create_chat_client(ChatApiType::Google).unwrap();
        assert_eq!(client.api_type(), "google");
    }

    #[test]
    fn test_create_anthropic_client() {
        let client = create_chat_client(ChatApiType::Anthropic).unwrap();
        assert_eq!(client.api_type(), "anthropic");
    }
}
