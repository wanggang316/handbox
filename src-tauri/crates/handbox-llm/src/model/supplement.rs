// Model supplement system - loading model supplement data from OSS
//
// This module provides a simple system for loading supplement data from OSS files.
// Each adapter is responsible for implementing its own merge logic.

use super::oss_client::OssClient;
use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::types::{ModelSupplement, ModelSupplementDocument};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for loading model supplement data from various sources
#[async_trait]
pub trait SupplementProvider: Send + Sync {
    /// Load supplement data for a given provider type
    /// Returns a map of model_id -> ModelSupplement
    async fn load_supplements(
        &self,
        provider_type: &str,
    ) -> Result<Option<HashMap<String, ModelSupplement>>, LlmClientError>;
}

/// Default supplement provider that loads from OSS based on config
pub struct OssSupplementProvider {
    config: Arc<dyn LlmConfigProvider>,
}

impl OssSupplementProvider {
    pub fn new(config: Arc<dyn LlmConfigProvider>) -> Self {
        Self { config }
    }

    /// Get the supplement file path from provider config
    fn get_supplement_file(&self, provider_type: &str) -> Option<String> {
        self.config
            .get_provider_config(provider_type)
            .and_then(|cfg| cfg.supplement_file)
            .and_then(|file| {
                let trimmed = file.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
    }

    /// Download and parse supplement document from OSS
    async fn download_supplement_document(
        &self,
        supplement_file: &str,
        provider_type: &str,
    ) -> Result<ModelSupplementDocument, LlmClientError> {
        let client = OssClient::from_env().map_err(|err| {
            tracing::error!(
                "OSS config not available for {} supplement '{}': {}",
                provider_type,
                supplement_file,
                err
            );
            LlmClientError::unexpected(format!("OSS client unavailable: {err}"))
        })?;

        let content = client
            .get_object_text(supplement_file)
            .await
            .map_err(|err| {
                tracing::error!(
                    "Unable to download supplement '{}' for {}: {}",
                    supplement_file,
                    provider_type,
                    err
                );
                LlmClientError::unexpected(format!("Failed to download supplement: {err}"))
            })?;

        tracing::info!(
            "Downloaded supplement file '{}' for provider '{}', content length: {} bytes",
            supplement_file,
            provider_type,
            content.len()
        );
        tracing::debug!(
            "Supplement file '{}' content:\n{}",
            supplement_file,
            content
        );

        serde_json::from_str(&content).map_err(|err| {
            tracing::error!(
                "Unable to parse supplement '{}' for {}: {}",
                supplement_file,
                provider_type,
                err
            );
            LlmClientError::unexpected(format!("Failed to parse supplement JSON: {err}"))
        })
    }

    /// Convert supplement document to supplement map
    fn document_to_supplement_map(
        document: ModelSupplementDocument,
    ) -> HashMap<String, ModelSupplement> {
        let mut supplements = HashMap::new();

        tracing::info!(
            "Converting supplement document to map, total models: {}",
            document.models.len()
        );

        // 打印前3个模型的详细信息作为示例
        for (idx, supplement) in document.models.iter().enumerate().take(3) {
            tracing::info!(
                "Sample supplement #{}: model_code='{}', url={:?}, input_cost={:?}, output_cost={:?}, currency={:?}",
                idx + 1,
                supplement.model_code,
                supplement.url,
                supplement.input_cost,
                supplement.output_cost,
                supplement.currency
            );
        }

        for supplement in document.models {
            // Use model_code as the key
            supplements.insert(supplement.model_code.clone(), supplement);
        }

        tracing::info!(
            "Supplement map created with {} entries",
            supplements.len()
        );

        supplements
    }
}

#[async_trait]
impl SupplementProvider for OssSupplementProvider {
    async fn load_supplements(
        &self,
        provider_type: &str,
    ) -> Result<Option<HashMap<String, ModelSupplement>>, LlmClientError> {
        let supplement_file = match self.get_supplement_file(provider_type) {
            Some(file) => {
                tracing::info!(
                    "Loading supplement file for provider '{}': {}",
                    provider_type,
                    file
                );
                file
            }
            None => {
                tracing::info!(
                    "No supplement file configured for provider '{}'",
                    provider_type
                );
                return Ok(None);
            }
        };

        match self
            .download_supplement_document(&supplement_file, provider_type)
            .await
        {
            Ok(document) if document.models.is_empty() => {
                tracing::warn!(
                    "Supplement document for provider '{}' is empty",
                    provider_type
                );
                Ok(None)
            }
            Ok(document) => Ok(Some(Self::document_to_supplement_map(document))),
            Err(err) => {
                tracing::error!(
                    "Failed to download/parse supplement for provider '{}': {:?}",
                    provider_type,
                    err
                );
                Ok(None) // Return None to continue without supplements
            }
        }
    }
}
