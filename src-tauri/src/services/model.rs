use crate::models::model::ModelResponse;
use crate::models::AppError;
use crate::services::model_runtime;
use crate::services::Database;
use crate::storage::types::{Model, Provider, UUID};
use crate::storage::{ModelRepository, ProviderRepository};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ModelService {
    model_repo: ModelRepository,
    provider_repo: ProviderRepository,
}

impl ModelService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            model_repo: ModelRepository::new(Arc::clone(&db)),
            provider_repo: ProviderRepository::new(db),
        }
    }

    /// Load models for `provider` from hand-ai's static catalog and persist them.
    /// No network request is made, so model ids absent from the catalog won't appear.
    /// `sync`: true = sync preserving user state, false = create new rows.
    pub(crate) async fn fetch_and_sync_models(
        &self,
        provider: &Provider,
        sync: bool,
    ) -> Result<(), AppError> {
        tracing::info!(
            "Loading catalog models from hand-ai for provider: {}",
            provider.name
        );

        let catalog_models = model_runtime::list_catalog_models(&provider.provider_type);

        // Catalog miss (misspelled provider_type, provider not yet in hand-ai, etc.):
        // warn and keep existing DB rows untouched.
        if catalog_models.is_empty() {
            tracing::warn!(
                provider_name = %provider.name,
                provider_type = %provider.provider_type,
                "hand-ai catalog returned 0 models; existing DB rows preserved"
            );
            return Ok(());
        }

        // Override the catalog's placeholder provider_id with the app-level one.
        let models: Vec<Model> = catalog_models
            .into_iter()
            .map(|mut model| {
                model.provider_id = provider.id.clone();
                model
            })
            .collect();

        if sync {
            self.model_repo
                .sync_provider_models(&provider.id, &models)
                .await?;
            tracing::info!(
                "Successfully synced {} models for provider: {}",
                models.len(),
                provider.name
            );
        } else {
            self.model_repo.create_models(&models).await?;
            tracing::info!(
                "Successfully created {} models for provider: {}",
                models.len(),
                provider.name
            );
        }

        Ok(())
    }

    /// List a provider's models as `ModelResponse`, dropping models without a chat method.
    pub async fn get_provider_models(
        &self,
        provider_id: &UUID,
        refresh_from_remote: bool,
    ) -> Result<Vec<ModelResponse>, AppError> {
        let provider = self
            .provider_repo
            .get_provider_by_id(provider_id)
            .await?
            .ok_or_else(|| AppError::validation_error("Provider not found"))?;

        tracing::info!(
            "Getting models for provider: {}, refresh_from_remote: {}",
            provider.name,
            refresh_from_remote
        );

        let models = if !refresh_from_remote {
            self.model_repo.get_models_by_provider(provider_id).await?
        } else {
            self.fetch_and_sync_models(&provider, true).await?;
            // Re-read from the DB so user state (enabled/favorite) survives the sync.
            self.model_repo.get_models_by_provider(&provider.id).await?
        };

        // provider_type carries provider-level parameter overrides.
        let provider_type = provider.provider_type.clone();
        Ok(models
            .into_iter()
            .map(|model| ModelResponse::from_model_with_provider(model, Some(&provider_type)))
            .filter(|model| model.chat_method.is_some())
            .collect())
    }

    pub async fn get_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<Model>, AppError> {
        self.model_repo.get_model(provider_id, model_id).await
    }

    /// Manually registers a model on a custom (openai-/anthropic-compatible) provider.
    ///
    /// Custom endpoints are absent from the hand-ai catalog, so `fetch_and_sync_models`
    /// yields nothing for them; `model_runtime::resolve_model` synthesizes the `Model`
    /// template from the protocol plus the provider base_url at request time. Restricted
    /// to custom providers so catalog-backed providers cannot gain phantom models.
    pub async fn add_manual_model(
        &self,
        provider_id: &UUID,
        model_id: &str,
        name: Option<String>,
    ) -> Result<ModelResponse, AppError> {
        let provider = self
            .provider_repo
            .get_provider_by_id(provider_id)
            .await?
            .ok_or_else(|| AppError::validation_error("Provider not found"))?;

        let supported_methods = model_runtime::custom_provider_supported_methods(
            &provider.provider_type,
        )
        .ok_or_else(|| {
            AppError::validation_error(
                "Manual model entry is only supported for custom providers \
                 (openai-compatible / anthropic-compatible)",
            )
        })?;

        let model_id = model_id.trim();
        if model_id.is_empty() {
            return Err(AppError::validation_error("Model id cannot be empty"));
        }

        let display_name = name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| model_id.to_string());

        let now = Self::current_timestamp();
        let model = Model {
            id: model_id.to_string(),
            provider_id: provider.id.clone(),
            name: display_name,
            context_length: None,
            output_max_tokens: None,
            supported_features: None,
            description: None,
            input_modalities: None,
            output_modalities: None,
            metadata: None,
            pricing: None,
            url: None,
            supported_parameters: None,
            default_parameters: None,
            max_parameters: None,
            supported_methods: Some(supported_methods),
            model_created_at: None,
            enabled: true,
            favorite: false,
            created_at: now,
            updated_at: now,
        };

        // INSERT OR REPLACE — re-adding the same id is idempotent.
        self.model_repo
            .create_models(std::slice::from_ref(&model))
            .await?;

        tracing::info!(
            "Manually added model '{}' to custom provider '{}'",
            model.id,
            provider.name
        );

        Ok(ModelResponse::from_model_with_provider(
            model,
            Some(&provider.provider_type),
        ))
    }

    fn current_timestamp() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

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

    pub async fn get_providers_models_batch(
        &self,
        provider_ids: &[UUID],
        refresh_from_remote: bool,
    ) -> Result<HashMap<UUID, Vec<ModelResponse>>, AppError> {
        if provider_ids.is_empty() {
            return Ok(HashMap::new());
        }

        if !refresh_from_remote {
            let all_models = self
                .model_repo
                .get_models_by_providers(provider_ids)
                .await?;

            let mut result: HashMap<UUID, Vec<ModelResponse>> = HashMap::new();
            for provider_id in provider_ids {
                let provider = self
                    .provider_repo
                    .get_provider_by_id(provider_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::validation_error(&format!("Provider {} not found", provider_id))
                    })?;

                let provider_models: Vec<ModelResponse> = all_models
                    .iter()
                    .filter(|m| &m.provider_id == provider_id)
                    .map(|m| {
                        ModelResponse::from_model_with_provider(
                            m.clone(),
                            Some(&provider.provider_type),
                        )
                    })
                    .filter(|m| m.chat_method.is_some())
                    .collect();

                result.insert(provider_id.clone(), provider_models);
            }

            Ok(result)
        } else {
            // Refresh path: sync every provider concurrently.
            use futures::future::join_all;

            let fetch_futures: Vec<_> = provider_ids
                .iter()
                .map(|provider_id| {
                    let provider_id = provider_id.clone();
                    async move {
                        let provider_result =
                            self.provider_repo.get_provider_by_id(&provider_id).await;
                        match provider_result {
                            Ok(Some(provider)) => {
                                match self.fetch_and_sync_models(&provider, true).await {
                                    Ok(()) => {
                                        let models = self
                                            .model_repo
                                            .get_models_by_provider(&provider_id)
                                            .await
                                            .ok();
                                        let provider_type = provider.provider_type.clone();
                                        (provider_id.clone(), models, Some(provider_type))
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to fetch models for {}: {}",
                                            provider_id,
                                            e
                                        );
                                        (provider_id.clone(), None, None)
                                    }
                                }
                            }
                            Ok(None) => (provider_id.clone(), None, None),
                            Err(e) => {
                                tracing::error!("Failed to get provider {}: {}", provider_id, e);
                                (provider_id.clone(), None, None)
                            }
                        }
                    }
                })
                .collect();

            let results = join_all(fetch_futures).await;

            let mut result: HashMap<UUID, Vec<ModelResponse>> = HashMap::new();
            for (provider_id, models_opt, provider_type_opt) in results {
                if let (Some(models), Some(provider_type)) = (models_opt, provider_type_opt) {
                    let model_responses: Vec<ModelResponse> = models
                        .into_iter()
                        .map(|m| ModelResponse::from_model_with_provider(m, Some(&provider_type)))
                        .filter(|m| m.chat_method.is_some())
                        .collect();
                    result.insert(provider_id, model_responses);
                }
            }

            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::Model;

    fn create_test_model_with_chat_methods(id: &str, provider_id: &str) -> Model {
        Model {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            name: format!("Test Model {id}"),
            context_length: Some(4096),
            output_max_tokens: Some(2048),
            supported_features: None,
            description: Some("A test model".to_string()),
            input_modalities: None,
            output_modalities: None,
            metadata: None,
            pricing: None,
            url: None,
            supported_parameters: None,
            default_parameters: None,
            max_parameters: None,
            supported_methods: Some(vec!["completions".to_string()]),
            model_created_at: None,
            enabled: true,
            favorite: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn create_test_model_without_chat_methods(id: &str, provider_id: &str) -> Model {
        Model {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            name: format!("Test Model {id}"),
            context_length: Some(4096),
            output_max_tokens: Some(2048),
            supported_features: None,
            description: Some("A test model without chat methods".to_string()),
            input_modalities: None,
            output_modalities: None,
            metadata: None,
            pricing: None,
            url: None,
            supported_parameters: None,
            default_parameters: None,
            max_parameters: None,
            supported_methods: None,
            model_created_at: None,
            enabled: true,
            favorite: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn test_model_response_conversion_filters_empty_chat_methods() {
        // Models that resolve to no chat method must be dropped by the conversion.
        let models = vec![
            create_test_model_with_chat_methods("model1", "provider1"),
            create_test_model_without_chat_methods("model2", "provider1"),
            create_test_model_with_chat_methods("model3", "provider1"),
        ];

        // Mirrors the conversion and filtering done by get_provider_models.
        let filtered_responses: Vec<ModelResponse> = models
            .into_iter()
            .map(ModelResponse::from_model)
            .filter(|model| model.chat_method.is_some())
            .collect();

        for response in &filtered_responses {
            assert!(
                response.chat_method.is_some(),
                "All returned models should have chat_method"
            );
            assert!(
                response.supported_chat_methods.is_some(),
                "All returned models should have supported_chat_methods"
            );
        }

        assert!(
            !filtered_responses.is_empty(),
            "Should have at least some models with chat_method"
        );
    }

    #[test]
    fn test_model_response_conversion_empty_input() {
        let models: Vec<Model> = vec![];

        let responses: Vec<ModelResponse> = models
            .into_iter()
            .map(ModelResponse::from_model)
            .filter(|model| model.chat_method.is_some())
            .collect();

        assert_eq!(responses.len(), 0);
    }
}
