use async_trait::async_trait;
use futures::Stream;

use crate::error::LlmClientError;
use crate::types::{LlmApiType, LlmChunkResponse, LlmProvider, LlmRequest, LlmResponse};

use super::{
    anthropic_adapter::AnthropicChatClient, google_adapter::GoogleChatClient,
    openai_completions_adapter::OpenAICompletionsChatClient,
    openai_responses_adapter::OpenAIResponsesChatClient,
};

#[async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<LlmResponse, LlmClientError>;

    async fn chat_stream(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<
        Box<dyn Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin>,
        LlmClientError,
    >;

    fn api_type(&self) -> &'static str;
}

pub fn create_chat_client(api_type: LlmApiType) -> Result<Box<dyn ChatClient>, LlmClientError> {
    Ok(match api_type {
        LlmApiType::OpenAICompletions => Box::new(OpenAICompletionsChatClient::new()) as Box<_>,
        LlmApiType::OpenAIResponses => Box::new(OpenAIResponsesChatClient::new()) as Box<_>,
        LlmApiType::Google => Box::new(GoogleChatClient::new()) as Box<_>,
        LlmApiType::Anthropic => Box::new(AnthropicChatClient::new()) as Box<_>,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LlmApiType;

    #[test]
    fn test_create_openai_completions_client() {
        let client = create_chat_client(LlmApiType::OpenAICompletions).unwrap();
        assert_eq!(client.api_type(), "openai-completions");
    }

    #[test]
    fn test_create_openai_responses_client() {
        let client = create_chat_client(LlmApiType::OpenAIResponses).unwrap();
        assert_eq!(client.api_type(), "openai-responses");
    }

    #[test]
    fn test_create_openai_legacy_client() {
        let client = create_chat_client(LlmApiType::OpenAICompletions).unwrap();
        assert_eq!(client.api_type(), "openai-completions");
    }

    #[test]
    fn test_create_google_client() {
        let client = create_chat_client(LlmApiType::Google).unwrap();
        assert_eq!(client.api_type(), "google");
    }

    #[test]
    fn test_create_anthropic_client() {
        let client = create_chat_client(LlmApiType::Anthropic).unwrap();
        assert_eq!(client.api_type(), "anthropic");
    }
}
