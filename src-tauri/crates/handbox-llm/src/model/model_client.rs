// Model client — fetches the model list from each provider's API.
//
// Until 2026-05 this module also orchestrated an OSS-hosted supplement
// merge step that filled in metadata (cost, context window, etc.) the
// provider's API didn't return. That subsystem is gone now: hand-ai's
// `Model` catalog already carries that data and the legacy adapters
// here are being phased out (Phase D of hand-ai integration). Until
// the legacy adapters are deleted, this client just returns the raw
// API model list — UI quality fields (cost, context_window) come from
// `hand_ai_catalog::list_providers` instead.

use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::model::anthropic_adapter::AnthropicFetcher;
use crate::model::google_adapter::GoogleFetcher;
use crate::model::openai_adapter::OpenAIFetcher;
use crate::model::openrouter_adapter::OpenRouterFetcher;
use crate::types::{LlmModel, LlmModelApiType, LlmProvider};
use async_trait::async_trait;
use regex::Regex;
use std::sync::Arc;

/// Strip well-known version/preview suffixes from a model id.
///
/// Patterns matched: `-preview`, `-preview-N(-N)*`, `-exp`, `-exp-N(-N)*`,
/// `-latest`. Returns `None` when nothing matches. Kept available for any
/// adapter that needs to normalize ids for downstream lookups.
#[allow(dead_code)]
fn strip_model_version_suffix(model_id: &str) -> Option<String> {
    let pattern = r"(-preview(?:-\d+)*|-exp(?:-\d+)*|-latest)$";
    let re = Regex::new(pattern).ok()?;
    if re.is_match(model_id) {
        Some(re.replace(model_id, "").to_string())
    } else {
        None
    }
}

/// Trait implemented per provider for fetching the base model list.
#[async_trait]
pub trait ModelFetcher: Send + Sync {
    /// Fetch the base model list from the provider's API.
    /// Providers without a public list endpoint (Anthropic) return Vec::new().
    async fn fetch_base_models(
        &self,
        provider: &LlmProvider,
    ) -> Result<Vec<LlmModel>, LlmClientError>;
}

/// Model client — thin wrapper around a `ModelFetcher`.
pub struct ModelClient {
    fetcher: Box<dyn ModelFetcher>,
}

impl ModelClient {
    pub fn new(
        fetcher: Box<dyn ModelFetcher>,
        _config: Arc<dyn LlmConfigProvider>,
        _provider_type: &str,
    ) -> Self {
        Self { fetcher }
    }

    pub async fn list_models(
        &self,
        provider: &LlmProvider,
        _provider_type: &str,
    ) -> Result<Vec<LlmModel>, LlmClientError> {
        self.fetcher.fetch_base_models(provider).await
    }
}

pub fn create_model_client(
    api_type: LlmModelApiType,
    provider_type: &str,
    config: Arc<dyn LlmConfigProvider>,
) -> Result<ModelClient, LlmClientError> {
    let fetcher: Box<dyn ModelFetcher> = match api_type {
        LlmModelApiType::OpenAI => Box::new(OpenAIFetcher::new()),
        LlmModelApiType::Google => Box::new(GoogleFetcher::new()),
        LlmModelApiType::Anthropic => Box::new(AnthropicFetcher),
        LlmModelApiType::OpenRouter => Box::new(OpenRouterFetcher::new()),
    };
    Ok(ModelClient::new(fetcher, config, provider_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_model_version_suffix() {
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview"),
            Some("gpt-4".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-1234"),
            Some("gpt-4".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-20240101"),
            Some("gpt-4".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-09-2025"),
            Some("gpt-4".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-1-2-3"),
            Some("gpt-4".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("claude-3-exp"),
            Some("claude-3".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("claude-3-exp-5678"),
            Some("claude-3".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("claude-3-exp-01-2025"),
            Some("claude-3".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("gpt-4-latest"),
            Some("gpt-4".to_string())
        );
        assert_eq!(strip_model_version_suffix("gpt-4-turbo"), None);
        assert_eq!(strip_model_version_suffix("gpt-4"), None);
        assert_eq!(
            strip_model_version_suffix("gpt-4-turbo-preview"),
            Some("gpt-4-turbo".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("claude-3-5-sonnet-latest"),
            Some("claude-3-5-sonnet".to_string())
        );
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-09"),
            Some("gpt-4".to_string())
        );
    }
}
