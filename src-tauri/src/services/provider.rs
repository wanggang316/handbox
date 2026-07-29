use crate::models::{AddProviderRequest, AppError};
use crate::services::{Database, ModelService};
use crate::storage::types::{Model, Provider, Timestamp, UUID};
use crate::storage::ProviderRepository;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone)]
pub struct ProviderService {
    provider_repo: ProviderRepository,
    model_service: ModelService,
}

impl ProviderService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            provider_repo: ProviderRepository::new(Arc::clone(&db)),
            model_service: ModelService::new(Arc::clone(&db)),
        }
    }

    pub fn initialize_model_database() {
        // The LLM client is assembled on demand, so there is nothing to preload.
        tracing::info!("Dynamic LLM client system ready");
    }

    /// Creates a provider and prefills its models from the hand-ai catalog.
    ///
    /// The API key is not verified here: `fetch_and_sync_models` reads a static catalog
    /// without any network call, so a bad key only surfaces on the first real request.
    /// TODO: add a lightweight one-shot probe to restore API key validation.
    pub async fn create_provider(&self, config: AddProviderRequest) -> Result<Provider, AppError> {
        let provider = Provider {
            id: Uuid::new_v4().to_string(),
            name: config.name,
            provider_type: config.provider_type,
            base_url: config.base_url,
            api_key: config.api_key,
            enabled: config.enabled.unwrap_or(true),
            created_at: self.current_timestamp(),
            updated_at: self.current_timestamp(),
        };

        self.provider_repo.create_provider(&provider).await?;

        self.model_service
            .fetch_and_sync_models(&provider, false)
            .await?;

        tracing::info!(
            "Successfully created provider and models: {}",
            provider.name
        );

        Ok(provider)
    }

    /// Test-only variant that stores the provider row without prefilling the model catalog.
    #[cfg(test)]
    async fn create_provider_without_validation(
        &self,
        config: AddProviderRequest,
    ) -> Result<Provider, AppError> {
        let provider = Provider {
            id: Uuid::new_v4().to_string(),
            name: config.name,
            provider_type: config.provider_type,
            base_url: config.base_url,
            api_key: config.api_key,
            enabled: config.enabled.unwrap_or(true),
            created_at: self.current_timestamp(),
            updated_at: self.current_timestamp(),
        };

        self.provider_repo.create_provider(&provider).await?;
        Ok(provider)
    }

    pub async fn update_provider(
        &self,
        provider_id: &UUID,
        config: AddProviderRequest,
    ) -> Result<Provider, AppError> {
        let existing_provider = self.get_provider(provider_id).await?;

        // Connection-level changes invalidate the model rows synced for this provider.
        let should_refresh_models = config.api_key != existing_provider.api_key
            || config.base_url != existing_provider.base_url
            || config.provider_type != existing_provider.provider_type;

        let updated_provider = Provider {
            id: existing_provider.id,
            name: config.name,
            provider_type: config.provider_type,
            base_url: config.base_url,
            api_key: config.api_key,
            enabled: config.enabled.unwrap_or(existing_provider.enabled),
            created_at: existing_provider.created_at,
            updated_at: self.current_timestamp(),
        };

        if should_refresh_models {
            self.model_service
                .fetch_and_sync_models(&updated_provider, true)
                .await?;
        }

        self.provider_repo
            .update_provider(&updated_provider)
            .await?;

        tracing::info!(
            "Successfully updated provider{}: {}",
            if should_refresh_models {
                " and synced models"
            } else {
                ""
            },
            updated_provider.name
        );

        Ok(updated_provider)
    }

    pub async fn get_provider(&self, provider_id: &UUID) -> Result<Provider, AppError> {
        self.provider_repo
            .get_provider_by_id(provider_id)
            .await?
            .ok_or_else(|| AppError::validation_error("Provider not found"))
    }

    pub async fn list_providers(&self) -> Result<Vec<Provider>, AppError> {
        self.provider_repo.list_providers().await
    }

    pub async fn get_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<Model>, AppError> {
        self.model_service.get_model(provider_id, model_id).await
    }

    pub async fn delete_provider(&self, provider_id: &UUID) -> Result<(), AppError> {
        self.provider_repo.delete_provider(provider_id).await
    }

    pub async fn toggle_provider(
        &self,
        provider_id: &UUID,
        enabled: bool,
    ) -> Result<Provider, AppError> {
        let mut provider = self.get_provider(provider_id).await?;

        provider.enabled = enabled;
        provider.updated_at = self.current_timestamp();

        self.provider_repo.update_provider(&provider).await?;

        Ok(provider)
    }

    fn current_timestamp(&self) -> Timestamp {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AddProviderRequest;
    use crate::storage::Database;
    use tempfile::tempdir;

    async fn create_service() -> (ProviderService, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).await.unwrap();
        (ProviderService::new(Arc::new(database)), temp_dir)
    }

    #[tokio::test]
    async fn create_provider_stores_record() {
        let (service, _dir) = create_service().await;

        let config = AddProviderRequest {
            name: "Test Provider".to_string(),
            provider_type: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "test-key".to_string(),
            enabled: Some(false),
        };

        let created = service
            .create_provider_without_validation(config)
            .await
            .unwrap();
        let fetched = service.get_provider(&created.id).await.unwrap();

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Test Provider");
        assert_eq!(fetched.provider_type, "anthropic");
    }

    #[tokio::test]
    async fn list_providers_returns_all() {
        let (service, _dir) = create_service().await;

        let configs = vec![
            AddProviderRequest {
                name: "OpenAI Provider".to_string(),
                provider_type: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "key1".to_string(),
                enabled: Some(true),
            },
            AddProviderRequest {
                name: "Anthropic Provider".to_string(),
                provider_type: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                api_key: "key2".to_string(),
                enabled: Some(false),
            },
        ];

        for cfg in configs {
            service
                .create_provider_without_validation(cfg)
                .await
                .unwrap();
        }

        let providers = service.list_providers().await.unwrap();
        assert_eq!(providers.len(), 2);
    }

    #[tokio::test]
    async fn update_provider_changes_fields() {
        let (service, _dir) = create_service().await;

        let provider = service
            .create_provider_without_validation(AddProviderRequest {
                name: "Original".to_string(),
                provider_type: "google".to_string(),
                base_url: "https://api.google.com".to_string(),
                api_key: "original".to_string(),
                enabled: Some(false),
            })
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

        let updated = service
            .update_provider(
                &provider.id,
                AddProviderRequest {
                    name: "Updated".to_string(),
                    provider_type: "google".to_string(),
                    base_url: "https://api.google.com".to_string(), // unchanged: avoids model resync
                    api_key: "original".to_string(), // unchanged: avoids model resync
                    enabled: Some(true),
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.base_url, "https://api.google.com");
        assert!(updated.enabled);
        assert!(updated.updated_at > provider.updated_at);
    }

    #[tokio::test]
    async fn delete_provider_removes_record() {
        let (service, _dir) = create_service().await;

        let provider = service
            .create_provider_without_validation(AddProviderRequest {
                name: "To Delete".to_string(),
                provider_type: "deepseek".to_string(),
                base_url: "https://api.deepseek.com".to_string(),
                api_key: "delete".to_string(),
                enabled: Some(true),
            })
            .await
            .unwrap();

        service.delete_provider(&provider.id).await.unwrap();
        assert!(service.get_provider(&provider.id).await.is_err());
    }

    #[tokio::test]
    async fn toggle_provider_updates_flag() {
        let (service, _dir) = create_service().await;

        let provider = service
            .create_provider_without_validation(AddProviderRequest {
                name: "Toggle".to_string(),
                provider_type: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                api_key: "toggle".to_string(),
                enabled: Some(false),
            })
            .await
            .unwrap();

        assert!(!provider.enabled);
        assert!(
            service
                .toggle_provider(&provider.id, true)
                .await
                .unwrap()
                .enabled
        );
        assert!(
            !service
                .toggle_provider(&provider.id, false)
                .await
                .unwrap()
                .enabled
        );
    }
}
